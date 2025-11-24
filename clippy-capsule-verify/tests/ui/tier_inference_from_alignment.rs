//! # UI Test: Tier Inference from Alignment
//!
//! **Test Objective**: Verify tier is correctly inferred from #[repr(C, align(N))] when
//! #[capsule(tier = "...")] is not explicitly specified.
//!
//! **Expected**: Lint warns if inferred tier size limit is exceeded.

#![warn(clippy::missing_capsule_verification)]
#![allow(dead_code)]

use core::sync::atomic::AtomicU64;

// Mock derive macro
struct ComputationalCapsule;

// Tier inference: align(64) → Atomic tier (heuristic)
// Expected: Should check Atomic tier limit (256B)
#[derive(ComputationalCapsule)]
#[repr(C, align(64))]
struct InferredAtomicCapsule {
    data: [AtomicU64; 16],  // 16 × 8 = 128 bytes (within 256B, OK)
}

// Tier inference: align(128) → Atomic tier (heuristic)
// Expected: Should check Atomic tier limit (256B)
#[derive(ComputationalCapsule)]  //~ WARNING: might exceed limit if >256B
#[repr(C, align(128))]
struct InferredAtomicCapsuleOversized {
    data: [AtomicU64; 48],  // 48 × 8 = 384 bytes (exceeds 256B, BAD)
}

fn main() {}
