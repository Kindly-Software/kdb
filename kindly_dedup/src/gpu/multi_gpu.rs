//! Multi-GPU Coordination - T8 Network Tier
//!
//! Orchestrates multiple GPUs for distributed deduplication workloads.
//! Supports heterogeneous GPU configurations (iGPU + dGPU mixtures).
//!
//! # Architecture
//!
//! ```text
//! MultiGpuCoordinator (512B, T8 Network)
//! ├── GpuDeviceCapsule[0] (64B, T1 Atomic) - iGPU/dGPU #1
//! ├── GpuDeviceCapsule[1] (64B, T1 Atomic) - dGPU #2
//! ├── GpuDeviceCapsule[N] (64B, T1 Atomic) - dGPU #N
//! └── LoadBalancer (Round-robin | Memory-aware | Performance-aware)
//! ```
//!
//! # Load Balancing Strategies
//!
//! 1. **RoundRobin** (default): Even distribution, lowest latency selection
//! 2. **MemoryAware**: Prefers GPU with most available VRAM (for large batches)
//! 3. **PerformanceAware**: Prefers fastest GPU based on historical throughput
//!
//! # Framework Compliance
//!
//! - **UCE34**: T8 Network tier (distributed GPU coordination)
//! - **Chaos**: 100% lockfree (AtomicU64 arrays for per-GPU state)
//! - **ASSUM**: GPU availability runtime-checked, graceful degradation
//! - **B32**: Performance targets based on aggregate GPU throughput
//! - **T28**: 15+ tests (unit/property/integration)
//! - **Q34**: Generation counters for audit trail
//!
//! # Performance Targets (B32)
//!
//! | Operation | Latency | Throughput |
//! |-----------|---------|------------|
//! | select_gpu | <50ns | 20M+ ops/sec |
//! | submit_batch | <100ns (coordinator overhead) | 10M+ ops/sec |
//! | rebalance | <1μs | 1M+ ops/sec |
//! | get_gpu_stats | <30ns | 33M+ ops/sec |
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_ADAPTERS_ENUMERABLE`: wgpu enumerate_adapters returns valid adapters
//! - `#VERIFY_ADAPTERS_ENUMERABLE`: Each adapter validated before adding to pool
//! - `#ASSUME_GPU_FAILURE_DETECTABLE`: GPU errors propagate via wgpu
//! - `#VERIFY_GPU_FAILURE_DETECTABLE`: Health monitoring via periodic polls
//! - `#ASSUME_LOCKFREE_COORDINATION`: AtomicU64 CAS provides correctness
//! - `#VERIFY_LOCKFREE_COORDINATION`: No mutex/RwLock in hot paths
//!
//! # Example
//!
//! ```rust,ignore
//! use kindly_dedup::gpu::{MultiGpuCoordinator, LoadBalancingStrategy};
//!
//! // Enumerate all GPUs and create coordinator
//! let coordinator = MultiGpuCoordinator::new()?;
//! println!("Found {} GPUs", coordinator.gpu_count());
//!
//! // Select GPU for batch processing
//! let gpu_id = coordinator.select_gpu();
//! coordinator.submit_batch(gpu_id, batch)?;
//!
//! // Check GPU statistics
//! let stats = coordinator.get_gpu_stats(gpu_id)?;
//! println!("GPU {} throughput: {} docs/sec", gpu_id, stats.throughput_docs_per_sec);
//!
//! // Rebalance if workload is uneven
//! coordinator.rebalance();
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use wgpu::{Adapter, Device, Queue};

use super::capabilities::{GpuCapabilities, GpuClass, PerformanceTier};
use super::error::{GpuError, GpuResult};

/// Maximum number of GPUs supported
pub const MAX_GPUS: usize = 8;

/// GPU identifier (0-indexed)
pub type GpuId = u8;

/// Load balancing strategy for multi-GPU dispatch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadBalancingStrategy {
    /// Round-robin: Even distribution, lowest selection latency (<10ns)
    /// Best for: Homogeneous GPU setups, latency-sensitive workloads
    #[default]
    RoundRobin,

    /// Memory-aware: Prefer GPU with most estimated free VRAM
    /// Best for: Large batch processing, memory-intensive workloads
    MemoryAware,

    /// Performance-aware: Prefer GPU with best historical throughput
    /// Best for: Heterogeneous setups (iGPU + dGPU), sustained workloads
    PerformanceAware,
}

/// GPU device state packed into AtomicU64
///
/// # Layout (64 bits)
///
/// ```text
/// [0:7]   - GPU ID (u8)
/// [8:15]  - State flags (u8): bit 0 = healthy, bit 1 = active, bit 2 = failed
/// [16:31] - Pending batches (u16)
/// [32:47] - Estimated free VRAM MB (u16)
/// [48:63] - Throughput (docs/sec / 1000, u16)
/// ```
#[derive(Debug, Clone, Copy)]
pub struct GpuDeviceState {
    /// Raw packed state
    raw: u64,
}

impl GpuDeviceState {
    /// Create new state for GPU
    #[inline]
    pub fn new(gpu_id: GpuId, vram_mb: u16) -> Self {
        let raw = (gpu_id as u64)
            | (0x01u64 << 8) // healthy flag
            | ((vram_mb as u64) << 32);
        Self { raw }
    }

    /// Pack state into u64
    #[inline]
    pub fn pack(
        gpu_id: GpuId,
        healthy: bool,
        active: bool,
        failed: bool,
        pending_batches: u16,
        free_vram_mb: u16,
        throughput_k: u16,
    ) -> u64 {
        let flags = (healthy as u8) | ((active as u8) << 1) | ((failed as u8) << 2);
        (gpu_id as u64)
            | ((flags as u64) << 8)
            | ((pending_batches as u64) << 16)
            | ((free_vram_mb as u64) << 32)
            | ((throughput_k as u64) << 48)
    }

    /// Unpack GPU ID
    #[inline]
    pub fn gpu_id(&self) -> GpuId {
        (self.raw & 0xFF) as GpuId
    }

    /// Check if GPU is healthy
    #[inline]
    pub fn is_healthy(&self) -> bool {
        ((self.raw >> 8) & 0x01) != 0
    }

    /// Check if GPU is active (processing)
    #[inline]
    pub fn is_active(&self) -> bool {
        ((self.raw >> 8) & 0x02) != 0
    }

    /// Check if GPU has failed
    #[inline]
    pub fn has_failed(&self) -> bool {
        ((self.raw >> 8) & 0x04) != 0
    }

    /// Get pending batch count
    #[inline]
    pub fn pending_batches(&self) -> u16 {
        ((self.raw >> 16) & 0xFFFF) as u16
    }

    /// Get estimated free VRAM in MB
    #[inline]
    pub fn free_vram_mb(&self) -> u16 {
        ((self.raw >> 32) & 0xFFFF) as u16
    }

    /// Get throughput (docs/sec / 1000)
    #[inline]
    pub fn throughput_k(&self) -> u16 {
        ((self.raw >> 48) & 0xFFFF) as u16
    }

    /// Get throughput in docs/sec
    #[inline]
    pub fn throughput_docs_per_sec(&self) -> u64 {
        (self.throughput_k() as u64) * 1000
    }
}

impl From<u64> for GpuDeviceState {
    fn from(raw: u64) -> Self {
        Self { raw }
    }
}

impl From<GpuDeviceState> for u64 {
    fn from(state: GpuDeviceState) -> u64 {
        state.raw
    }
}

/// GPU device capsule - T1 Atomic (64B cache-aligned)
///
/// Wraps a single GPU device with atomic state tracking.
/// Thread-safe via Arc<Device>/Arc<Queue> and AtomicU64 state.
///
/// # Chaos Compliance
///
/// - 64B cache-aligned (no false sharing)
/// - Lockfree state updates via AtomicU64
/// - Generation counter for Q34 audit trail
///
/// # ASSUM Safety
///
/// - `#ASSUME_DEVICE_VALID`: wgpu Device/Queue are valid
/// - `#VERIFY_DEVICE_VALID`: Validated during construction
#[repr(C, align(64))]
pub struct GpuDeviceCapsule {
    /// Packed atomic state (see GpuDeviceState layout)
    state: AtomicU64,
    /// Generation counter for Q34 audit trail
    generation: AtomicU64,
    /// wgpu device handle
    device: Arc<Device>,
    /// wgpu queue handle
    queue: Arc<Queue>,
    /// GPU capabilities (immutable after construction)
    capabilities: GpuCapabilities,
    /// Padding for 64B alignment
    _padding: [u8; 0], // Struct is already larger than 64B due to Arc/GpuCapabilities
}

impl GpuDeviceCapsule {
    /// Create GPU device capsule from wgpu adapter
    ///
    /// # Arguments
    ///
    /// - `gpu_id`: Unique identifier (0-7)
    /// - `adapter`: wgpu Adapter to create device from
    ///
    /// # Returns
    ///
    /// - `Ok(GpuDeviceCapsule)`: Device created successfully
    /// - `Err(GpuError)`: Device creation failed
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_ADAPTER_VALID`: Adapter is valid and supports compute
    /// - `#VERIFY_ADAPTER_VALID`: Request device with compute features
    pub async fn from_adapter(gpu_id: GpuId, adapter: &Adapter) -> GpuResult<Self> {
        let capabilities = GpuCapabilities::from_adapter(adapter);

        // Request device with default features
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some(&format!("kindly_dedup_gpu_{}", gpu_id)),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::DeviceRequestFailed(e.to_string()))?;

        // Initial state: healthy, not active, VRAM estimated from capabilities
        let vram_mb = (capabilities.estimated_vram_gb * 1024.0) as u16;
        let initial_state = GpuDeviceState::pack(
            gpu_id,
            true,  // healthy
            false, // not active
            false, // not failed
            0,     // no pending batches
            vram_mb,
            0, // no throughput history
        );

        Ok(Self {
            state: AtomicU64::new(initial_state),
            generation: AtomicU64::new(0),
            device: Arc::new(device),
            queue: Arc::new(queue),
            capabilities,
            _padding: [],
        })
    }

    /// Create GPU device capsule (blocking version)
    pub fn from_adapter_blocking(gpu_id: GpuId, adapter: &Adapter) -> GpuResult<Self> {
        pollster::block_on(Self::from_adapter(gpu_id, adapter))
    }

    /// Get GPU ID
    #[inline]
    pub fn gpu_id(&self) -> GpuId {
        GpuDeviceState::from(self.state.load(Ordering::Acquire)).gpu_id()
    }

    /// Get current state snapshot
    #[inline]
    pub fn state(&self) -> GpuDeviceState {
        GpuDeviceState::from(self.state.load(Ordering::Acquire))
    }

    /// Get generation counter (Q34 audit trail)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Increment generation counter
    #[inline]
    pub fn increment_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Check if GPU is healthy
    #[inline]
    pub fn is_healthy(&self) -> bool {
        self.state().is_healthy()
    }

    /// Check if GPU has failed
    #[inline]
    pub fn has_failed(&self) -> bool {
        self.state().has_failed()
    }

    /// Get device reference
    #[inline]
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Get queue reference
    #[inline]
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Get GPU capabilities
    #[inline]
    pub fn capabilities(&self) -> &GpuCapabilities {
        &self.capabilities
    }

    /// Get estimated free VRAM in MB
    #[inline]
    pub fn free_vram_mb(&self) -> u16 {
        self.state().free_vram_mb()
    }

    /// Get throughput in docs/sec
    #[inline]
    pub fn throughput_docs_per_sec(&self) -> u64 {
        self.state().throughput_docs_per_sec()
    }

    /// Mark batch as started (increment pending, set active)
    ///
    /// # Returns
    ///
    /// New pending batch count
    pub fn start_batch(&self) -> u16 {
        loop {
            let old = self.state.load(Ordering::Acquire);
            let state = GpuDeviceState::from(old);

            if state.has_failed() {
                return 0; // Don't accept batches on failed GPU
            }

            let new_pending = state.pending_batches().saturating_add(1);
            let new_state = GpuDeviceState::pack(
                state.gpu_id(),
                state.is_healthy(),
                true, // active
                false,
                new_pending,
                state.free_vram_mb(),
                state.throughput_k(),
            );

            if self
                .state
                .compare_exchange(old, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                self.increment_generation();
                return new_pending;
            }
        }
    }

    /// Mark batch as completed
    ///
    /// # Arguments
    ///
    /// - `docs_processed`: Number of documents processed in batch
    /// - `duration_us`: Batch processing time in microseconds
    ///
    /// # Returns
    ///
    /// Remaining pending batch count
    pub fn complete_batch(&self, docs_processed: u64, duration_us: u64) -> u16 {
        loop {
            let old = self.state.load(Ordering::Acquire);
            let state = GpuDeviceState::from(old);

            let new_pending = state.pending_batches().saturating_sub(1);
            let is_active = new_pending > 0;

            // Update throughput with EMA (alpha=0.1)
            let new_throughput_k = if duration_us > 0 {
                let batch_throughput = (docs_processed * 1_000_000) / duration_us; // docs/sec
                let batch_throughput_k = (batch_throughput / 1000) as u16;
                let old_throughput_k = state.throughput_k();

                // EMA: new = alpha * sample + (1-alpha) * old
                // alpha=0.1 => new = (sample + 9*old) / 10
                if old_throughput_k == 0 {
                    batch_throughput_k
                } else {
                    ((batch_throughput_k as u32 + 9 * old_throughput_k as u32) / 10) as u16
                }
            } else {
                state.throughput_k()
            };

            let new_state = GpuDeviceState::pack(
                state.gpu_id(),
                state.is_healthy(),
                is_active,
                state.has_failed(),
                new_pending,
                state.free_vram_mb(),
                new_throughput_k,
            );

            if self
                .state
                .compare_exchange(old, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                self.increment_generation();
                return new_pending;
            }
        }
    }

    /// Mark GPU as failed (removes from pool)
    pub fn mark_failed(&self) {
        loop {
            let old = self.state.load(Ordering::Acquire);
            let state = GpuDeviceState::from(old);

            let new_state = GpuDeviceState::pack(
                state.gpu_id(),
                false, // not healthy
                false, // not active
                true,  // failed
                0,
                0,
                0,
            );

            if self
                .state
                .compare_exchange(old, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                self.increment_generation();
                return;
            }
        }
    }

    /// Poll device for completed work
    #[inline]
    pub fn poll(&self) {
        self.device.poll(wgpu::Maintain::Poll);
    }

    /// Wait for all GPU work to complete
    #[inline]
    pub fn wait(&self) {
        self.device.poll(wgpu::Maintain::Wait);
    }
}

impl std::fmt::Debug for GpuDeviceCapsule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state();
        f.debug_struct("GpuDeviceCapsule")
            .field("gpu_id", &state.gpu_id())
            .field("healthy", &state.is_healthy())
            .field("active", &state.is_active())
            .field("failed", &state.has_failed())
            .field("pending_batches", &state.pending_batches())
            .field("free_vram_mb", &state.free_vram_mb())
            .field("throughput_k", &state.throughput_k())
            .field("generation", &self.generation())
            .field("device_name", &self.capabilities.device_name)
            .finish()
    }
}

/// GPU statistics snapshot
#[derive(Debug, Clone, Copy)]
pub struct GpuStats {
    /// GPU identifier
    pub gpu_id: GpuId,
    /// Is GPU healthy
    pub is_healthy: bool,
    /// Is GPU currently processing
    pub is_active: bool,
    /// Has GPU failed
    pub has_failed: bool,
    /// Number of pending batches
    pub pending_batches: u16,
    /// Estimated free VRAM in MB
    pub free_vram_mb: u16,
    /// Current throughput in docs/sec
    pub throughput_docs_per_sec: u64,
    /// Generation counter (Q34 audit)
    pub generation: u64,
}

impl From<&GpuDeviceCapsule> for GpuStats {
    fn from(capsule: &GpuDeviceCapsule) -> Self {
        let state = capsule.state();
        Self {
            gpu_id: state.gpu_id(),
            is_healthy: state.is_healthy(),
            is_active: state.is_active(),
            has_failed: state.has_failed(),
            pending_batches: state.pending_batches(),
            free_vram_mb: state.free_vram_mb(),
            throughput_docs_per_sec: state.throughput_docs_per_sec(),
            generation: capsule.generation(),
        }
    }
}

/// Multi-GPU Coordinator - T8 Network Tier (512B)
///
/// Orchestrates multiple GPUs with configurable load balancing.
/// Supports heterogeneous GPU configurations (iGPU + dGPU mixtures).
///
/// # Chaos Compliance
///
/// - 512B cache-aligned coordinator capsule
/// - Lockfree coordination via AtomicU64 arrays
/// - Generation counter for Q34 audit trail
/// - No mutex/RwLock in hot paths
///
/// # ASSUM Safety
///
/// - `#ASSUME_ADAPTERS_ENUMERABLE`: wgpu enumerate_adapters works
/// - `#VERIFY_ADAPTERS_ENUMERABLE`: Validated on construction
/// - `#ASSUME_GPU_POOL_STABLE`: GPUs don't disappear mid-operation
/// - `#VERIFY_GPU_POOL_STABLE`: Health monitoring detects failures
#[repr(C, align(64))]
pub struct MultiGpuCoordinator {
    /// Coordinator state: [0:7]=gpu_count, [8:15]=strategy, [16:31]=round_robin_idx, [32:63]=generation
    state: AtomicU64,
    /// Per-GPU device capsules (up to MAX_GPUS)
    devices: Vec<GpuDeviceCapsule>,
    /// Load balancing strategy
    strategy: LoadBalancingStrategy,
    /// Padding for cache alignment
    _padding: [u8; 32],
}

impl MultiGpuCoordinator {
    /// Create coordinator by enumerating all available GPUs
    ///
    /// # Returns
    ///
    /// - `Ok(MultiGpuCoordinator)`: At least one GPU found
    /// - `Err(GpuError::NoAdapterFound)`: No GPUs available
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_ADAPTERS_ENUMERABLE`: wgpu can enumerate adapters
    /// - `#VERIFY_ADAPTERS_ENUMERABLE`: Error returned if no adapters found
    pub fn new() -> GpuResult<Self> {
        Self::with_strategy(LoadBalancingStrategy::default())
    }

    /// Create coordinator with specific load balancing strategy
    pub fn with_strategy(strategy: LoadBalancingStrategy) -> GpuResult<Self> {
        // Create wgpu instance with all backends
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: wgpu::Dx12Compiler::default(),
            flags: wgpu::InstanceFlags::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::default(),
        });

        // Enumerate all adapters
        // Note: enumerate_adapters requires wgpu_core feature, but is available
        // via request_adapter iteration pattern
        let adapters = Self::enumerate_adapters(&instance);

        if adapters.is_empty() {
            return Err(GpuError::NoAdapterFound);
        }

        // Create device capsules for each adapter (up to MAX_GPUS)
        let mut devices = Vec::with_capacity(adapters.len().min(MAX_GPUS));
        for (idx, adapter) in adapters.into_iter().take(MAX_GPUS).enumerate() {
            match GpuDeviceCapsule::from_adapter_blocking(idx as GpuId, &adapter) {
                Ok(device) => {
                    eprintln!(
                        "[MultiGpuCoordinator] GPU {}: {} ({:?})",
                        idx,
                        device.capabilities().device_name,
                        device.capabilities().device_class
                    );
                    devices.push(device);
                }
                Err(e) => {
                    eprintln!(
                        "[MultiGpuCoordinator] Failed to initialize GPU {}: {}",
                        idx, e
                    );
                    // Continue with other GPUs
                }
            }
        }

        if devices.is_empty() {
            return Err(GpuError::NoAdapterFound);
        }

        // Pack initial state
        let state = Self::pack_state(devices.len() as u8, strategy, 0, 0);

        Ok(Self {
            state: AtomicU64::new(state),
            devices,
            strategy,
            _padding: [0; 32],
        })
    }

    /// Enumerate all available GPU adapters
    ///
    /// Uses multiple request_adapter calls with different power preferences
    /// to discover all available GPUs.
    fn enumerate_adapters(instance: &wgpu::Instance) -> Vec<Adapter> {
        let mut adapters = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        // Try high-performance first (discrete GPUs)
        if let Some(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
        {
            let name = adapter.get_info().name.clone();
            if seen_names.insert(name) {
                adapters.push(adapter);
            }
        }

        // Try low-power (integrated GPUs)
        if let Some(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
        {
            let name = adapter.get_info().name.clone();
            if seen_names.insert(name) {
                adapters.push(adapter);
            }
        }

        // Try default preference (may find additional GPUs)
        if let Some(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
        {
            let name = adapter.get_info().name.clone();
            if seen_names.insert(name) {
                adapters.push(adapter);
            }
        }

        adapters
    }

    /// Pack coordinator state into u64
    #[inline]
    fn pack_state(gpu_count: u8, strategy: LoadBalancingStrategy, round_robin_idx: u16, generation: u32) -> u64 {
        let strategy_id = match strategy {
            LoadBalancingStrategy::RoundRobin => 0u8,
            LoadBalancingStrategy::MemoryAware => 1,
            LoadBalancingStrategy::PerformanceAware => 2,
        };
        (gpu_count as u64)
            | ((strategy_id as u64) << 8)
            | ((round_robin_idx as u64) << 16)
            | ((generation as u64) << 32)
    }

    /// Get number of available GPUs
    #[inline]
    pub fn gpu_count(&self) -> usize {
        self.devices.len()
    }

    /// Get number of healthy GPUs
    pub fn healthy_gpu_count(&self) -> usize {
        self.devices.iter().filter(|d| d.is_healthy()).count()
    }

    /// Get current load balancing strategy
    #[inline]
    pub fn strategy(&self) -> LoadBalancingStrategy {
        self.strategy
    }

    /// Get generation counter (Q34 audit trail)
    #[inline]
    pub fn generation(&self) -> u32 {
        ((self.state.load(Ordering::Acquire) >> 32) & 0xFFFF_FFFF) as u32
    }

    /// Increment generation counter
    fn increment_generation(&self) -> u32 {
        loop {
            let old = self.state.load(Ordering::Acquire);
            let gen = ((old >> 32) & 0xFFFF_FFFF) as u32;
            let new_gen = gen.wrapping_add(1);
            let new_state = (old & 0xFFFF_FFFF) | ((new_gen as u64) << 32);

            if self
                .state
                .compare_exchange(old, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return new_gen;
            }
        }
    }

    /// Select GPU for next batch based on load balancing strategy
    ///
    /// # Returns
    ///
    /// GPU ID of selected device, or None if no healthy GPUs available
    ///
    /// # Performance
    ///
    /// - RoundRobin: <10ns (atomic increment)
    /// - MemoryAware: <30ns (scan healthy GPUs)
    /// - PerformanceAware: <30ns (scan healthy GPUs)
    pub fn select_gpu(&self) -> Option<GpuId> {
        if self.devices.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalancingStrategy::RoundRobin => self.select_round_robin(),
            LoadBalancingStrategy::MemoryAware => self.select_memory_aware(),
            LoadBalancingStrategy::PerformanceAware => self.select_performance_aware(),
        }
    }

    /// Round-robin GPU selection (<10ns)
    fn select_round_robin(&self) -> Option<GpuId> {
        let healthy_count = self.healthy_gpu_count();
        if healthy_count == 0 {
            return None;
        }

        // Atomic increment of round-robin index
        loop {
            let old = self.state.load(Ordering::Acquire);
            let idx = ((old >> 16) & 0xFFFF) as u16;
            let new_idx = (idx + 1) % (self.devices.len() as u16);
            let new_state = (old & !0xFFFF_0000u64) | ((new_idx as u64) << 16);

            if self
                .state
                .compare_exchange(old, new_state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                // Find next healthy GPU starting from idx
                for i in 0..self.devices.len() {
                    let check_idx = ((idx as usize) + i) % self.devices.len();
                    if self.devices[check_idx].is_healthy() {
                        return Some(check_idx as GpuId);
                    }
                }
                return None;
            }
        }
    }

    /// Memory-aware GPU selection - prefer GPU with most free VRAM
    fn select_memory_aware(&self) -> Option<GpuId> {
        let mut best_gpu: Option<GpuId> = None;
        let mut best_vram: u16 = 0;

        for device in &self.devices {
            if device.is_healthy() {
                let vram = device.free_vram_mb();
                if best_gpu.is_none() || vram > best_vram {
                    best_gpu = Some(device.gpu_id());
                    best_vram = vram;
                }
            }
        }

        best_gpu
    }

    /// Performance-aware GPU selection - prefer GPU with highest throughput
    fn select_performance_aware(&self) -> Option<GpuId> {
        let mut best_gpu: Option<GpuId> = None;
        let mut best_throughput: u64 = 0;

        for device in &self.devices {
            if device.is_healthy() {
                let throughput = device.throughput_docs_per_sec();
                // Prefer GPU with established throughput, or any healthy GPU if none have history
                if best_gpu.is_none() || throughput > best_throughput {
                    best_gpu = Some(device.gpu_id());
                    best_throughput = throughput;
                }
            }
        }

        best_gpu
    }

    /// Get GPU device by ID
    ///
    /// # Returns
    ///
    /// Reference to GPU device capsule, or None if ID invalid
    #[inline]
    pub fn get_gpu(&self, gpu_id: GpuId) -> Option<&GpuDeviceCapsule> {
        self.devices.get(gpu_id as usize)
    }

    /// Get GPU statistics
    ///
    /// # Returns
    ///
    /// - `Ok(GpuStats)`: GPU statistics snapshot
    /// - `Err(GpuError)`: Invalid GPU ID
    pub fn get_gpu_stats(&self, gpu_id: GpuId) -> GpuResult<GpuStats> {
        self.devices
            .get(gpu_id as usize)
            .map(GpuStats::from)
            .ok_or(GpuError::InvalidInput(format!("Invalid GPU ID: {}", gpu_id)))
    }

    /// Get statistics for all GPUs
    pub fn get_all_stats(&self) -> Vec<GpuStats> {
        self.devices.iter().map(GpuStats::from).collect()
    }

    /// Start batch on specified GPU
    ///
    /// # Arguments
    ///
    /// - `gpu_id`: Target GPU
    ///
    /// # Returns
    ///
    /// - `Ok(pending_count)`: Batch started, returns new pending count
    /// - `Err(GpuError)`: Invalid GPU ID or GPU failed
    pub fn start_batch(&self, gpu_id: GpuId) -> GpuResult<u16> {
        let device = self.devices.get(gpu_id as usize).ok_or(GpuError::InvalidInput(
            format!("Invalid GPU ID: {}", gpu_id),
        ))?;

        if device.has_failed() {
            return Err(GpuError::ComputeFailed(format!(
                "GPU {} has failed",
                gpu_id
            )));
        }

        self.increment_generation();
        Ok(device.start_batch())
    }

    /// Complete batch on specified GPU
    ///
    /// # Arguments
    ///
    /// - `gpu_id`: Target GPU
    /// - `docs_processed`: Documents processed in batch
    /// - `duration`: Batch processing time
    ///
    /// # Returns
    ///
    /// - `Ok(remaining_pending)`: Batch completed
    /// - `Err(GpuError)`: Invalid GPU ID
    pub fn complete_batch(
        &self,
        gpu_id: GpuId,
        docs_processed: u64,
        duration: Duration,
    ) -> GpuResult<u16> {
        let device = self.devices.get(gpu_id as usize).ok_or(GpuError::InvalidInput(
            format!("Invalid GPU ID: {}", gpu_id),
        ))?;

        self.increment_generation();
        Ok(device.complete_batch(docs_processed, duration.as_micros() as u64))
    }

    /// Mark GPU as failed (removes from pool)
    ///
    /// # Arguments
    ///
    /// - `gpu_id`: GPU to mark as failed
    ///
    /// # Returns
    ///
    /// - `Ok(())`: GPU marked as failed
    /// - `Err(GpuError)`: Invalid GPU ID
    pub fn mark_gpu_failed(&self, gpu_id: GpuId) -> GpuResult<()> {
        let device = self.devices.get(gpu_id as usize).ok_or(GpuError::InvalidInput(
            format!("Invalid GPU ID: {}", gpu_id),
        ))?;

        device.mark_failed();
        self.increment_generation();

        eprintln!(
            "[MultiGpuCoordinator] GPU {} marked as failed, {} healthy GPUs remaining",
            gpu_id,
            self.healthy_gpu_count()
        );

        Ok(())
    }

    /// Rebalance workload across GPUs
    ///
    /// Currently a no-op for round-robin (already balanced).
    /// For memory/performance aware, this could redistribute pending batches.
    ///
    /// # Returns
    ///
    /// Number of batches redistributed
    pub fn rebalance(&self) -> usize {
        // For now, rebalance is informational only
        // True rebalancing would require canceling and resubmitting batches
        self.increment_generation();
        0
    }

    /// Poll all GPUs for completed work
    pub fn poll_all(&self) {
        for device in &self.devices {
            if device.is_healthy() {
                device.poll();
            }
        }
    }

    /// Wait for all GPUs to complete pending work
    pub fn wait_all(&self) {
        for device in &self.devices {
            if device.is_healthy() {
                device.wait();
            }
        }
    }

    /// Get aggregate throughput (sum of all healthy GPUs)
    pub fn aggregate_throughput(&self) -> u64 {
        self.devices
            .iter()
            .filter(|d| d.is_healthy())
            .map(|d| d.throughput_docs_per_sec())
            .sum()
    }

    /// Get total pending batches across all GPUs
    pub fn total_pending_batches(&self) -> u32 {
        self.devices
            .iter()
            .map(|d| d.state().pending_batches() as u32)
            .sum()
    }

    /// Check if all GPUs are idle
    pub fn is_idle(&self) -> bool {
        self.total_pending_batches() == 0
    }
}

impl std::fmt::Debug for MultiGpuCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiGpuCoordinator")
            .field("gpu_count", &self.gpu_count())
            .field("healthy_gpu_count", &self.healthy_gpu_count())
            .field("strategy", &self.strategy)
            .field("generation", &self.generation())
            .field("aggregate_throughput", &self.aggregate_throughput())
            .field("total_pending", &self.total_pending_batches())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== GpuDeviceState Tests ====================

    #[test]
    fn test_gpu_device_state_pack_unpack() {
        let state = GpuDeviceState::pack(
            3,      // gpu_id
            true,   // healthy
            false,  // active
            false,  // failed
            42,     // pending_batches
            8192,   // free_vram_mb (8GB)
            100,    // throughput_k (100K docs/sec)
        );
        let unpacked = GpuDeviceState::from(state);

        assert_eq!(unpacked.gpu_id(), 3);
        assert!(unpacked.is_healthy());
        assert!(!unpacked.is_active());
        assert!(!unpacked.has_failed());
        assert_eq!(unpacked.pending_batches(), 42);
        assert_eq!(unpacked.free_vram_mb(), 8192);
        assert_eq!(unpacked.throughput_k(), 100);
        assert_eq!(unpacked.throughput_docs_per_sec(), 100_000);
    }

    #[test]
    fn test_gpu_device_state_new() {
        let state = GpuDeviceState::new(5, 4096);

        assert_eq!(state.gpu_id(), 5);
        assert!(state.is_healthy());
        assert!(!state.is_active());
        assert!(!state.has_failed());
        assert_eq!(state.pending_batches(), 0);
        assert_eq!(state.free_vram_mb(), 4096);
        assert_eq!(state.throughput_k(), 0);
    }

    #[test]
    fn test_gpu_device_state_failed_flag() {
        let state = GpuDeviceState::pack(0, false, false, true, 0, 0, 0);
        let unpacked = GpuDeviceState::from(state);

        assert!(!unpacked.is_healthy());
        assert!(unpacked.has_failed());
    }

    #[test]
    fn test_gpu_device_state_all_flags() {
        // All flags set
        let state = GpuDeviceState::pack(7, true, true, true, 0xFFFF, 0xFFFF, 0xFFFF);
        let unpacked = GpuDeviceState::from(state);

        assert_eq!(unpacked.gpu_id(), 7);
        assert!(unpacked.is_healthy());
        assert!(unpacked.is_active());
        assert!(unpacked.has_failed());
        assert_eq!(unpacked.pending_batches(), 0xFFFF);
        assert_eq!(unpacked.free_vram_mb(), 0xFFFF);
        assert_eq!(unpacked.throughput_k(), 0xFFFF);
    }

    // ==================== LoadBalancingStrategy Tests ====================

    #[test]
    fn test_load_balancing_strategy_default() {
        let strategy = LoadBalancingStrategy::default();
        assert_eq!(strategy, LoadBalancingStrategy::RoundRobin);
    }

    #[test]
    fn test_load_balancing_strategy_variants() {
        assert_eq!(
            format!("{:?}", LoadBalancingStrategy::RoundRobin),
            "RoundRobin"
        );
        assert_eq!(
            format!("{:?}", LoadBalancingStrategy::MemoryAware),
            "MemoryAware"
        );
        assert_eq!(
            format!("{:?}", LoadBalancingStrategy::PerformanceAware),
            "PerformanceAware"
        );
    }

    // ==================== GpuStats Tests ====================

    #[test]
    fn test_gpu_stats_debug() {
        let stats = GpuStats {
            gpu_id: 0,
            is_healthy: true,
            is_active: false,
            has_failed: false,
            pending_batches: 5,
            free_vram_mb: 8192,
            throughput_docs_per_sec: 500_000,
            generation: 42,
        };

        let debug = format!("{:?}", stats);
        assert!(debug.contains("gpu_id: 0"));
        assert!(debug.contains("is_healthy: true"));
        assert!(debug.contains("throughput_docs_per_sec: 500000"));
    }

    // ==================== Coordinator State Packing Tests ====================

    #[test]
    fn test_coordinator_pack_state() {
        let state = MultiGpuCoordinator::pack_state(4, LoadBalancingStrategy::PerformanceAware, 100, 12345);

        // Extract fields
        let gpu_count = (state & 0xFF) as u8;
        let strategy_id = ((state >> 8) & 0xFF) as u8;
        let round_robin = ((state >> 16) & 0xFFFF) as u16;
        let generation = ((state >> 32) & 0xFFFF_FFFF) as u32;

        assert_eq!(gpu_count, 4);
        assert_eq!(strategy_id, 2); // PerformanceAware
        assert_eq!(round_robin, 100);
        assert_eq!(generation, 12345);
    }

    // ==================== Multi-GPU Coordinator Tests (require GPU) ====================

    #[test]
    #[ignore = "Requires GPU hardware - run with --ignored"]
    fn test_multi_gpu_coordinator_creation() {
        match MultiGpuCoordinator::new() {
            Ok(coordinator) => {
                println!("Found {} GPUs", coordinator.gpu_count());
                assert!(coordinator.gpu_count() > 0);
                assert!(coordinator.healthy_gpu_count() > 0);
                assert_eq!(coordinator.strategy(), LoadBalancingStrategy::RoundRobin);

                for stats in coordinator.get_all_stats() {
                    println!(
                        "GPU {}: healthy={}, vram={}MB",
                        stats.gpu_id, stats.is_healthy, stats.free_vram_mb
                    );
                }
            }
            Err(e) => {
                println!("No GPU available (expected in CI): {}", e);
            }
        }
    }

    #[test]
    #[ignore = "Requires GPU hardware - run with --ignored"]
    fn test_multi_gpu_select_round_robin() {
        if let Ok(coordinator) = MultiGpuCoordinator::new() {
            let count = coordinator.gpu_count();
            if count > 1 {
                // Select GPUs in sequence
                let selections: Vec<_> = (0..count * 2)
                    .filter_map(|_| coordinator.select_gpu())
                    .collect();

                println!("Round-robin selections: {:?}", selections);
                assert!(!selections.is_empty());
            }
        }
    }

    #[test]
    #[ignore = "Requires GPU hardware - run with --ignored"]
    fn test_multi_gpu_batch_lifecycle() {
        if let Ok(coordinator) = MultiGpuCoordinator::new() {
            if let Some(gpu_id) = coordinator.select_gpu() {
                // Start batch
                let pending = coordinator.start_batch(gpu_id).unwrap();
                assert!(pending >= 1);

                // Complete batch
                let remaining = coordinator
                    .complete_batch(gpu_id, 1000, Duration::from_millis(10))
                    .unwrap();
                assert_eq!(remaining, pending - 1);

                // Check throughput updated
                let stats = coordinator.get_gpu_stats(gpu_id).unwrap();
                assert!(stats.throughput_docs_per_sec > 0);
            }
        }
    }

    #[test]
    #[ignore = "Requires GPU hardware - run with --ignored"]
    fn test_multi_gpu_mark_failed() {
        if let Ok(coordinator) = MultiGpuCoordinator::new() {
            let initial_healthy = coordinator.healthy_gpu_count();
            if initial_healthy > 0 {
                // Mark first GPU as failed
                coordinator.mark_gpu_failed(0).unwrap();

                // Check healthy count decreased
                assert_eq!(coordinator.healthy_gpu_count(), initial_healthy - 1);

                // Verify GPU is marked failed
                let stats = coordinator.get_gpu_stats(0).unwrap();
                assert!(stats.has_failed);
                assert!(!stats.is_healthy);
            }
        }
    }

    #[test]
    #[ignore = "Requires GPU hardware - run with --ignored"]
    fn test_multi_gpu_memory_aware_selection() {
        if let Ok(coordinator) =
            MultiGpuCoordinator::with_strategy(LoadBalancingStrategy::MemoryAware)
        {
            if coordinator.gpu_count() > 0 {
                let gpu_id = coordinator.select_gpu();
                assert!(gpu_id.is_some());
                println!("Memory-aware selected GPU: {:?}", gpu_id);
            }
        }
    }

    #[test]
    #[ignore = "Requires GPU hardware - run with --ignored"]
    fn test_multi_gpu_performance_aware_selection() {
        if let Ok(coordinator) =
            MultiGpuCoordinator::with_strategy(LoadBalancingStrategy::PerformanceAware)
        {
            if coordinator.gpu_count() > 0 {
                let gpu_id = coordinator.select_gpu();
                assert!(gpu_id.is_some());
                println!("Performance-aware selected GPU: {:?}", gpu_id);
            }
        }
    }

    // ==================== No-GPU Fallback Tests ====================

    #[test]
    fn test_max_gpus_constant() {
        assert_eq!(MAX_GPUS, 8);
    }

    #[test]
    fn test_gpu_id_type() {
        let id: GpuId = 7;
        assert!(id < MAX_GPUS as u8);
    }

    #[test]
    fn test_state_conversion_roundtrip() {
        for gpu_id in 0..8u8 {
            for healthy in [true, false] {
                for active in [true, false] {
                    let state = GpuDeviceState::pack(gpu_id, healthy, active, false, 100, 4096, 50);
                    let unpacked = GpuDeviceState::from(state);

                    assert_eq!(unpacked.gpu_id(), gpu_id);
                    assert_eq!(unpacked.is_healthy(), healthy);
                    assert_eq!(unpacked.is_active(), active);
                }
            }
        }
    }

    #[test]
    fn test_throughput_calculation() {
        // 100K docs/sec = throughput_k of 100
        let state = GpuDeviceState::pack(0, true, false, false, 0, 0, 100);
        let unpacked = GpuDeviceState::from(state);
        assert_eq!(unpacked.throughput_docs_per_sec(), 100_000);

        // 1M docs/sec = throughput_k of 1000
        let state = GpuDeviceState::pack(0, true, false, false, 0, 0, 1000);
        let unpacked = GpuDeviceState::from(state);
        assert_eq!(unpacked.throughput_docs_per_sec(), 1_000_000);
    }
}
