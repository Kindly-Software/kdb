//! Missing #[capsule(...)] attribute - should fail

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[repr(C, align(64))]
struct MissingAttr {
    data: [u8; 64],
}

fn main() {}
