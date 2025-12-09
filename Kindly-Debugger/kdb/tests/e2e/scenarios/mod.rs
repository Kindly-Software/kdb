//! E2E Test Scenarios Module
//!
//! This module contains comprehensive end-to-end test scenarios for the kdb debugger.
//! Each scenario tests a specific debugging workflow or feature.
//!
//! # Scenario Categories
//!
//! - **attach_detach**: Basic process attachment and detachment (E2E-01)
//! - **breakpoint_basic**: Breakpoint setting and hit detection (E2E-02)
//! - **time_travel**: Snapshot capture and bidirectional replay (E2E-03, E2E-04)
//! - **stack_unwinding**: SIMD-accelerated stack trace correctness (E2E-05)
//! - **memory_read**: Memory examination scenarios (E2E-06)
//! - **multi_thread**: Multi-threaded debugging scenarios (E2E-07)
//! - **user_journey**: Full debugging workflow integration tests (E2E-18)
//!
//! # ASSUM Safety
//!
//! - #ASSUME_LINUX_PTRACE: All scenarios require Linux ptrace permissions
//! - #ASSUME_TEST_ISOLATION: Each test uses independent fixture instances
//! - #ASSUME_CLEANUP_ON_DROP: All resources cleaned up via Drop impls
//! - #ASSUME_AUDIT_ENABLED: Q34 audit trail validation in relevant tests
//!
//! # Framework Compliance
//!
//! - **T28**: Q22-Q28 Production tier tests
//! - **Chaos**: Uses lockfree patterns via harness
//! - **Q34**: Validates audit trail integrity in time-travel and user journey tests

// Re-export harness types for use in scenario modules
#[allow(unused_imports)]
pub use super::harness::{
    ComparisonFixture, E2EFixture, E2EResult, E2EError, OutputValidator, ValidationConfig,
    ProcessSpawner, SpawnedProcess,
    DebuggerDriver, DebuggerEvent, Registers, StackFrame, BreakpointId, SnapshotId, StopReason,
    ComparisonResult,
};
pub use super::{has_gdb, is_linux};

pub mod attach_detach;
pub mod breakpoint_basic;
pub mod memory_read;
pub mod multi_thread;
pub mod stack_unwinding;
pub mod time_travel;
pub mod user_journey;
