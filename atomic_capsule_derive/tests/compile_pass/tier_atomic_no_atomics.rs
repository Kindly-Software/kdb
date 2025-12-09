//! Test: Tier "Atomic" specified but no atomic fields present
//!
//! T28 Q2 (Edge Cases): Testing tier validation against field types
//! UCE34 Q10: Atomic tier requires atomic field types
//!
//! Expected: Compilation succeeds (tier is metadata, doesn't enforce field types)
//! Note: This is a pass case demonstrating tier documentation vs enforcement

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, tier = "Atomic")]
#[repr(C, align(64))]
struct MislabeledTier {
    // No atomic fields - just plain data
    data: [u8; 64],
}

fn main() {
    // This compiles - tier is documentation, not enforcement
    let _capsule = MislabeledTier {
        data: [0u8; 64],
    };
}
