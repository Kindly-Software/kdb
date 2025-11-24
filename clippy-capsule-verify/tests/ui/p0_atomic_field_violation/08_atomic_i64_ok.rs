// P0.4 Test 8: T1 Atomic capsule with AtomicI64 is OK (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_non_atomic_field)]

use std::sync::atomic::{AtomicU64, AtomicI64};

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct AtomicI64Capsule {
    state: AtomicU64,
    delta: AtomicI64,
    generation: AtomicU64,
    _padding: [u8; 40],
}

fn main() {}
