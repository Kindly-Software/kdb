//! Compile-fail test: DualAtomicU64 wrong alignment detection
//!
//! This test verifies that verify_dual_atomic_u64! macro requires 128-byte alignment.

use atomic_capsule::verify_dual_atomic_u64;
use core::sync::atomic::AtomicU64;

#[repr(C, align(64))] // Wrong! DualAtomicU64 requires 128-byte alignment
struct WrongDualCapsule {
    primary: AtomicU64,
    secondary: AtomicU64,
}

fn main() {
    // This should fail to compile: DualAtomicU64 requires 128-byte alignment
    verify_dual_atomic_u64!(WrongDualCapsule);
}
