// P0.4 Test 2: T1 Atomic capsule with non-atomic bool field (FAIL)
#![feature(rustc_private)]
#![deny(clippy::capsule_non_atomic_field)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct BadBoolCapsule { //~ ERROR: T1 Atomic capsule contains non-atomic field `active` of type `bool`. Use `AtomicBool` instead
    state: AtomicU64,
    active: bool,
    generation: AtomicU64,
    _padding: [u8; 47],
}

fn main() {}
