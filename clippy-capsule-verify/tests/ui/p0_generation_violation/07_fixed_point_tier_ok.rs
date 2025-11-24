// P0.3 Test 7: T3 Fixed-Point capsule without generation is OK (PASS)
#![feature(rustc_private)]
#![deny(clippy::capsule_missing_generation)]

#[derive(ComputationalCapsule)]
#[capsule(tier = "Fixed-Point", alignment = 64)]
#[repr(C, align(64))]
struct FixedPointCapsule {
    value_q16_16: u32,
    multiplier_q8_8: u16,
    _padding: [u8; 58],
}

fn main() {}
