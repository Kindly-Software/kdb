// P0.3 Test 1: T1 Atomic capsule missing generation counter (FAIL)
#![feature(rustc_private)]
#![deny(clippy::capsule_missing_generation)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct BadCapsule { //~ ERROR: T1 Atomic capsule missing generation counter field (generation or gen)
    state: AtomicU64,
    _padding: [u8; 56],
}

fn main() {}
