//! Test: Empty struct with zero fields
//!
//! T28 Q2 (Edge Cases): Testing empty/minimal structures
//! UCE34 Q33: Capsules should contain data
//!
//! Expected: Compilation succeeds (Rust allows empty structs)
//! Note: Empty struct has size=0 which will fail size assertion

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]  // Claims 64 bytes...
#[repr(C, align(64))]
struct EmptyStruct {
    // ❌ No fields! Actual size will be 0
}

fn main() {}
