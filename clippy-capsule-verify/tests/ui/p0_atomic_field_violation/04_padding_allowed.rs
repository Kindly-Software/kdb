// P0.4 Test 4: T1 Atomic capsule with padding arrays is OK (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_non_atomic_field)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct PaddingCapsule {
    state: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 48],
}

fn main() {}
