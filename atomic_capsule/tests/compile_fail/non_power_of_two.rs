//! Compile-fail test: Non-power-of-two alignment detection
//!
//! This test verifies that verify_alignment! macro catches non-power-of-two alignments.

use atomic_capsule::verify_alignment;

#[repr(C, align(64))]
struct ValidCapsule {
    data: [u8; 64],
}

fn main() {
    // This should fail to compile: 48 is not a power of 2
    verify_alignment!(ValidCapsule, 48);
}
