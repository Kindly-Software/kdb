//! # UI Test: Combined serde + CapsuleSerialize (Dual-Derivation Pattern)
//!
//! **Test Objective**: Verify lint allows dual serde + CapsuleSerialize derives when
//! ComputationalCapsule is also present.
//!
//! **Expected**: No warnings (all derives correct)

#![warn(clippy::missing_capsule_verification)]
#![allow(dead_code)]

// Mock derive macros
struct Serialize;
struct Deserialize;
struct CapsuleSerialize;
struct ComputationalCapsule;

// GOOD: Triple derivation (serde + CapsuleSerialize + ComputationalCapsule)
#[derive(Serialize, Deserialize, CapsuleSerialize, ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct PaymentCapsule {
    amount_cents: i64,
    fee_cents: i64,
    timestamp_unix: u64,
    _padding: [u8; 40],
}

fn main() {}
