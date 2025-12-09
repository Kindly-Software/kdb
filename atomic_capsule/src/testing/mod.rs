//! # Testing Module
//!
//! Deterministic testing infrastructure for atomic_capsule properties (Q8-Q14).
//!
//! **Exports**:
//! - `deterministic_time` - Mocked time provider for reproducible tests
//!
//! **Framework Compliance**:
//! - UCE34 Q8-Q14 (Property tests for determinism)
//! - Chaos (100% lockfree, atomic-only coordination)
//! - ASSUM (99.99% safe, zero unsafe code)
//! - B32 (Fair baselines, zero production overhead)
//! - T28 (Property tier validation)
//! - I20 (Feature-gated, backward compatible)
//!
//! # Example
//!
//! ```ignore
//! use atomic_capsule::testing::deterministic_time::{DeterministicClock, set_test_clock};
//!
//! #[test]
//! fn deterministic_test() {
//!     let clock = DeterministicClock::new(1_000_000_000);
//!     set_test_clock(Some(std::sync::Arc::new(clock)));
//!
//!     // All time-dependent operations are now deterministic
//!     assert!(true);
//! }
//! ```

pub mod deterministic_time;

pub use deterministic_time::{
    DeterministicClock, deterministic_timestamp, deterministic_timestamp_ms,
    deterministic_timestamp_us, get_test_clock, measure_test_time, set_test_clock,
    with_deterministic_clock,
};
