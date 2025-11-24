// P0.3 Test 9: T4 Batch capsule without generation is OK (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_missing_generation)]

use std::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Batch", alignment = 64)]
#[repr(C, align(64))]
struct BatchCapsule {
    batch_size: AtomicU64,
    processed: AtomicU64,
    _padding: [u8; 48],
}

fn main() {}
