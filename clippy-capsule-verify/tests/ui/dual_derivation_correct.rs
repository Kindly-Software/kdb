//! # UI Test: Correct Dual-Derivation
//!
//! **Test Objective**: Verify NO lint fires when both #[derive(CapsuleSerialize)] and
//! #[derive(ComputationalCapsule)] are present.
//!
//! **Expected**: No warnings (compile-pass)

#![warn(clippy::missing_capsule_verification)]
#![allow(dead_code)]

// Mock derive macros (for UI test purposes)
struct CapsuleSerialize;
struct ComputationalCapsule;

// GOOD: Both derives present (dual-derivation)
#[derive(CapsuleSerialize, ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct GoodCapsule {
    field1: u64,
    field2: i32,
    _padding: [u8; 52],
}

// GOOD: Only ComputationalCapsule (no CapsuleSerialize required)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct OnlyCapsule {
    field1: u64,
    _padding: [u8; 56],
}

fn main() {}
