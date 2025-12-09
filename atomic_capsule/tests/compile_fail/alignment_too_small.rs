//! Compile-fail test: Alignment too small detection
//!
//! This test verifies that verify_alignment! macro catches alignments below minimum (64 bytes).

use atomic_capsule::verify_alignment;

#[repr(C, align(32))]
struct TooSmallCapsule {
    data: [u8; 32],
}

fn main() {
    // This should fail to compile: 32 bytes is below minimum of 64
    verify_alignment!(TooSmallCapsule, 32);
}
