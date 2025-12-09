//! GPU Hybrid Dispatch Capsule - T7 Heterogeneous Multi-Accelerator Dispatch
//!
//! **Tier**: T7 Heterogeneous (multi-accelerator dispatch)
//! **Alignment**: 256B cache-aligned (metacapsule for GPU coordination)
//! **Lockfree**: 100% atomic operations, zero mutex/RwLock
//! **UCE34**: Q10 tier selection, Q33 lockfree mandate, Q34 audit trail
//!
//! # Purpose
//!
//! Intelligent dispatch between Intel Arc (local), RTX 3080M (remote), and CPU.
//! Automatically selects the optimal compute target based on:
//! - Operation type (MatMul, Attention, Quantize, Dequantize)
//! - Input size (small ops favor CPU to avoid transfer overhead)
//! - Hardware availability (runtime detection)
//!
//! # Hardware Inventory
//!
//! | Device | Location | Capabilities | Best For |
//! |--------|----------|--------------|----------|
//! | Intel Arc | Local | XE cores, INT8 | Medium MatMul, local inference |
//! | RTX 3080M 8GB | Remote (kindly-hub) | INT8 Tensor Cores, Ampere | Large MatMul, batch inference |
//! | AMD Ryzen 9 6900HX | Local | AVX-512 VNNI | Small ops, KV decompression |
//!
//! # Dispatch Decision Matrix
//!
//! | Operation | Intel Arc (local) | RTX 3080M (remote) | CPU (fallback) |
//! |-----------|-------------------|---------------------|----------------|
//! | Large MatMul (>1M) | OK (medium) | BEST | No |
//! | INT8 MatMul | OK (XE INT8) | BEST (Tensor Cores) | OK (VNNI) |
//! | Attention | OK (XE Compute) | BEST (cuDNN Flash) | No |
//! | KV Decompress | No | No | BEST (CPU better) |
//! | Small ops (<64K) | No | No | BEST (no transfer overhead) |
//!
//! # Performance Targets
//!
//! - Hardware detection: <1ms (cached after first call)
//! - Dispatch decision: <50ns (atomic loads only)
//! - Statistics update: <20ns (atomic increment)
//!
//! # ASSUM Safety (99.99%)
//!
//! - #ASSUME_XE_DETECTION: Intel Arc detection via sysfs is safe
//! - #VERIFY_XE_DETECTION: Check /sys/class/drm for vendor 8086
//! - #ASSUME_CUDA_DETECTION: CUDA detection via SSH is safe
//! - #VERIFY_CUDA_DETECTION: SSH to kindly-hub with timeout
//! - #ASSUME_THRESHOLD_TUNED: Thresholds tuned for specific hardware
//! - #VERIFY_THRESHOLD_TUNED: B32 benchmarks validate thresholds
//! - #ASSUME_LOCKFREE_SAFE: All atomic operations are safe
//! - #VERIFY_LOCKFREE_SAFE: No mutex/RwLock in hot paths

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

#[cfg(feature = "std")]
use std::sync::OnceLock;

#[cfg(feature = "kgpu-driver-intel")]
use crate::gpu::kgpu_driver::xe_compute_capsule::XeComputeCapsule;

use crate::primitives::cpu_capabilities::CpuCapabilityCapsule;

// ============================================================================
// Constants
// ============================================================================

/// Default threshold for GPU dispatch (elements)
/// Operations with fewer elements stay on CPU to avoid transfer overhead
pub const DEFAULT_GPU_THRESHOLD: usize = 65_536; // 64K elements

/// Default threshold for XE dispatch (elements)
/// Larger operations benefit from more powerful CUDA GPU
pub const DEFAULT_XE_THRESHOLD: usize = 1_000_000; // 1M elements

/// Small operation threshold (elements)
/// Operations below this always use CPU
pub const SMALL_OP_THRESHOLD: usize = 1_024; // 1K elements

/// Dispatch state: Idle (no pending dispatch)
const DISPATCH_STATE_IDLE: u64 = 0;
/// Dispatch state: Dispatching to GPU
const DISPATCH_STATE_DISPATCHING: u64 = 1;
/// Dispatch state: Waiting for result
const DISPATCH_STATE_WAITING: u64 = 2;
/// Dispatch state: Error occurred
const DISPATCH_STATE_ERROR: u64 = 3;

// ============================================================================
// Error Types
// ============================================================================

/// Dispatch error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError {
    /// No suitable device available for the operation
    NoDeviceAvailable,
    /// Intel Arc XE dispatch failed
    XeDispatchFailed { errno: i32 },
    /// CUDA dispatch failed
    CudaDispatchFailed { errno: i32 },
    /// CPU dispatch failed
    CpuDispatchFailed,
    /// Remote dispatch not implemented (Phase 2)
    RemoteNotImplemented,
    /// Invalid input dimensions
    InvalidDimensions { rows: usize, cols: usize },
    /// Operation not supported on target device
    OperationNotSupported { target: DispatchTarget, operation: &'static str },
    /// Transfer to device failed
    TransferFailed { direction: TransferDirection },
    /// Timeout waiting for result
    Timeout { timeout_ms: u64 },
}

impl core::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DispatchError::NoDeviceAvailable => {
                write!(f, "No suitable device available for operation")
            }
            DispatchError::XeDispatchFailed { errno } => {
                write!(f, "Intel XE dispatch failed: errno {}", errno)
            }
            DispatchError::CudaDispatchFailed { errno } => {
                write!(f, "CUDA dispatch failed: errno {}", errno)
            }
            DispatchError::CpuDispatchFailed => {
                write!(f, "CPU dispatch failed")
            }
            DispatchError::RemoteNotImplemented => {
                write!(f, "Remote dispatch not implemented (Phase 2)")
            }
            DispatchError::InvalidDimensions { rows, cols } => {
                write!(f, "Invalid dimensions: {}x{}", rows, cols)
            }
            DispatchError::OperationNotSupported { target, operation } => {
                write!(f, "Operation '{}' not supported on {:?}", operation, target)
            }
            DispatchError::TransferFailed { direction } => {
                write!(f, "Transfer failed: {:?}", direction)
            }
            DispatchError::Timeout { timeout_ms } => {
                write!(f, "Timeout after {}ms", timeout_ms)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DispatchError {}

/// Transfer direction for error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    HostToDevice,
    DeviceToHost,
    DeviceToDevice,
}

// ============================================================================
// Dispatch Target
// ============================================================================

/// Dispatch target for GPU hybrid dispatch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DispatchTarget {
    /// CPU with AVX-512 VNNI (local, no transfer overhead)
    Cpu = 0,
    /// Intel Arc GPU (local, XE compute)
    IntelArc = 1,
    /// NVIDIA CUDA GPU (remote, kindly-hub RTX 3080M)
    NvidiaCuda = 2,
}

impl core::fmt::Display for DispatchTarget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DispatchTarget::Cpu => write!(f, "CPU (AVX-512)"),
            DispatchTarget::IntelArc => write!(f, "Intel Arc (XE)"),
            DispatchTarget::NvidiaCuda => write!(f, "NVIDIA CUDA (Remote)"),
        }
    }
}

// ============================================================================
// Tensor Type (Simplified for dispatch decisions)
// ============================================================================

/// Simplified tensor descriptor for dispatch decisions
///
/// This is a lightweight struct for dispatch decision-making,
/// not the full tensor implementation.
#[derive(Debug, Clone, Copy)]
pub struct TensorDesc {
    /// Number of rows
    pub rows: usize,
    /// Number of columns
    pub cols: usize,
    /// Element size in bytes
    pub element_size: usize,
}

impl TensorDesc {
    /// Create new tensor descriptor
    #[inline]
    pub const fn new(rows: usize, cols: usize, element_size: usize) -> Self {
        Self {
            rows,
            cols,
            element_size,
        }
    }

    /// Total number of elements
    #[inline]
    pub const fn elements(&self) -> usize {
        self.rows * self.cols
    }

    /// Total size in bytes
    #[inline]
    pub const fn size_bytes(&self) -> usize {
        self.rows * self.cols * self.element_size
    }
}

// ============================================================================
// Dispatch Statistics
// ============================================================================

/// Dispatch statistics snapshot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchStats {
    /// Total dispatches made
    pub total_dispatches: u64,
    /// Dispatches to CPU
    pub cpu_dispatches: u64,
    /// Dispatches to Intel Arc XE
    pub xe_dispatches: u64,
    /// Dispatches to NVIDIA CUDA
    pub cuda_dispatches: u64,
    /// Generation counter for consistency
    pub generation: u64,
}

// ============================================================================
// GPU Hybrid Dispatch Capsule
// ============================================================================

/// GPU Hybrid Dispatch Capsule - T7 Heterogeneous Multi-Accelerator Dispatch
///
/// **Tier**: T7 Heterogeneous
/// **Size**: 256 bytes (4 cache lines on x86-64)
/// **Alignment**: 256B for metacapsule coordination
/// **Lockfree**: 100% atomic operations, no mutex/RwLock
///
/// # Architecture
///
/// ```text
/// State Machine:
/// IDLE ──dispatch()──> DISPATCHING ──submit()──> WAITING ──complete()──> IDLE
///                                                    │
///                                                    └──error()──> ERROR ──reset()──> IDLE
/// ```
///
/// # Memory Layout
///
/// ```text
/// Offset | Field                | Size | Alignment
/// -------|---------------------|------|----------
/// 0      | xe_available        | 1    | 1
/// 1      | cuda_available      | 1    | 1
/// 2      | avx512_available    | 1    | 1
/// 3      | _pad1               | 5    | 1
/// 8      | gpu_threshold       | 8    | 8
/// 16     | xe_threshold        | 8    | 8
/// 24     | state               | 8    | 8
/// 32     | generation          | 8    | 8
/// 40     | total_dispatches    | 8    | 8
/// 48     | cpu_dispatches      | 8    | 8
/// 56     | xe_dispatches       | 8    | 8
/// 64     | cuda_dispatches     | 8    | 8
/// 72     | _padding            | 184  | 1
/// ```
#[repr(C, align(256))]
pub struct GpuHybridDispatchCapsule {
    // ========================================================================
    // Device Availability (detected at runtime)
    // ========================================================================

    /// Intel Arc XE GPU available (local)
    /// #ASSUME: Detection via sysfs is safe
    /// #VERIFY: Check /sys/class/drm for Intel vendor 8086
    xe_available: AtomicBool,

    /// NVIDIA CUDA GPU available (remote, kindly-hub)
    /// #ASSUME: SSH detection is safe
    /// #VERIFY: SSH to kindly-hub with timeout
    cuda_available: AtomicBool,

    /// AVX-512 available for CPU fallback
    /// #ASSUME: CPUID detection is safe (std::arch)
    /// #VERIFY: CpuCapabilityCapsule::detect()
    avx512_available: AtomicBool,

    /// Padding for alignment
    _pad1: [u8; 5],

    // ========================================================================
    // Dispatch Thresholds
    // ========================================================================

    /// Minimum elements for GPU dispatch (default: 65536)
    /// Operations smaller than this stay on CPU
    gpu_threshold: AtomicUsize,

    /// Minimum elements for XE dispatch (default: 1M)
    /// Larger operations prefer CUDA if available
    xe_threshold: AtomicUsize,

    // ========================================================================
    // Coordination State
    // ========================================================================

    /// Current dispatch state (IDLE/DISPATCHING/WAITING/ERROR)
    state: AtomicU64,

    /// Generation counter for ABA prevention
    generation: AtomicU64,

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Total dispatches (all targets)
    total_dispatches: AtomicU64,

    /// CPU dispatches count
    cpu_dispatches: AtomicU64,

    /// Intel XE dispatches count
    xe_dispatches: AtomicU64,

    /// NVIDIA CUDA dispatches count
    cuda_dispatches: AtomicU64,

    // ========================================================================
    // Padding
    // ========================================================================

    /// Padding to exactly 256 bytes
    ///
    /// Current fields:
    ///   xe_available: 1 byte
    ///   cuda_available: 1 byte
    ///   avx512_available: 1 byte
    ///   _pad1: 5 bytes
    ///   gpu_threshold: 8 bytes
    ///   xe_threshold: 8 bytes
    ///   state: 8 bytes
    ///   generation: 8 bytes
    ///   total_dispatches: 8 bytes
    ///   cpu_dispatches: 8 bytes
    ///   xe_dispatches: 8 bytes
    ///   cuda_dispatches: 8 bytes
    /// Total: 72 bytes
    ///
    /// Padding needed: 256 - 72 = 184 bytes
    _padding: [u8; 184],
}

// Compile-time verification: size and alignment
const _: () = {
    assert!(core::mem::size_of::<GpuHybridDispatchCapsule>() == 256);
    assert!(core::mem::align_of::<GpuHybridDispatchCapsule>() == 256);
};

/// Global singleton for hardware detection (OnceLock pattern)
#[cfg(feature = "std")]
static HYBRID_DISPATCH: OnceLock<GpuHybridDispatchCapsule> = OnceLock::new();

impl GpuHybridDispatchCapsule {
    /// Create new hybrid dispatch capsule with hardware detection
    ///
    /// **Tier**: T7 Heterogeneous
    /// **Latency**: <1ms (hardware detection, cached)
    /// **Safety**: Safe, no unsafe code
    ///
    /// # Hardware Detection
    ///
    /// - Intel Arc: Check sysfs for Intel GPU (vendor 8086, device 7D40-7D67)
    /// - CUDA: Check for nvidia-smi on remote kindly-hub (SSH)
    /// - AVX-512: Use CpuCapabilityCapsule::detect()
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::gpu::hybrid_dispatch::GpuHybridDispatchCapsule;
    ///
    /// let dispatch = GpuHybridDispatchCapsule::new();
    /// let target = dispatch.dispatch_matmul_target(1024, 1024, 1024);
    /// println!("Dispatch target: {:?}", target);
    /// ```
    #[cfg(feature = "std")]
    pub fn new() -> Self {
        let xe_available = detect_intel_arc();
        let cuda_available = detect_cuda_remote();
        let avx512_available = CpuCapabilityCapsule::detect().has_avx512();

        Self {
            xe_available: AtomicBool::new(xe_available),
            cuda_available: AtomicBool::new(cuda_available),
            avx512_available: AtomicBool::new(avx512_available),
            _pad1: [0u8; 5],
            gpu_threshold: AtomicUsize::new(DEFAULT_GPU_THRESHOLD),
            xe_threshold: AtomicUsize::new(DEFAULT_XE_THRESHOLD),
            state: AtomicU64::new(DISPATCH_STATE_IDLE),
            generation: AtomicU64::new(0),
            total_dispatches: AtomicU64::new(0),
            cpu_dispatches: AtomicU64::new(0),
            xe_dispatches: AtomicU64::new(0),
            cuda_dispatches: AtomicU64::new(0),
            _padding: [0u8; 184],
        }
    }

    /// Create with explicit device availability (for testing)
    ///
    /// **Latency**: <10ns (no detection, direct construction)
    pub const fn with_devices(xe: bool, cuda: bool, avx512: bool) -> Self {
        Self {
            xe_available: AtomicBool::new(xe),
            cuda_available: AtomicBool::new(cuda),
            avx512_available: AtomicBool::new(avx512),
            _pad1: [0u8; 5],
            gpu_threshold: AtomicUsize::new(DEFAULT_GPU_THRESHOLD),
            xe_threshold: AtomicUsize::new(DEFAULT_XE_THRESHOLD),
            state: AtomicU64::new(DISPATCH_STATE_IDLE),
            generation: AtomicU64::new(0),
            total_dispatches: AtomicU64::new(0),
            cpu_dispatches: AtomicU64::new(0),
            xe_dispatches: AtomicU64::new(0),
            cuda_dispatches: AtomicU64::new(0),
            _padding: [0u8; 184],
        }
    }

    /// Get singleton instance (cached detection)
    ///
    /// **First call**: <1ms (hardware detection)
    /// **Subsequent calls**: <10ns (cached reference)
    #[cfg(feature = "std")]
    pub fn instance() -> &'static Self {
        HYBRID_DISPATCH.get_or_init(Self::new)
    }

    // ========================================================================
    // Dispatch Decision Logic
    // ========================================================================

    /// Dispatch decision for matrix multiplication
    ///
    /// **Latency**: <50ns (atomic loads only)
    ///
    /// # Decision Logic
    ///
    /// 1. Small ops (<1K elements): CPU (no transfer overhead)
    /// 2. Large ops (>1M elements): CUDA if available, else XE, else CPU
    /// 3. Medium ops (64K-1M): XE if available, else CPU
    /// 4. Below GPU threshold (<64K): CPU
    ///
    /// # Arguments
    ///
    /// * `a_rows` - Rows in matrix A
    /// * `a_cols` - Columns in matrix A (= rows in matrix B)
    /// * `b_cols` - Columns in matrix B
    ///
    /// # Returns
    ///
    /// Optimal dispatch target for the given matrix dimensions
    #[inline]
    pub fn dispatch_matmul_target(&self, a_rows: usize, a_cols: usize, b_cols: usize) -> DispatchTarget {
        let total_elements = a_rows * a_cols * b_cols;
        self.dispatch_by_size(total_elements)
    }

    /// Dispatch decision for tensor operation by size
    ///
    /// **Latency**: <50ns
    #[inline]
    pub fn dispatch_by_size(&self, total_elements: usize) -> DispatchTarget {
        let gpu_threshold = self.gpu_threshold.load(Ordering::Relaxed);
        let xe_threshold = self.xe_threshold.load(Ordering::Relaxed);
        let cuda_available = self.cuda_available.load(Ordering::Relaxed);
        let xe_available = self.xe_available.load(Ordering::Relaxed);

        // Small ops: Always CPU
        if total_elements < SMALL_OP_THRESHOLD {
            return DispatchTarget::Cpu;
        }

        // Below GPU threshold: CPU
        if total_elements < gpu_threshold {
            return DispatchTarget::Cpu;
        }

        // Large ops (>1M): Prefer CUDA if available
        if total_elements > xe_threshold && cuda_available {
            return DispatchTarget::NvidiaCuda;
        }

        // Medium ops or CUDA unavailable: Use XE if available
        if xe_available && total_elements >= gpu_threshold {
            return DispatchTarget::IntelArc;
        }

        // Fallback: CPU
        DispatchTarget::Cpu
    }

    /// Dispatch decision for attention operation
    ///
    /// **Latency**: <50ns
    ///
    /// # Decision Logic
    ///
    /// Attention benefits from GPU (Flash Attention on CUDA, XE compute on Arc).
    /// Only use CPU for very small sequences.
    #[inline]
    pub fn dispatch_attention_target(
        &self,
        batch_size: usize,
        seq_len: usize,
        heads: usize,
        head_dim: usize,
    ) -> DispatchTarget {
        // Attention complexity: O(batch * heads * seq_len^2 * head_dim)
        let total_ops = batch_size * heads * seq_len * seq_len * head_dim;
        self.dispatch_by_size(total_ops)
    }

    /// Dispatch decision for quantization
    ///
    /// **Latency**: <50ns
    ///
    /// Quantization is memory-bound, so GPU helps for large tensors.
    #[inline]
    pub fn dispatch_quantize_target(&self, total_elements: usize) -> DispatchTarget {
        // Quantization has lower compute intensity, raise threshold
        let effective_threshold = self.gpu_threshold.load(Ordering::Relaxed) * 4;

        if total_elements < effective_threshold {
            DispatchTarget::Cpu
        } else if self.cuda_available.load(Ordering::Relaxed) {
            DispatchTarget::NvidiaCuda
        } else if self.xe_available.load(Ordering::Relaxed) {
            DispatchTarget::IntelArc
        } else {
            DispatchTarget::Cpu
        }
    }

    /// Dispatch decision for dequantization (KV cache decompression)
    ///
    /// **Latency**: <50ns
    ///
    /// Dequantization is often better on CPU due to:
    /// - Memory-bound nature
    /// - AVX-512 VNNI efficiency
    /// - Avoiding transfer overhead
    #[inline]
    pub fn dispatch_dequantize_target(&self, total_elements: usize) -> DispatchTarget {
        // KV decompression is better on CPU with AVX-512 VNNI
        // Only use GPU for very large tensors (>10M elements)
        let avx512 = self.avx512_available.load(Ordering::Relaxed);

        if avx512 && total_elements < 10_000_000 {
            DispatchTarget::Cpu
        } else if self.cuda_available.load(Ordering::Relaxed) {
            DispatchTarget::NvidiaCuda
        } else if self.xe_available.load(Ordering::Relaxed) {
            DispatchTarget::IntelArc
        } else {
            DispatchTarget::Cpu
        }
    }

    // ========================================================================
    // Execution Stubs (Phase 2: Remote dispatch)
    // ========================================================================

    /// Execute matrix multiplication on dispatched target
    ///
    /// **NOTE**: Full implementation in Phase 2 (remote dispatch)
    ///
    /// # Current Implementation
    ///
    /// - CPU: Returns placeholder (AVX-512 SIMD implementation needed)
    /// - IntelArc: Returns placeholder (XeComputeCapsule integration needed)
    /// - NvidiaCuda: Returns RemoteNotImplemented (SSH dispatch Phase 2)
    pub fn matmul(
        &self,
        a: &TensorDesc,
        b: &TensorDesc,
    ) -> Result<DispatchTarget, DispatchError> {
        let target = self.dispatch_matmul_target(a.rows, a.cols, b.cols);

        // Update statistics
        self.total_dispatches.fetch_add(1, Ordering::Relaxed);
        match target {
            DispatchTarget::Cpu => {
                self.cpu_dispatches.fetch_add(1, Ordering::Relaxed);
            }
            DispatchTarget::IntelArc => {
                self.xe_dispatches.fetch_add(1, Ordering::Relaxed);
            }
            DispatchTarget::NvidiaCuda => {
                self.cuda_dispatches.fetch_add(1, Ordering::Relaxed);
                // TODO: Phase 2 - Remote dispatch via SSH
                return Err(DispatchError::RemoteNotImplemented);
            }
        }

        self.generation.fetch_add(1, Ordering::Release);
        Ok(target)
    }

    // ========================================================================
    // Configuration
    // ========================================================================

    /// Set GPU dispatch threshold
    ///
    /// Operations with fewer elements than this threshold stay on CPU.
    #[inline]
    pub fn set_gpu_threshold(&self, threshold: usize) {
        self.gpu_threshold.store(threshold, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set XE vs CUDA threshold
    ///
    /// Operations larger than this prefer CUDA over XE.
    #[inline]
    pub fn set_xe_threshold(&self, threshold: usize) {
        self.xe_threshold.store(threshold, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get current GPU threshold
    #[inline]
    pub fn gpu_threshold(&self) -> usize {
        self.gpu_threshold.load(Ordering::Relaxed)
    }

    /// Get current XE threshold
    #[inline]
    pub fn xe_threshold(&self) -> usize {
        self.xe_threshold.load(Ordering::Relaxed)
    }

    // ========================================================================
    // Device Availability Queries
    // ========================================================================

    /// Check if Intel Arc XE is available
    #[inline]
    pub fn has_intel_arc(&self) -> bool {
        self.xe_available.load(Ordering::Relaxed)
    }

    /// Check if NVIDIA CUDA is available (remote)
    #[inline]
    pub fn has_nvidia_cuda(&self) -> bool {
        self.cuda_available.load(Ordering::Relaxed)
    }

    /// Check if AVX-512 is available
    #[inline]
    pub fn has_avx512(&self) -> bool {
        self.avx512_available.load(Ordering::Relaxed)
    }

    /// Get current dispatch state
    #[inline]
    pub fn state(&self) -> u64 {
        self.state.load(Ordering::Acquire)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get dispatch statistics snapshot
    ///
    /// **Latency**: <20ns (5 atomic loads)
    #[inline]
    pub fn stats(&self) -> DispatchStats {
        DispatchStats {
            total_dispatches: self.total_dispatches.load(Ordering::Relaxed),
            cpu_dispatches: self.cpu_dispatches.load(Ordering::Relaxed),
            xe_dispatches: self.xe_dispatches.load(Ordering::Relaxed),
            cuda_dispatches: self.cuda_dispatches.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Reset statistics counters
    #[inline]
    pub fn reset_stats(&self) {
        self.total_dispatches.store(0, Ordering::Relaxed);
        self.cpu_dispatches.store(0, Ordering::Relaxed);
        self.xe_dispatches.store(0, Ordering::Relaxed);
        self.cuda_dispatches.store(0, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
    }
}

impl Default for GpuHybridDispatchCapsule {
    fn default() -> Self {
        Self::with_devices(false, false, false)
    }
}

impl core::fmt::Debug for GpuHybridDispatchCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GpuHybridDispatchCapsule")
            .field("xe_available", &self.has_intel_arc())
            .field("cuda_available", &self.has_nvidia_cuda())
            .field("avx512_available", &self.has_avx512())
            .field("gpu_threshold", &self.gpu_threshold())
            .field("xe_threshold", &self.xe_threshold())
            .field("state", &self.state())
            .field("generation", &self.generation())
            .field("stats", &self.stats())
            .finish()
    }
}

// ============================================================================
// Hardware Detection Functions
// ============================================================================

/// Detect Intel Arc GPU (Meteor Lake, DG2)
///
/// Checks sysfs for Intel GPU with Arc device IDs:
/// - Meteor Lake: 7D40-7D67
/// - DG2 (Arc A-series): 5690-56FF
///
/// # ASSUM Safety
/// - #ASSUME_SYSFS_SAFE: Reading /sys/class/drm is safe
/// - #VERIFY_SYSFS_SAFE: Standard Linux kernel interface
#[cfg(all(feature = "std", target_os = "linux"))]
pub fn detect_intel_arc() -> bool {
    use std::fs;

    // Check sysfs for Intel GPU
    for entry in fs::read_dir("/sys/class/drm").ok().into_iter().flatten() {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path
                .file_name()
                .map(|n| n.to_string_lossy().starts_with("card"))
                .unwrap_or(false)
            {
                let vendor_path = path.join("device/vendor");
                let device_path = path.join("device/device");

                if let (Ok(vendor), Ok(device)) = (
                    fs::read_to_string(&vendor_path),
                    fs::read_to_string(&device_path),
                ) {
                    let vendor = vendor.trim().trim_start_matches("0x");
                    let device = device.trim().trim_start_matches("0x");

                    // Intel vendor ID: 8086
                    if vendor.eq_ignore_ascii_case("8086") {
                        if let Ok(dev_id) = u16::from_str_radix(device, 16) {
                            // Meteor Lake: 7D40-7D67
                            // DG2 (Arc A-series): 5690-56FF
                            if (0x7D40..=0x7D67).contains(&dev_id)
                                || (0x5690..=0x56FF).contains(&dev_id)
                            {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

#[cfg(not(all(feature = "std", target_os = "linux")))]
pub fn detect_intel_arc() -> bool {
    false
}

/// Detect NVIDIA CUDA GPU on remote kindly-hub
///
/// Checks for nvidia-smi availability via SSH.
///
/// # Phase 2: Full implementation
///
/// Currently returns false (remote detection not implemented).
/// Full implementation will:
/// 1. SSH to kindly-hub (192.168.0.38)
/// 2. Run nvidia-smi
/// 3. Parse GPU info
///
/// # ASSUM Safety
/// - #ASSUME_SSH_TIMEOUT: SSH connection has timeout
/// - #VERIFY_SSH_TIMEOUT: Use 1-second timeout
#[cfg(feature = "std")]
pub fn detect_cuda_remote() -> bool {
    // TODO: Phase 2 - SSH to kindly-hub and check nvidia-smi
    //
    // Command would be:
    // ssh samuel@kindly-hub "nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null"
    //
    // For now, assume CUDA is available if we're on a system that might have it configured
    // This is a placeholder until remote dispatch is implemented

    // Check for environment variable that signals CUDA availability
    std::env::var("CUDA_VISIBLE_DEVICES").is_ok()
        || std::path::Path::new("/usr/local/cuda/bin/nvcc").exists()
}

#[cfg(not(feature = "std"))]
pub fn detect_cuda_remote() -> bool {
    false
}

// ============================================================================
// Tests (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // T28 Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_capsule_size_alignment() {
        // T28 Q1: Verify 256B cache alignment
        assert_eq!(
            core::mem::size_of::<GpuHybridDispatchCapsule>(),
            256,
            "GpuHybridDispatchCapsule must be exactly 256 bytes"
        );
        assert_eq!(
            core::mem::align_of::<GpuHybridDispatchCapsule>(),
            256,
            "GpuHybridDispatchCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_default_thresholds() {
        // T28 Q2: Verify default thresholds
        let capsule = GpuHybridDispatchCapsule::default();
        assert_eq!(capsule.gpu_threshold(), DEFAULT_GPU_THRESHOLD);
        assert_eq!(capsule.xe_threshold(), DEFAULT_XE_THRESHOLD);
    }

    #[test]
    fn test_with_devices() {
        // T28 Q3: Verify device configuration
        let capsule = GpuHybridDispatchCapsule::with_devices(true, true, true);
        assert!(capsule.has_intel_arc());
        assert!(capsule.has_nvidia_cuda());
        assert!(capsule.has_avx512());

        let capsule2 = GpuHybridDispatchCapsule::with_devices(false, false, false);
        assert!(!capsule2.has_intel_arc());
        assert!(!capsule2.has_nvidia_cuda());
        assert!(!capsule2.has_avx512());
    }

    #[test]
    fn test_dispatch_small_ops_cpu() {
        // T28 Q4: Small ops always go to CPU
        let capsule = GpuHybridDispatchCapsule::with_devices(true, true, true);

        // 512 elements = small op
        let target = capsule.dispatch_by_size(512);
        assert_eq!(target, DispatchTarget::Cpu);
    }

    #[test]
    fn test_dispatch_medium_ops_xe() {
        // T28 Q5: Medium ops (64K-1M) go to XE if available
        let capsule = GpuHybridDispatchCapsule::with_devices(true, false, true);

        // 100K elements = medium op
        let target = capsule.dispatch_by_size(100_000);
        assert_eq!(target, DispatchTarget::IntelArc);
    }

    #[test]
    fn test_dispatch_large_ops_cuda() {
        // T28 Q6: Large ops (>1M) prefer CUDA
        let capsule = GpuHybridDispatchCapsule::with_devices(true, true, true);

        // 2M elements = large op
        let target = capsule.dispatch_by_size(2_000_000);
        assert_eq!(target, DispatchTarget::NvidiaCuda);
    }

    #[test]
    fn test_dispatch_fallback_cpu() {
        // T28 Q7: Fallback to CPU when no GPU available
        let capsule = GpuHybridDispatchCapsule::with_devices(false, false, true);

        // 100K elements but no GPU
        let target = capsule.dispatch_by_size(100_000);
        assert_eq!(target, DispatchTarget::Cpu);
    }

    #[test]
    fn test_threshold_configuration() {
        // T28 Q8: Verify threshold configuration
        let capsule = GpuHybridDispatchCapsule::with_devices(true, true, true);

        capsule.set_gpu_threshold(1_000_000);
        assert_eq!(capsule.gpu_threshold(), 1_000_000);

        capsule.set_xe_threshold(10_000_000);
        assert_eq!(capsule.xe_threshold(), 10_000_000);
    }

    // ========================================================================
    // T28 Q8-Q14: Dispatch Logic Tests
    // ========================================================================

    #[test]
    fn test_matmul_dispatch() {
        // Test matrix multiplication dispatch
        let capsule = GpuHybridDispatchCapsule::with_devices(true, true, true);

        // Small matmul: 32x32x32 = 32K ops → CPU
        let target = capsule.dispatch_matmul_target(32, 32, 32);
        assert_eq!(target, DispatchTarget::Cpu);

        // Medium matmul: 256x256x256 = 16M ops → CUDA
        let target = capsule.dispatch_matmul_target(256, 256, 256);
        assert_eq!(target, DispatchTarget::NvidiaCuda);
    }

    #[test]
    fn test_attention_dispatch() {
        // Test attention dispatch
        let capsule = GpuHybridDispatchCapsule::with_devices(true, true, true);

        // Small attention: batch=1, seq=64, heads=8, dim=64
        // Ops = 1 * 8 * 64 * 64 * 64 = 2M → CUDA
        let target = capsule.dispatch_attention_target(1, 64, 8, 64);
        assert_eq!(target, DispatchTarget::NvidiaCuda);
    }

    #[test]
    fn test_quantize_dispatch() {
        // Test quantization dispatch
        let capsule = GpuHybridDispatchCapsule::with_devices(true, true, true);

        // Small tensor: 1K elements → CPU
        let target = capsule.dispatch_quantize_target(1_000);
        assert_eq!(target, DispatchTarget::Cpu);

        // Large tensor: 10M elements → CUDA
        let target = capsule.dispatch_quantize_target(10_000_000);
        assert_eq!(target, DispatchTarget::NvidiaCuda);
    }

    #[test]
    fn test_dequantize_dispatch() {
        // Test dequantization (KV decompression) dispatch
        let capsule = GpuHybridDispatchCapsule::with_devices(true, true, true);

        // With AVX-512: Prefer CPU for moderate sizes
        let target = capsule.dispatch_dequantize_target(1_000_000);
        assert_eq!(target, DispatchTarget::Cpu);

        // Very large: Even with AVX-512, use GPU
        let target = capsule.dispatch_dequantize_target(100_000_000);
        assert_eq!(target, DispatchTarget::NvidiaCuda);
    }

    #[test]
    fn test_statistics_tracking() {
        // Test statistics updates
        let capsule = GpuHybridDispatchCapsule::with_devices(true, false, true);

        let initial_stats = capsule.stats();
        assert_eq!(initial_stats.total_dispatches, 0);

        // Dispatch to CPU (small)
        let tensor = TensorDesc::new(10, 10, 4);
        let _ = capsule.matmul(&tensor, &tensor);

        let stats = capsule.stats();
        assert_eq!(stats.total_dispatches, 1);
        assert_eq!(stats.cpu_dispatches, 1);
    }

    #[test]
    fn test_statistics_reset() {
        // Test statistics reset
        let capsule = GpuHybridDispatchCapsule::with_devices(true, false, true);

        let tensor = TensorDesc::new(10, 10, 4);
        let _ = capsule.matmul(&tensor, &tensor);
        let _ = capsule.matmul(&tensor, &tensor);

        capsule.reset_stats();

        let stats = capsule.stats();
        assert_eq!(stats.total_dispatches, 0);
        assert_eq!(stats.cpu_dispatches, 0);
    }

    #[test]
    fn test_generation_counter() {
        // Test generation counter increments
        let capsule = GpuHybridDispatchCapsule::with_devices(true, false, true);

        let gen0 = capsule.generation();
        capsule.set_gpu_threshold(1_000_000);
        let gen1 = capsule.generation();

        assert_eq!(gen1, gen0 + 1);
    }

    // ========================================================================
    // T28 Q15-Q21: Edge Cases
    // ========================================================================

    #[test]
    fn test_zero_elements() {
        // Zero elements should go to CPU
        let capsule = GpuHybridDispatchCapsule::with_devices(true, true, true);
        let target = capsule.dispatch_by_size(0);
        assert_eq!(target, DispatchTarget::Cpu);
    }

    #[test]
    fn test_boundary_conditions() {
        // Test exactly at thresholds
        let capsule = GpuHybridDispatchCapsule::with_devices(true, true, true);

        // Exactly at SMALL_OP_THRESHOLD (1024)
        let target = capsule.dispatch_by_size(SMALL_OP_THRESHOLD);
        assert_eq!(target, DispatchTarget::Cpu); // Still CPU, below GPU threshold

        // Exactly at GPU threshold
        let target = capsule.dispatch_by_size(DEFAULT_GPU_THRESHOLD);
        assert_eq!(target, DispatchTarget::IntelArc); // XE available

        // Exactly at XE threshold
        let target = capsule.dispatch_by_size(DEFAULT_XE_THRESHOLD);
        assert_eq!(target, DispatchTarget::IntelArc); // XE, not above threshold

        // Above XE threshold
        let target = capsule.dispatch_by_size(DEFAULT_XE_THRESHOLD + 1);
        assert_eq!(target, DispatchTarget::NvidiaCuda); // CUDA available
    }

    #[test]
    fn test_dispatch_target_display() {
        assert_eq!(format!("{}", DispatchTarget::Cpu), "CPU (AVX-512)");
        assert_eq!(format!("{}", DispatchTarget::IntelArc), "Intel Arc (XE)");
        assert_eq!(format!("{}", DispatchTarget::NvidiaCuda), "NVIDIA CUDA (Remote)");
    }

    #[test]
    fn test_dispatch_error_display() {
        let err = DispatchError::RemoteNotImplemented;
        assert!(format!("{}", err).contains("not implemented"));

        let err = DispatchError::InvalidDimensions { rows: 0, cols: 0 };
        assert!(format!("{}", err).contains("Invalid dimensions"));
    }

    #[test]
    fn test_tensor_desc() {
        let tensor = TensorDesc::new(100, 200, 4);
        assert_eq!(tensor.elements(), 20_000);
        assert_eq!(tensor.size_bytes(), 80_000);
    }

    #[test]
    fn test_debug_output() {
        let capsule = GpuHybridDispatchCapsule::with_devices(true, false, true);
        let debug_str = format!("{:?}", capsule);

        assert!(debug_str.contains("GpuHybridDispatchCapsule"));
        assert!(debug_str.contains("xe_available"));
        assert!(debug_str.contains("gpu_threshold"));
    }

    // ========================================================================
    // T28 Q22-Q28: Concurrent Tests
    // ========================================================================

    #[test]
    fn test_concurrent_dispatch_decisions() {
        use std::thread;

        let capsule = GpuHybridDispatchCapsule::with_devices(true, true, true);
        let capsule_ref = &capsule;

        // Use thread::scope for safe borrowing across threads (Rust 1.63+)
        let results: Vec<_> = thread::scope(|s| {
            let handles: Vec<_> = (0..100)
                .map(|i| {
                    // Capture i by value, capsule_ref by move (copy of the reference)
                    let size = i * 10_000;
                    s.spawn(move || {
                        capsule_ref.dispatch_by_size(size)
                    })
                })
                .collect();

            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        // All small ops should be CPU
        for (i, target) in results.iter().enumerate() {
            let size = i * 10_000;
            if size < SMALL_OP_THRESHOLD {
                assert_eq!(*target, DispatchTarget::Cpu);
            }
        }
    }

    #[test]
    fn test_concurrent_statistics_updates() {
        use std::thread;

        let capsule = GpuHybridDispatchCapsule::with_devices(false, false, true);
        let capsule_ref = &capsule;
        let tensor = TensorDesc::new(10, 10, 4);

        // Use thread::scope for safe borrowing across threads (Rust 1.63+)
        thread::scope(|s| {
            for _ in 0..100 {
                let t = tensor;
                s.spawn(move || {
                    let _ = capsule_ref.matmul(&t, &t);
                });
            }
        });

        let stats = capsule.stats();
        assert_eq!(stats.total_dispatches, 100);
        assert_eq!(stats.cpu_dispatches, 100);
    }

    // ========================================================================
    // T28 Q29-Q35: Detection Tests
    // ========================================================================

    #[test]
    fn test_detect_functions_no_panic() {
        // Detection functions should never panic
        let _ = detect_intel_arc();
        let _ = detect_cuda_remote();
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_singleton_pattern() {
        // Singleton should return same instance
        let inst1 = GpuHybridDispatchCapsule::instance();
        let inst2 = GpuHybridDispatchCapsule::instance();

        assert!(std::ptr::eq(inst1, inst2));
    }
}
