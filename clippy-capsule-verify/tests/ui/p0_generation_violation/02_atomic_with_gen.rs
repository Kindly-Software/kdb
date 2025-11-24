// P0.3 Test 2: T1 Atomic capsule with generation counter (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_missing_generation)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct GoodCapsule {
    state: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 48],
}

fn main() {}
