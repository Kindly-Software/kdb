// P0.3 Test 3: DualAtomicU64 pattern with generation (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_missing_generation)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct DualAtomicCapsule {
    primary: AtomicU64,
    secondary: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 40],
}

fn main() {}
