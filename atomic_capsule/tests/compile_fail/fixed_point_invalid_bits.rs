//! Compile-fail test: Invalid fractional bits detection
//!
//! This test verifies that verify_fixed_point_capsule! macro catches invalid fractional bits.

use atomic_capsule::verify_fixed_point_capsule;

#[repr(C, align(64))]
struct FixedPointCapsule {
    price: u16,
}

fn main() {
    // This should fail to compile: 0 fractional bits is invalid (must be 1..32)
    verify_fixed_point_capsule!(FixedPointCapsule, 64, 0);
}
