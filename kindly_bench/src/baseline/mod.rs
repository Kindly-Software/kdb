//! Baseline generation strategies for T7-T11 specialized tiers
//!
//! # Overview
//!
//! Phase 3 tiers require different baseline strategies than T1-T6:
//!
//! - **T7 Heterogeneous**: Manual CPU baseline (GPU → CPU)
//! - **T8 Network**: Manual single-node baseline (Distributed → Single-node)
//! - **T9 Persistent**: Auto-generated in-memory baseline (Mmap → In-memory)
//! - **T10 Probabilistic**: Manual exact baseline (Approximate → Exact)
//! - **T11 QuantumHybrid**: Manual classical baseline (Quantum → Classical)
//!
//! # Manual vs Auto-Generated
//!
//! **Auto-generated baselines** (T1-T6, T9):
//! - Framework can automatically generate fair comparison code
//! - Example: T1 Atomic → RwLock, T2 SIMD → Scalar
//!
//! **Manual baselines** (T7-T8, T10-T11):
//! - Require domain expertise for fair comparison
//! - Framework provides guides and examples
//! - User implements baseline function

pub mod t7_gpu;
pub mod t8_network;
pub mod t9_persistent;
pub mod t10_probabilistic;
pub mod t11_quantum;

pub use t7_gpu::T7GpuBaseline;
pub use t8_network::T8NetworkBaseline;
pub use t9_persistent::T9PersistentBaseline;
pub use t10_probabilistic::T10ProbabilisticBaseline;
pub use t11_quantum::T11QuantumBaseline;

/// Manual baseline function type
pub type ManualBaselineFn<T> = Box<dyn Fn() -> T>;

/// Baseline generator trait for specialized tiers
pub trait BaselineGenerator<T> {
    /// Generate baseline implementation
    ///
    /// For manual baselines (T7, T8, T10, T11), this returns None.
    /// For auto-generated baselines (T9), this returns Some(baseline_fn).
    fn generate_baseline(&self) -> Option<ManualBaselineFn<T>>;

    /// Whether this baseline is auto-generated or requires manual implementation
    fn is_auto_generated(&self) -> bool;

    /// Baseline description for manual implementation guide
    fn manual_guide(&self) -> &'static str;
}
