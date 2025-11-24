// P0.4 Test 10: T1 Atomic capsule with nested padding structs is OK (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_non_atomic_field)]

use std::sync::atomic::AtomicU64;

#[repr(C)]
struct Padding48 {
    _bytes: [u8; 48],
}

#[derive(ComputationalCapsule)]
#[capsule(tier = "Atomic", alignment = 64)]
#[repr(C, align(64))]
struct NestedPaddingCapsule {
    state: AtomicU64,
    generation: AtomicU64,
    _padding: Padding48,
}

fn main() {}
