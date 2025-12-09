//! Size zero - should fail

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 0)]  // Invalid! Must be non-zero
#[repr(C, align(64))]
struct ZeroSize;

fn main() {}
