//! Test: Mismatched alignment between capsule attr and repr should fail
//!
//! T28 Q2 (Edge Cases): Testing alignment validation
//! UCE34 Q33: Alignment must match between #[capsule] and #[repr]
//!
//! Expected: Compilation error - alignment mismatch (64 vs 128)

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]  // Says 64 bytes
#[repr(C, align(128))]                  // ❌ Actually 128 bytes - MISMATCH
struct AlignmentMismatch {
    state: AtomicU64,
    _padding: [u8; 56],
}

fn main() {}
