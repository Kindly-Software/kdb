//! Multi-timer infrastructure for tier-specific benchmarking
//!
//! # Overview
//!
//! Different computational capsule tiers require different timing strategies:
//!
//! - **TSC (T1-T6)**: Cycle-accurate CPU timing via RDTSC
//! - **Instant (T8)**: Wall-clock timing for network benchmarks
//! - **GPU (T7)**: CUDA/Vulkan event timing for GPU kernels
//! - **Quantum (T11)**: Specialized quantum backend timing
//!
//! # Timer Trait
//!
//! All timers implement the `BenchTimer` trait for uniform API:
//!
//! ```rust,ignore
//! pub trait BenchTimer {
//!     type Timestamp;
//!
//!     fn start(&mut self) -> Self::Timestamp;
//!     fn end(&mut self, start: Self::Timestamp) -> u64; // nanoseconds
//!     fn calibrate_overhead(&mut self) -> u64;
//!     fn resolution(&self) -> u64; // nanoseconds
//! }
//! ```

// TSC timer (Phase 1, T1-T6)
pub mod tsc;

// Instant timer (Phase 3, T8 Network)
pub mod instant;

// GPU timer (Phase 3, T7 Heterogeneous)
#[cfg(feature = "gpu")]
pub mod gpu;

// Quantum timer (Phase 3, T11 QuantumHybrid)
#[cfg(feature = "quantum")]
pub mod quantum;

pub use tsc::TscTimer;
pub use instant::InstantTimer;

#[cfg(feature = "gpu")]
pub use gpu::GpuTimer;

#[cfg(feature = "quantum")]
pub use quantum::QuantumTimer;

/// Universal timer trait for all tiers
pub trait BenchTimer {
    /// Timestamp type (platform-specific)
    type Timestamp: Copy;

    /// Start timing measurement
    fn start(&mut self) -> Self::Timestamp;

    /// End timing measurement and return elapsed nanoseconds
    fn end(&mut self, start: Self::Timestamp) -> u64;

    /// Calibrate timer overhead (nanoseconds)
    fn calibrate_overhead(&mut self) -> u64;

    /// Timer resolution (nanoseconds)
    fn resolution(&self) -> u64;
}

/// Timer selection enum for builder API
#[derive(Debug, Clone, Copy)]
pub enum Timer {
    /// TSC timing (T1-T6, cycle-accurate)
    Tsc,
    /// Instant timing (T8 Network, wall-clock)
    Instant,
    /// GPU timing (T7 Heterogeneous, CUDA/Vulkan events)
    #[cfg(feature = "gpu")]
    Gpu,
    /// Quantum timing (T11 QuantumHybrid, specialized)
    #[cfg(feature = "quantum")]
    Quantum,
}

impl Default for Timer {
    fn default() -> Self {
        Timer::Tsc
    }
}
