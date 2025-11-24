// P0.3 Test 5: T1 Atomic capsule with abbreviated "gen" field (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_missing_generation)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct AbbreviatedGenCapsule {
    state: AtomicU64,
    gen: AtomicU64,
    _padding: [u8; 48],
}

fn main() {}
