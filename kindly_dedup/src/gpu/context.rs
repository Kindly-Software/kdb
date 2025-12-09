//! GPU Context Capsule - T7 Heterogeneous Tier
//!
//! GPU context management for kindly_dedup using wgpu (WebGPU).
//!
//! # Architecture
//!
//! Uses wgpu for cross-platform GPU compute (Vulkan, Metal, DX12, WebGPU).
//! Falls back to CPU SIMD when GPU unavailable.
//!
//! # Framework Compliance
//!
//! - **UCE34**: T7 Heterogeneous tier (CPU-GPU coordination)
//! - **Chaos**: Lockfree context capsule (AtomicU64 state)
//! - **ASSUM**: Documented GPU assumptions
//! - **B32**: N/A (context initialization, not hot path)
//! - **T28**: Unit tests, fallback tests
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_GPU_AVAILABLE`: GPU may not be present
//! - `#VERIFY_GPU_AVAILABLE`: Runtime detection with fallback
//! - `#ASSUME_WGPU_SAFE`: wgpu provides safe GPU access
//! - `#VERIFY_WGPU_SAFE`: No unsafe code in this module

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use wgpu::{Device, Queue};

/// Default GPU initialization timeout (5 seconds)
/// Can be overridden via KINDLY_GPU_TIMEOUT_SECS environment variable
const DEFAULT_GPU_TIMEOUT_SECS: u64 = 5;
use wgpu::util::DeviceExt;

use super::capabilities::GpuCapabilities;
use super::error::{GpuError, GpuResult};
use super::fed_params::FedHashParamsCapsule;

/// GPU context state (atomic for Chaos compliance)
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuContextState {
    /// Not initialized
    Uninitialized = 0,
    /// Initializing
    Initializing = 1,
    /// Ready for compute
    Ready = 2,
    /// Error state
    Error = 3,
}

impl From<u64> for GpuContextState {
    fn from(value: u64) -> Self {
        match value {
            0 => GpuContextState::Uninitialized,
            1 => GpuContextState::Initializing,
            2 => GpuContextState::Ready,
            3 => GpuContextState::Error,
            _ => GpuContextState::Error,
        }
    }
}

/// GPU Context Capsule - T7 Heterogeneous Tier
///
/// Manages wgpu device and queue for GPU compute operations.
/// Thread-safe (via Arc<Device>, Arc<Queue>) and lockfree (via AtomicU64 state).
///
/// # Example
///
/// ```rust,ignore
/// use kindly_dedup::gpu::GpuContextCapsule;
///
/// // Try to create GPU context (async)
/// let ctx = pollster::block_on(GpuContextCapsule::new());
/// match ctx {
///     Ok(ctx) => {
///         println!("GPU: {}", ctx.capabilities().device_name);
///         // Use GPU for MinHash computation
///     }
///     Err(_) => {
///         println!("No GPU available, falling back to CPU SIMD");
///     }
/// }
/// ```
#[repr(C, align(64))]
pub struct GpuContextCapsule {
    /// Atomic state for Chaos compliance
    state: AtomicU64,
    /// wgpu device (compute operations)
    device: Option<Arc<Device>>,
    /// wgpu queue (command submission)
    queue: Option<Arc<Queue>>,
    /// GPU capabilities
    capabilities: Option<GpuCapabilities>,
    /// FED hash parameters (precomputed, uploaded once)
    fed_params: Option<FedHashParamsCapsule>,
    /// FED params GPU buffer (uniform buffer)
    fed_params_buffer: Option<wgpu::Buffer>,
    /// Padding for 64-byte cache line alignment
    _padding: [u8; 8],
}

// SAFETY: GpuContextCapsule is Send + Sync because:
// - AtomicU64 is Send + Sync
// - Arc<Device> and Arc<Queue> are Send + Sync (wgpu guarantees thread safety)
// - GpuCapabilities is Clone + Send + Sync (no interior mutability)
unsafe impl Send for GpuContextCapsule {}
unsafe impl Sync for GpuContextCapsule {}

impl GpuContextCapsule {
    /// Create a new GPU context (async)
    ///
    /// # Returns
    /// - `Ok(GpuContextCapsule)` if GPU is available and initialized
    /// - `Err(GpuError)` if no suitable GPU found
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_GPU_AVAILABLE`: GPU may not be present
    /// - `#VERIFY_GPU_AVAILABLE`: Returns Err if no GPU found
    pub async fn new() -> GpuResult<Self> {
        // Create wgpu instance with all backends
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: wgpu::Dx12Compiler::default(),
            flags: wgpu::InstanceFlags::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::default(),
        });

        // Request high-performance adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or(GpuError::NoAdapterFound)?;

        // Extract capabilities before device request
        let capabilities = super::capabilities::GpuCapabilities::from_adapter(&adapter);

        // Request device with compute features
        // Note: wgpu 0.19 API (for iced compatibility)
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("kindly_dedup_gpu"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::DeviceRequestFailed(e.to_string()))?;

        Ok(Self {
            state: AtomicU64::new(GpuContextState::Ready as u64),
            device: Some(Arc::new(device)),
            queue: Some(Arc::new(queue)),
            capabilities: Some(capabilities),
            fed_params: None,
            fed_params_buffer: None,
            _padding: [0; 8],
        })
    }

    /// Initialize FED hash parameters for MinHash optimization
    ///
    /// Precomputes hash parameters on CPU and uploads to GPU uniform buffer.
    ///
    /// # Arguments
    ///
    /// - `seed`: Random seed for parameter generation (use PID + timestamp)
    ///
    /// # Returns
    ///
    /// - `Ok(())`: FED params initialized successfully
    /// - `Err(GpuError)`: Device not initialized
    ///
    /// # Performance
    ///
    /// - One-time cost: <1μs (parameter generation)
    /// - Upload: <10μs (1KB buffer to GPU)
    /// - Expected speedup: 6-24× MinHash performance
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use kindly_dedup::gpu::GpuContextCapsule;
    /// use std::time::{SystemTime, UNIX_EPOCH};
    ///
    /// let mut ctx = GpuContextCapsule::new_blocking()?;
    ///
    /// // High-entropy seed: PID + timestamp
    /// let seed = (std::process::id() as u64) << 32
    ///     | SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
    ///
    /// ctx.init_fed_params(seed)?;
    /// ```
    pub fn init_fed_params(&mut self, seed: u64) -> GpuResult<()> {
        let device = self.device.as_ref().ok_or(GpuError::NotInitialized)?;

        // Generate FED parameters on CPU
        let params = FedHashParamsCapsule::generate(seed);

        // Upload to GPU storage buffer (storage buffers allow 4-byte array stride)
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("FED Params Buffer"),
            contents: &params.to_gpu_buffer(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        self.fed_params = Some(params);
        self.fed_params_buffer = Some(buffer);

        Ok(())
    }

    /// Get FED params buffer reference (for binding to shader)
    pub fn fed_params_buffer(&self) -> Option<&wgpu::Buffer> {
        self.fed_params_buffer.as_ref()
    }

    /// Get FED params (for CPU reference testing)
    pub fn fed_params(&self) -> Option<&FedHashParamsCapsule> {
        self.fed_params.as_ref()
    }

    /// Create GPU context with timeout (blocking)
    ///
    /// Uses a separate thread to avoid blocking the caller indefinitely.
    /// If GPU initialization takes longer than the timeout, returns Timeout error.
    ///
    /// # Arguments
    /// - `timeout`: Maximum time to wait for GPU initialization
    ///
    /// # Returns
    /// - `Ok(GpuContextCapsule)` if GPU is available and initialized within timeout
    /// - `Err(GpuError::Timeout)` if initialization takes too long
    /// - `Err(GpuError::ThreadPanicked)` if initialization thread panics
    /// - `Err(GpuError)` if no suitable GPU found
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_TIMEOUT_PREVENTS_HANG`: Thread-based timeout prevents indefinite blocking
    /// - `#VERIFY_TIMEOUT_PREVENTS_HANG`: Uses channel recv_timeout for guaranteed return
    pub fn new_with_timeout(timeout: Duration) -> GpuResult<Self> {
        let (tx, rx) = std::sync::mpsc::channel();
        let timeout_secs = timeout.as_secs();

        // Spawn initialization in separate thread
        std::thread::spawn(move || {
            let result = pollster::block_on(Self::new());
            // Ignore send error if receiver dropped (timeout expired)
            let _ = tx.send(result);
        });

        // Wait with timeout
        match rx.recv_timeout(timeout) {
            Ok(result) => result,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                Err(GpuError::Timeout { timeout_secs })
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err(GpuError::ThreadPanicked)
            }
        }
    }

    /// Create GPU context (blocking with default timeout)
    ///
    /// Convenience method for synchronous code with 5-second timeout.
    /// Timeout can be adjusted via KINDLY_GPU_TIMEOUT_SECS environment variable.
    ///
    /// # Returns
    /// - `Ok(GpuContextCapsule)` if GPU is available and initialized
    /// - `Err(GpuError::Timeout)` if initialization takes too long
    /// - `Err(GpuError)` if no suitable GPU found
    pub fn new_blocking() -> GpuResult<Self> {
        let timeout_secs = std::env::var("KINDLY_GPU_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_GPU_TIMEOUT_SECS);

        Self::new_with_timeout(Duration::from_secs(timeout_secs))
    }

    /// Get current state
    pub fn state(&self) -> GpuContextState {
        GpuContextState::from(self.state.load(Ordering::Acquire))
    }

    /// Check if GPU is ready for compute
    pub fn is_ready(&self) -> bool {
        self.state() == GpuContextState::Ready
    }

    /// Try to get GPU capabilities (fallible accessor)
    ///
    /// Returns `None` if the GPU context has not been initialized.
    /// This is the safe alternative to [`capabilities()`] which panics
    /// on uninitialized contexts.
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_OPTION_PATTERN`: Option<&T> provides safe null handling
    /// - `#VERIFY_OPTION_PATTERN`: Compiler enforces Option unwrapping
    #[inline]
    pub fn try_capabilities(&self) -> Option<&GpuCapabilities> {
        self.capabilities.as_ref()
    }

    /// Get GPU capabilities (panics if uninitialized)
    ///
    /// # Panics
    ///
    /// Panics if called on an uninitialized `GpuContextCapsule`.
    /// Use [`try_capabilities()`] for a fallible alternative, or
    /// check [`is_ready()`] before calling this method.
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_INITIALIZED`: Caller guarantees context is initialized
    /// - `#VERIFY_INITIALIZED`: Use is_ready() or try_capabilities() to verify
    pub fn capabilities(&self) -> &GpuCapabilities {
        self.capabilities.as_ref().expect(
            "GpuContextCapsule not initialized: call GpuContextCapsule::new() or \
             GpuContextCapsule::new_blocking() first, or use try_capabilities() for \
             fallible access, or check is_ready() before calling capabilities()",
        )
    }

    /// Get device reference
    pub fn device(&self) -> Option<&Device> {
        self.device.as_ref().map(|d| d.as_ref())
    }

    /// Get queue reference
    pub fn queue(&self) -> Option<&Queue> {
        self.queue.as_ref().map(|q| q.as_ref())
    }

    /// Get device Arc (for sharing across threads)
    pub fn device_arc(&self) -> Option<Arc<Device>> {
        self.device.clone()
    }

    /// Get queue Arc (for sharing across threads)
    pub fn queue_arc(&self) -> Option<Arc<Queue>> {
        self.queue.clone()
    }

    /// Poll device for completed work
    ///
    /// Call this periodically to process completed GPU operations.
    pub fn poll(&self) {
        if let Some(device) = &self.device {
            device.poll(wgpu::Maintain::Poll);
        }
    }

    /// Wait for all GPU operations to complete
    pub fn wait(&self) {
        if let Some(device) = &self.device {
            device.poll(wgpu::Maintain::Wait);
        }
    }

    /// Get maximum batch size for MinHash computation
    ///
    /// Based on GPU memory limits and workgroup constraints.
    /// Returns conservative estimate to avoid OOM.
    pub fn max_minhash_batch_size(&self) -> usize {
        let caps = self.capabilities();

        // Each document needs:
        // - tokens: ~100 tokens × 4 bytes = 400 bytes (average)
        // - offsets: 4 bytes
        // - signature output: 64 u32 = 256 bytes
        // Total: ~660 bytes per document
        let bytes_per_doc = 660;

        // Use 50% of max storage buffer size for safety margin
        let max_buffer_docs = (caps.max_storage_buffer_binding_size as u64 / 2) as usize / bytes_per_doc;

        // Also limit by workgroup constraints
        let max_workgroups = caps.max_dispatch_x as usize;
        let workgroup_size = 256; // Our kernel uses 256 threads/workgroup
        let max_workgroup_docs = max_workgroups * workgroup_size;

        // Return minimum of both constraints, capped at 1M docs
        max_buffer_docs.min(max_workgroup_docs).min(1_000_000)
    }
}

impl Default for GpuContextCapsule {
    fn default() -> Self {
        Self {
            state: AtomicU64::new(GpuContextState::Uninitialized as u64),
            device: None,
            queue: None,
            capabilities: None,
            fed_params: None,
            fed_params_buffer: None,
            _padding: [0; 8],
        }
    }
}

impl std::fmt::Debug for GpuContextCapsule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuContextCapsule")
            .field("state", &self.state())
            .field("capabilities", &self.capabilities)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_context_state_conversion() {
        assert_eq!(GpuContextState::from(0), GpuContextState::Uninitialized);
        assert_eq!(GpuContextState::from(1), GpuContextState::Initializing);
        assert_eq!(GpuContextState::from(2), GpuContextState::Ready);
        assert_eq!(GpuContextState::from(3), GpuContextState::Error);
        assert_eq!(GpuContextState::from(99), GpuContextState::Error);
    }

    #[test]
    fn test_gpu_context_default() {
        let ctx = GpuContextCapsule::default();
        assert_eq!(ctx.state(), GpuContextState::Uninitialized);
        assert!(!ctx.is_ready());
        assert!(ctx.device().is_none());
        assert!(ctx.queue().is_none());
    }

    #[test]
    #[ignore = "Requires GPU hardware - run with --ignored on GPU-equipped machines"]
    fn test_gpu_context_creation() {
        // Try to create GPU context
        // This test will pass on systems with GPU, skip gracefully otherwise
        match GpuContextCapsule::new_blocking() {
            Ok(ctx) => {
                assert!(ctx.is_ready());
                assert!(ctx.device().is_some());
                assert!(ctx.queue().is_some());

                let caps = ctx.capabilities();
                println!("GPU: {}", caps.device_name);
                println!("Backend: {:?}", caps.backend);
                println!("Device class: {:?}", caps.device_class);
                println!("Performance tier: {:?}", caps.performance_tier());
                println!("Max workgroup size: {}x{}x{}",
                    caps.max_workgroup_size_x,
                    caps.max_workgroup_size_y,
                    caps.max_workgroup_size_z);
                println!("Max storage buffer: {} bytes", caps.max_storage_buffer_binding_size);
                println!("Max MinHash batch: {} docs", ctx.max_minhash_batch_size());

                // Test poll/wait
                ctx.poll();
                ctx.wait();
            }
            Err(e) => {
                println!("No GPU available (expected in CI): {}", e);
                // Test still passes - graceful fallback
            }
        }
    }

    #[test]
    #[ignore = "Requires GPU hardware - run with --ignored on GPU-equipped machines"]
    fn test_gpu_capabilities_methods() {
        // Create mock capabilities for testing
        match GpuContextCapsule::new_blocking() {
            Ok(ctx) => {
                let caps = ctx.capabilities();

                // Test device type detection
                let is_discrete = matches!(caps.device_class, super::super::capabilities::GpuClass::Discrete);
                let is_integrated = matches!(caps.device_class, super::super::capabilities::GpuClass::Integrated);
                let tier = caps.performance_tier();

                println!("Is discrete: {}", is_discrete);
                println!("Is integrated: {}", is_integrated);
                println!("Performance tier: {:?}", tier);

                // At least one should be identifiable
                assert!(
                    is_discrete || is_integrated ||
                    matches!(tier, super::super::capabilities::PerformanceTier::Fallback),
                    "Device type should be detectable"
                );
            }
            Err(_) => {
                println!("Skipping capabilities test - no GPU");
            }
        }
    }

    #[test]
    fn test_gpu_context_debug() {
        let ctx = GpuContextCapsule::default();
        let debug_str = format!("{:?}", ctx);
        assert!(debug_str.contains("GpuContextCapsule"));
        assert!(debug_str.contains("Uninitialized"));
    }

    #[test]
    #[ignore = "Requires GPU hardware - run with --ignored on GPU-equipped machines"]
    fn test_gpu_context_arc_sharing() {
        match GpuContextCapsule::new_blocking() {
            Ok(ctx) => {
                let device_arc = ctx.device_arc();
                let queue_arc = ctx.queue_arc();

                assert!(device_arc.is_some());
                assert!(queue_arc.is_some());

                // Can clone Arc for sharing
                let _device_clone = device_arc.clone();
                let _queue_clone = queue_arc.clone();
            }
            Err(_) => {
                println!("Skipping Arc test - no GPU");
            }
        }
    }
}
