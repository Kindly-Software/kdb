//! Alignment too large (> 512) - should fail

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 1024)]  // Too large! Must be <= 512
#[repr(C, align(1024))]
struct TooLarge {
    data: [u8; 1024],
}

fn main() {}
