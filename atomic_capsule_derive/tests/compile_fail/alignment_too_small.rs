//! Alignment too small (< 32) - should fail

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 16)]  // Too small! Must be >= 32
#[repr(C, align(16))]
struct TooSmall {
    data: [u8; 16],
}

fn main() {}
