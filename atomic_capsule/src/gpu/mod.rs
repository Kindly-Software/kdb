//! GPU Intel Chaos Driver Capsules
//!
//! T0-T8 lockfree GPU power management, memory bandwidth tracking, firmware authentication, multi-engine scheduling, telemetry streaming, and integration capsules.
//!
//! # Modules
//!
//! - [`memory_bandwidth_capsule`]: MemoryBandwidthCapsule (128B, T3 Fixed-Point) for deterministic bandwidth tracking
//! - [`power_management_capsule`]: PowerManagementCapsule (64B) for GPU power state tracking
//! - [`huc_authentication_capsule`]: HuCAuthenticationCapsule (128B, T8 Network) for HuC firmware authentication
//! - [`multi_engine_scheduler_capsule`]: MultiEngineSchedulerCapsule (256B, T8 Network) for multi-engine workload distribution
//! - [`telemetry_capsule`]: TelemetryCapsule (512B, T5 Streaming) for GPU telemetry streaming (temperature, frequency, utilization, power)
//! - [`kernels`]: GPU kernel capsules (T7 Heterogeneous tier, CUDA/ROCm/CPU fallback)
//! - [`hybrid_dispatch`]: GpuHybridDispatchCapsule (256B, T7 Heterogeneous) for intelligent multi-accelerator dispatch

pub mod memory_bandwidth_capsule;
pub mod power_management_capsule;
pub mod huc_authentication_capsule;
pub mod cross_process_sync_capsule;
pub mod multi_engine_scheduler_capsule;
pub mod display_engine_capsule;
pub mod telemetry_capsule;

// GPU kernels module (T7 Heterogeneous tier)
pub mod error;
pub mod kernels;

// GPU Hybrid Dispatch (T7 Heterogeneous tier - multi-accelerator)
#[cfg(feature = "gpu-hybrid-dispatch")]
pub mod hybrid_dispatch;

pub use memory_bandwidth_capsule::{
    MemoryBandwidthCapsuleAligned, Q16_16, Q24_8,
};

pub use power_management_capsule::{
    PowerManagementCapsule, PowerManagementSnapshot, PowerState, FrequencyBand,
};

pub use huc_authentication_capsule::{
    HuCAuthenticationCapsule, AuthState, Challenge, AuthResponse, AuthSnapshot, HuCAuthError,
};

pub use multi_engine_scheduler_capsule::{
    MultiEngineSchedulerCapsule, GpuEngine, EngineLoadSnapshot,
};

pub use display_engine_capsule::{
    DisplayEngineCapsule,
    DisplayState,
    PlaneType,
    ConnectorType,
    VsyncState,
    ScanoutMode,
    ColorSpace,
};

pub use cross_process_sync_capsule::{
    CrossProcessSyncCapsule, SyncState, CrossProcessSyncError, CrossProcessSyncResult,
};

pub use telemetry_capsule::{
    TelemetryCapsule, TelemetryMetric, TelemetrySnapshot,
};

// GPU Hybrid Dispatch exports
#[cfg(feature = "gpu-hybrid-dispatch")]
pub use hybrid_dispatch::{
    GpuHybridDispatchCapsule,
    DispatchTarget,
    DispatchError,
    DispatchStats,
    TensorDesc,
    TransferDirection,
    detect_intel_arc,
    detect_cuda_remote,
    DEFAULT_GPU_THRESHOLD,
    DEFAULT_XE_THRESHOLD,
    SMALL_OP_THRESHOLD,
};
