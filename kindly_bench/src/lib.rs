//! kindly_bench: B32-compliant benchmark framework for computational capsule primitives
//!
//! # Overview
//!
//! Provides automatic baseline generation, multi-timer infrastructure, and B32 compliance
//! enforcement for benchmarking computational capsules across all 11 tiers (T0-T11).
//!
//! # Features
//!
//! - **Phase 1 (T1-T3)**: TSC timing, atomic/SIMD/fixed-point baselines
//! - **Phase 2 (T0, T4-T6)**: Extended tier support, builder API
//! - **Phase 3 (T7-T11)**: Specialized tier support, multi-timer infrastructure
//!
//! # Phase 3 Tiers
//!
//! - **T7 Heterogeneous**: GPU/FPGA/TPU acceleration (GPU timer, manual CPU baseline)
//! - **T8 Network**: Distributed coordination (Instant timer, manual single-node baseline)
//! - **T9 Persistent**: Memory-mapped atomics (auto-generated in-memory baseline)
//! - **T10 Probabilistic**: Approximate algorithms (manual exact baseline)
//! - **T11 QuantumHybrid**: Quantum/neuromorphic (Quantum timer, manual classical baseline)
//!
//! # Examples
//!
//! ```rust,ignore
//! use kindly_bench::*;
//!
//! // T7 GPU benchmark with manual CPU baseline
//! #[cfg(feature = "gpu")]
//! let config = BenchmarkConfig::builder()
//!     .tier(Tier::T7Heterogeneous)
//!     .timer(Timer::Gpu(GpuTimer::cuda()))
//!     .baseline_manual(Box::new(|| cpu_matmul(&a, &b)))
//!     .build();
//!
//! // T8 Network benchmark with manual single-node baseline
//! #[cfg(feature = "network")]
//! let config = BenchmarkConfig::builder()
//!     .tier(Tier::T8Network)
//!     .timer(Timer::Instant)
//!     .baseline_manual(Box::new(|| single_node_training(&model, &data)))
//!     .build();
//!
//! // T9 Persistent benchmark with auto-generated in-memory baseline
//! let config = BenchmarkConfig::builder()
//!     .tier(Tier::T9Persistent)
//!     .baseline(BaselineKind::InMemory)
//!     .build();
//! ```

#![cfg_attr(not(test), no_std)]
#![allow(internal_features)]
#![feature(core_intrinsics)]  // For TSC timing (Phase 1)

// Phase 3: Timing infrastructure
pub mod timing;

// Phase 3: Baseline generation strategies
pub mod baseline;

// Phase 3: Specialized validation
pub mod validation;

// Re-exports for public API
pub use timing::{BenchTimer, TscTimer, InstantTimer};

#[cfg(feature = "gpu")]
pub use timing::GpuTimer;

#[cfg(feature = "quantum")]
pub use timing::QuantumTimer;

pub use baseline::{
    T7GpuBaseline, T8NetworkBaseline, T9PersistentBaseline, T10ProbabilisticBaseline,
    T11QuantumBaseline,
};

pub use validation::specialized::{
    validate_gpu_available, validate_network_config, validate_quantum_backend,
};

/// Phase 3 tier enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    // Phase 1 tiers
    T1Atomic,
    T2Simd,
    T3FixedPoint,
    // Phase 2 tiers
    T0Auditable,
    T4Batch,
    T5Streaming,
    T6Mixed,
    // Phase 3 tiers
    T7Heterogeneous,
    T8Network,
    T9Persistent,
    T10Probabilistic,
    T11QuantumHybrid,
}

/// Phase 3 baseline kind enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineKind {
    // Phase 1 baselines
    RwLock,
    Mutex,
    Scalar,
    F64,
    // Phase 2 baselines
    NoAuditTrail,
    Sequential,
    Batch,
    NonOptimizedComposition,
    // Phase 3 baselines
    CpuOnly,        // T7 Heterogeneous
    SingleNode,     // T8 Network
    InMemory,       // T9 Persistent
    Exact,          // T10 Probabilistic
    ClassicalOnly,  // T11 QuantumHybrid
    Custom,
}

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const FRAMEWORK_NAME: &str = "kindly_bench";
