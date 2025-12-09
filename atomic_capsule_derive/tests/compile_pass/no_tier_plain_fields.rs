//! Test: No tier specified with plain fields (no inference)
//!
//! T28 Q2 (Edge Cases): Cannot infer tier from plain data
//! UCE34 Q10: Tier inference requires recognizable types
//!
//! Expected: Compilation succeeds (no tier inferred, no error)

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]  // No tier, plain fields
#[repr(C, align(64))]
struct PlainData {
    data: [u8; 64],
}

fn main() {
    let capsule = PlainData {
        data: [0u8; 64],
    };

    println!("No tier inferred from plain byte array!");
}
