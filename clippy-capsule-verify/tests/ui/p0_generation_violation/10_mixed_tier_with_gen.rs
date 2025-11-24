// P0.3 Test 10: T6 Mixed capsule with generation is OK (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_missing_generation)]

use std::sync::atomic::AtomicU64;
use std::simd::f32x8;

#[derive(ComputationalCapsule)]
#[capsule(tier = "Mixed", alignment = 256)]
#[repr(C, align(256))]
struct MixedCapsule {
    atomic_state: AtomicU64,
    simd_data: f32x8,
    generation: AtomicU64,
    _padding: [u8; 208],
}

fn main() {}
