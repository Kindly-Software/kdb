// P0.3 Test 4: T2 SIMD capsule without generation is OK (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_missing_generation)]

use std::simd::f32x8;

#[derive(ComputationalCapsule)]
#[capsule(tier = "SIMD", alignment = 256)]
#[repr(C, align(256))]
struct SimdCapsule {
    data: f32x8,
    _padding: [u8; 224],
}

fn main() {}
