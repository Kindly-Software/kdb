//! Test: verified capsules require >= 64-byte alignment
//!
//! # Expected Error
//! "Verified capsules require >= 64-byte alignment"

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 32, size = 32, verified = true)]  // ❌ Too small! Verified requires >= 64 bytes
#[repr(C, align(32))]
struct TooSmallVerified {
    state: AtomicU64,
    _padding: [u8; 24],
}

fn main() {}
