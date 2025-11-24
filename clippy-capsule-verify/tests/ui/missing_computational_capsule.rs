//! # UI Test: Missing ComputationalCapsule on CapsuleSerialize
//!
//! **Test Objective**: Verify lint fires when #[derive(CapsuleSerialize)] is used without
//! #[derive(ComputationalCapsule)].
//!
//! **Expected Lint Message**:
//! ```text
//! warning: struct `BadCapsule` uses #[derive(CapsuleSerialize)] but missing #[derive(ComputationalCapsule)]
//!   --> tests/ui/missing_computational_capsule.rs:XX:1
//!    |
//! XX | / #[derive(CapsuleSerialize)]
//! XX | | #[repr(C)]
//! XX | | struct BadCapsule {
//!    | |_^
//!    |
//!    = help: add `#[derive(ComputationalCapsule)]` above `#[derive(CapsuleSerialize)]`
//!    = note: CapsuleSerialize requires compile-time verification for audit trail integrity
//!    = note: missing verification causes:
//!    = note:   - Alignment mismatches → false sharing → UB in concurrent hash updates
//!    = note:   - Size mismatches → layout corruption → broken audit trails
//!    = note:   - Compliance failures: SOX 404, SOC2 Type II, GDPR Article 30
//! ```

#![warn(clippy::missing_capsule_verification)]

// Mock derive macro (for UI test purposes, not real implementation)
// In real code, this would be atomic_capsule::serialize::CapsuleSerialize
#[allow(dead_code)]
struct CapsuleSerialize;

// BAD: CapsuleSerialize without ComputationalCapsule
#[derive(CapsuleSerialize)]  //~ WARNING: missing #[derive(ComputationalCapsule)]
#[repr(C)]
struct BadCapsule {
    field1: u64,
    field2: i32,
}

fn main() {}
