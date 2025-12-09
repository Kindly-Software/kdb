//! Compile-fail test: Alignment too large detection
//!
//! This test verifies that verify_alignment! macro catches alignments above maximum (256 bytes).

use atomic_capsule::verify_alignment;

#[repr(C, align(512))]
struct TooLargeCapsule {
    data: [u8; 512],
}

fn main() {
    // This should fail to compile: 512 bytes is above maximum of 256
    verify_alignment!(TooLargeCapsule, 512);
}
