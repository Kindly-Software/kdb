//! Compile-fail test: Alignment mismatch detection
//!
//! This test verifies that verify_capsule! macro catches alignment mismatches at compile-time.

use atomic_capsule::verify_capsule;

#[repr(C, align(32))] // Wrong alignment! Should be 64
struct BadAlignmentCapsule {
    data: [u8; 64],
}

fn main() {
    // This should fail to compile: actual alignment is 32, expected 64
    verify_capsule!(BadAlignmentCapsule, 64, 64);
}
