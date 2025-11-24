// P0.4 Test 5: T1 Atomic capsule with non-atomic i64 field (FAIL)
#![feature(rustc_private)]
#![deny(clippy::capsule_non_atomic_field)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct BadI64Capsule { //~ ERROR: T1 Atomic capsule contains non-atomic field `delta` of type `i64`. Use `AtomicI64` instead
    state: AtomicU64,
    delta: i64,
    generation: AtomicU64,
    _padding: [u8; 40],
}

fn main() {}
