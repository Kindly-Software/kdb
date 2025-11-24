// P0.4 Test 7: T2 SIMD capsule with u64 field is OK (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_non_atomic_field)]

use std::simd::f32x8;

#[derive(ComputationalCapsule)]
#[capsule(tier = "SIMD", alignment = 256)]
#[repr(C, align(256))]
struct SimdWithU64Capsule {
    data: f32x8,
    count: u64,
    _padding: [u8; 216],
}

fn main() {}
