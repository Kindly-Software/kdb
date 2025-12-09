//! Alignment not power of 2 - should fail

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 100)]  // Not power of 2!
#[repr(C, align(128))]
struct BadAlignment {
    data: [u8; 128],
}

fn main() {}
