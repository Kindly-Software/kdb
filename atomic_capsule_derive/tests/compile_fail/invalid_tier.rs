//! Invalid tier name - should fail

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, tier = "InvalidTier")]  // Not a valid UCE33 tier!
#[repr(C, align(64))]
struct BadTier {
    data: [u8; 64],
}

fn main() {}
