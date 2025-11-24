// P0.4 Test 3: T1 Atomic capsule with all atomic fields (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_non_atomic_field)]

use std::sync::atomic::{AtomicU64, AtomicBool, AtomicUsize};

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct AllAtomicCapsule {
    state: AtomicU64,
    count: AtomicUsize,
    active: AtomicBool,
    generation: AtomicU64,
    _padding: [u8; 31],
}

fn main() {}
