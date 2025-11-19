//! Testing infrastructure for kindly_dedup validation
//!
//! This module provides high-performance testing primitives for validating
//! PersistentDedupPipeline claims using computational capsule architecture.
//!
//! ## Components
//!
//! - **PerformanceBenchmarkCapsule** (T0 Auditable + B32): Statistical benchmarking with 1000+ iterations, 95% CI
//! - **MemoryMonitorCapsule** (T1 Atomic): Lockfree RSS tracking for memory reduction validation
//! - **SyntheticCorpusGeneratorCapsule** (T2 SIMD + T10 Probabilistic): Deterministic corpus generation
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q1-Q34 systematic discovery (T0, T1, and T2+T10 tier selection)
//! - **ASSUM**: 99.99% safety (all assumptions documented and verified)
//! - **B32**: Fair benchmarking framework (1000+ iterations, 95% CI, deterministic)
//! - **T28**: Comprehensive testing (unit, property, integration, production tests)
//! - **COCA**: 100% computational capsules (no mutex, zero unsafe code in hot paths)

pub mod benchmark;
pub mod corpus_generator;
pub mod crash_recovery;
pub mod memory_monitor;
pub mod scale_suite;

pub use benchmark::{BenchmarkResult, PerformanceBenchmarkCapsule};
pub use corpus_generator::{GenerationStats, SyntheticCorpusGeneratorCapsule};
pub use crash_recovery::{CrashRecoveryResult, CrashRecoveryTesterCapsule, MmapHeader};
pub use memory_monitor::MemoryMonitorCapsule;
pub use scale_suite::{
    ScaleTestConfig, ScaleTestResult, ScaleTestSuiteCapsule, TestStatus,
};
