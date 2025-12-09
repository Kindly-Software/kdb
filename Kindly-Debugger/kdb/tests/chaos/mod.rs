//! P0 Critical Chaos Tests - Real Failure Injection for kdb
//!
//! These tests inject real system failures (OOM, FD exhaustion, signals, process death)
//! to validate kdb's resilience under chaos conditions.
//!
//! # Requirements
//!
//! - Linux x86_64
//! - `chaos-testing` feature enabled
//! - Some tests require forking (marked with `#[ignore]`)
//! - Some tests require elevated privileges (marked with `#[ignore]`)
//!
//! # Running
//!
//! ```bash
//! # All chaos tests (requires root for some)
//! cargo test --features chaos-testing --test chaos_tests -- --ignored
//!
//! # Specific test suite
//! cargo test --features chaos-testing --test chaos_tests audit_trail -- --ignored
//! cargo test --features chaos-testing --test chaos_tests ptrace_resilience -- --ignored
//! cargo test --features chaos-testing --test chaos_tests resource_exhaustion -- --ignored
//! ```
//!
//! # Safety
//!
//! - Tests use `#[ignore]` for destructive operations
//! - Original resource limits are restored on Drop
//! - Temporary directories are cleaned up automatically
//! - Forked processes are properly waited/killed
//!
//! # Framework Compliance
//!
//! - T28 Q22-Q28: Production stress scenarios
//! - ASSUM: All assumptions documented and verified
//! - Chaos: Uses kdb's lockfree capsule architecture

#[cfg(all(target_os = "linux", feature = "chaos-testing"))]
mod infrastructure;

#[cfg(all(target_os = "linux", feature = "chaos-testing"))]
mod audit_trail_tests;

#[cfg(all(target_os = "linux", feature = "chaos-testing"))]
mod ptrace_resilience_tests;

#[cfg(all(target_os = "linux", feature = "chaos-testing"))]
mod resource_exhaustion_tests;

// Re-export infrastructure for use in test modules
#[cfg(all(target_os = "linux", feature = "chaos-testing"))]
pub use infrastructure::*;

// Placeholder test when chaos-testing is not enabled
#[cfg(not(all(target_os = "linux", feature = "chaos-testing")))]
#[test]
fn chaos_testing_requires_linux_and_feature() {
    eprintln!("Chaos testing requires Linux and the 'chaos-testing' feature.");
    eprintln!("Run with: cargo test --features chaos-testing --test chaos_tests");
}
