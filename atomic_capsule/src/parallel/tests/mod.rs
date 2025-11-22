//! Comprehensive test suite for lockfree parallel computing
//!
//! ## Test Organization (T28 Framework)
//!
//! - **unit.rs**: Tier 1 unit tests (25+ tests, basic correctness)
//! - **property.rs**: Tier 2 property tests (15+ tests, invariant validation)
//! - **integration.rs**: Tier 3 integration tests (12+ tests, component composition)
//! - **stress.rs**: Tier 4 stress tests (10+ tests, production scenarios)
//! - **scoped_tests.rs**: Phase 2 scoped thread tests (35+ tests, T28 comprehensive)
//! - **iter_tests.rs**: Phase 3 parallel iterator tests (33+ tests, T28 comprehensive)
//! - **phase3_1_tests.rs**: Phase 3.1 new features tests (32+ tests, fold_with_combiner/reduce/lazy/auto-batch)
//! - **phase3_2_lazy_tests.rs**: Phase 3.2 lazy evaluation tests (30+ tests, zero-allocation closure composition)
//! - **phase4_partition_find_tests.rs**: Phase 4 partition/find tests (40+ tests, parallel split and early-exit search)
//! - **chunked_tests.rs**: Phase 5.16.1 ChunkedMmapReader tests (15+ tests, T28 Q1-Q7 unit validation)
//! - **loom_tests.rs**: Tier 3 Loom model checking (7 scenarios, exhaustive interleaving validation)
//! - **parallel_panic_minimal.rs**: Minimal panic-induced livelock reproduction (3 scenarios)
//! - **parallel_shutdown_minimal.rs**: Minimal shutdown livelock reproduction (4 scenarios)
//! - **parallel_contention_minimal.rs**: Minimal contention livelock reproduction (5 scenarios)
//!
//! ## Framework Application
//!
//! **T28**: 28-question systematic testing (unit/property/integration/production)
//! **B32**: Honest benchmarking (statistical rigor, optimized baseline)
//! **ASSUM**: Safety assumption verification (all 10 categories)
//! **I20**: Integration analysis (20 questions for composition)

pub mod chunked_tests;
pub mod integration;
pub mod iter_tests;
pub mod phase3_1_tests;
pub mod phase3_2_lazy_tests;
pub mod phase4_partition_find_tests;
pub mod property;
pub mod scoped_tests;
pub mod stress;
pub mod unit;

// Minimal livelock reproduction tests (instrumented for debugging)
pub mod parallel_contention_minimal;
pub mod parallel_panic_minimal;
pub mod parallel_shutdown_minimal;

#[cfg(loom)]
pub mod loom_tests;
