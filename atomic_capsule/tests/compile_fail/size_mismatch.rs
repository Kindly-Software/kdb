//! Compile-fail test: Size mismatch detection
//!
//! This test verifies that verify_capsule! macro catches size mismatches at compile-time.

use atomic_capsule::verify_capsule;

#[repr(C, align(64))]
struct WrongSizeCapsule {
    data: [u8; 32], // Actual size is 32 bytes
}

fn main() {
    // This should fail to compile: actual size is 32, expected 64
    verify_capsule!(WrongSizeCapsule, 64, 64);
}
