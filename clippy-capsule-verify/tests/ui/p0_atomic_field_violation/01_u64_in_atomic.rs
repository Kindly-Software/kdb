// P0.4 Test 1: T1 Atomic capsule with non-atomic u64 field (FAIL)
#![feature(rustc_private)]
#![deny(clippy::capsule_non_atomic_field)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct BadU64Capsule { //~ ERROR: T1 Atomic capsule contains non-atomic field `count` of type `u64`. Use `AtomicU64` instead
    state: AtomicU64,
    count: u64,
    generation: AtomicU64,
    _padding: [u8; 40],
}

fn main() {}
