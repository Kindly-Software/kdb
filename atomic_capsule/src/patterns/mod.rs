//! # Field Optimization Patterns for Atomic Capsules
//!
//! **UCE34 Tier 1 Atomic Capsule field-level optimization helpers.**
//!
//! This module provides reusable patterns for field-level optimization in atomic capsules:
//! - **DualAtomicU64**: Cache-line-separated dual-channel coordination (128-byte aligned)
//! - **CacheLineAligned<T>**: Generic cache-line-aligned wrapper (64-byte aligned)
//! - **CircuitBreaker**: Production-grade circuit breaker patterns (migrated from atomic_breaker)
//! - **PositionTrackerCapsule**: Position + timestamp coordination (APC-512 simplified pattern)
//! - **RateLimiterCapsule**: Token bucket rate limiting (T1 Atomic + T3 Fixed-Point, Q16.16)
//!
//! ## Performance Benefits (B32 Validated)
//! - **False sharing elimination**: 15-25% speedup under contention
//! - **Cache line separation**: 2-3× faster for dual-channel patterns
//! - **Zero-cost abstractions**: All compile-time verification, no runtime overhead
//! - **Circuit breaker**: <5ns reads, <15ns writes, <50ns MPMC (production-tested)
//!
//! ## Pattern Origin
//! From AGENT2_PERFORMANCE_ANALYSIS.md:
//! > "DualAtomicU64 pattern used everywhere (but manually implemented each time)
//! > 15-25% performance gain from proper cache line separation
//! > Helper utilities would eliminate 500+ lines of boilerplate"
//!
//! ## Usage
//! ```rust
//! use atomic_capsule::patterns::{DualAtomicU64, PositionTrackerCapsule};
//! use core::sync::atomic::{AtomicU64, Ordering};
//!
//! // Dual-channel coordination (circuit breaker pattern)
//! struct CircuitBreaker {
//!     state: DualAtomicU64,  // Primary: level, Secondary: generation
//! }
//!
//! impl CircuitBreaker {
//!     pub fn check_level(&self) -> u8 {
//!         let packed = self.state.load_primary(Ordering::Relaxed);
//!         (packed & 0x3) as u8
//!     }
//!
//!     pub fn generation(&self) -> u64 {
//!         self.state.load_secondary(Ordering::Acquire)
//!     }
//! }
//!
//! // Position tracking (ready-to-use pattern)
//! let tracker = PositionTrackerCapsule::new();
//! tracker.update_position(100, 1000);
//! let (position, timestamp) = tracker.load_position();
//! ```

#[cfg(feature = "nightly")]
pub mod cache_aligned;
pub mod dual_atomic;

// Circuit breaker patterns (feature-gated)
#[cfg(any(
    feature = "circuit-breaker-standard64",
    feature = "circuit-breaker-compact48"
))]
pub mod circuit_breaker;

// Phase 4.2: CNLS (Cubic-Nonlinear Schrodinger) Rule Capsule (T6 Mixed)
#[cfg(feature = "cnls")]
pub mod cnls;

// Lockfree Task Executor (T6 Mixed: T1 Atomic + T4 Batch)
#[cfg(feature = "lockfree-executor")]
pub mod lockfree_task_executor;

// Position Tracker (T1 Atomic: DualAtomicU64 pattern)
pub mod position_tracker;

// Rate Limiter (T1 Atomic + T3 Fixed-Point: Token bucket rate limiting)
pub mod rate_limiter;

// Quota Tracker (T1 Atomic: Per-user monthly quota tracking, 64 KB)
pub mod quota_tracker;

// Leader Election (T1 Atomic: Raft-style epoch-based leader election)
#[cfg(feature = "leader-election")]
pub mod leader_election;

// Time-Travel Replay Engine (T0+T1: Hash-chained bidirectional replay with Q34 compliance)
#[cfg(feature = "time-travel")]
pub mod time_travel;

// Re-export for convenience
#[cfg(feature = "nightly")]
pub use cache_aligned::CacheLineAligned;
pub use dual_atomic::DualAtomicU64;
pub use position_tracker::PositionTrackerCapsule;
pub use quota_tracker::{QuotaError, QuotaTrackerCapsule};
pub use rate_limiter::{RateLimiterCapsule, RateLimitResult};

// Re-export circuit breaker types (feature-gated)
#[cfg(any(
    feature = "circuit-breaker-standard64",
    feature = "circuit-breaker-compact48"
))]
pub use circuit_breaker::{evaluate, CircuitBreaker, CircuitBreakerGuard, Policy, State};

#[cfg(feature = "circuit-breaker-mpmc")]
pub use circuit_breaker::AtomicBreakerMPMC;

// Re-export CNLS types (feature-gated) - Phase 4.2
// Note: Re-exports will be enabled when CNLSRuleCapsule and evolve_cnls_4d are implemented
// #[cfg(feature = "cnls")]
// pub use cnls::{CNLSRuleCapsule, evolve_cnls_4d};

// Re-export LockfreeTaskExecutor types (feature-gated)
#[cfg(feature = "lockfree-executor")]
pub use lockfree_task_executor::{ExecutionReport, LockfreeTaskExecutor};

// Re-export time-travel types (feature-gated)
#[cfg(feature = "time-travel")]
pub use time_travel::{ReplayEngineCapsule, TimeSnapshot, MAX_SNAPSHOTS};

// Re-export leader-election types (feature-gated)
#[cfg(feature = "leader-election")]
pub use leader_election::{ElectionResult, LeaderElectionCapsule, LeaderInfo, LeaderState};

// Deployment orchestration (T1 Atomic + T0 Auditable)
#[cfg(feature = "std")]
pub mod deployment;

// Re-export deployment types
#[cfg(feature = "std")]
pub use deployment::{
    BackupInfo, BuildArtifact, DeploymentCapsule, DeploymentConfig, DeploymentError,
    DeploymentPhase, DeploymentResult, DeploymentState, DeploymentStats, HealthStatus,
};
