//! # UI Test: Suppressed Dual-Derivation Warning
//!
//! **Test Objective**: Verify lint can be suppressed with #[allow(...)] attribute.
//!
//! **Expected**: No warnings (suppression works correctly)

#![warn(clippy::missing_capsule_verification)]
#![allow(dead_code)]

// Mock derive macro
struct CapsuleSerialize;

// SUPPRESSED: CapsuleSerialize without ComputationalCapsule (explicit allow)
#[allow(clippy::missing_capsule_verification)]
#[derive(CapsuleSerialize)]
#[repr(C)]
struct SuppressedCapsule {
    field1: u64,
    field2: i32,
}

fn main() {}
