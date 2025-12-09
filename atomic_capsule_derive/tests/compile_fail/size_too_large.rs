//! Size too large (> 1MB) - should fail

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 2097152)]  // 2MB - too large!
#[repr(C, align(64))]
struct TooLarge {
    data: [u8; 64],
}

fn main() {}
