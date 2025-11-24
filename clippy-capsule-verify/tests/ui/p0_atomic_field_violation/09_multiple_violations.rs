// P0.4 Test 9: T1 Atomic capsule with multiple non-atomic fields (FAIL)
#![feature(rustc_private)]
#![deny(clippy::capsule_non_atomic_field)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct MultipleViolations { //~ ERROR: T1 Atomic capsule contains non-atomic field `count` of type `u64`. Use `AtomicU64` instead
    //~^ ERROR: T1 Atomic capsule contains non-atomic field `active` of type `bool`. Use `AtomicBool` instead
    state: AtomicU64,
    count: u64,
    active: bool,
    generation: AtomicU64,
    _padding: [u8; 31],
}

fn main() {}
