// P0.3 Test 6: T1 Atomic capsule with multiple atomic fields but no generation (FAIL)
#![feature(rustc_private)]
#![deny(clippy::capsule_missing_generation)]

use std::sync::atomic::{AtomicU64, AtomicU32};

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct MultiAtomicBad { //~ ERROR: T1 Atomic capsule missing generation counter field (generation or gen)
    state: AtomicU64,
    count: AtomicU32,
    flags: AtomicU32,
    _padding: [u8; 48],
}

fn main() {}
