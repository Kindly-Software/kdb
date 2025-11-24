// P0.3 Test 8: T1 Atomic capsule with "generation_counter" field (not exact match) (FAIL)
#![feature(rustc_private)]
#![deny(clippy::capsule_missing_generation)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct WrongNameCapsule { //~ ERROR: T1 Atomic capsule missing generation counter field (generation or gen)
    state: AtomicU64,
    generation_counter: AtomicU64,
    _padding: [u8; 48],
}

fn main() {}
