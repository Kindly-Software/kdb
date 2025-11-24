// P0.4 Test 6: T1 Atomic capsule with non-atomic usize field (FAIL)
#![feature(rustc_private)]
#![deny(clippy::capsule_non_atomic_field)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct BadUsizeCapsule { //~ ERROR: T1 Atomic capsule contains non-atomic field `index` of type `usize`. Use `AtomicUsize` instead
    state: AtomicU64,
    index: usize,
    generation: AtomicU64,
    _padding: [u8; 40],
}

fn main() {}
