//! Test utilities for clapi_core
//!
//! ## Purpose
//! Reduce test boilerplate and improve test productivity (P1 Enhancements 7-8)
//!
//! ## Modules
//! - `concurrent_test_builder`: Fluent API for concurrent property tests (87% boilerplate reduction)
//! - `timeline_fixture`: Pre-built fixtures for timeline aggregation tests (zero duplication)

pub mod concurrent_test_builder;
pub mod timeline_fixture;

// Re-export commonly used items
pub use concurrent_test_builder::{ConcurrentTestBuilder, ConcurrentTestResult};
pub use timeline_fixture::TimelineFixture;
