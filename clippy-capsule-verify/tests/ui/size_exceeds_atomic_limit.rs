//! # UI Test: Atomic Capsule Exceeds 256B Limit
//!
//! **Test Objective**: Verify lint fires when T1 (Atomic) capsule exceeds 256B size limit.
//!
//! **Expected Lint Message**:
//! ```text
//! warning: capsule struct `OversizedAtomicCapsule` exceeds Atomic tier size limit (512 bytes > 256 bytes max)
//!   --> tests/ui/size_exceeds_atomic_limit.rs:XX:1
//!    |
//! XX | / #[derive(ComputationalCapsule)]
//! XX | | #[capsule(alignment = 64, tier = "Atomic")]
//! XX | | #[repr(C, align(64))]
//! XX | | struct OversizedAtomicCapsule {
//!    | |_^
//!    |
//!    = help: reduce struct size to 256 bytes or use a larger tier
//!    = note: Atomic tier limit: 256 bytes (actual: 512 bytes)
//!    = note: oversized capsules cause:
//!    = note:   - More cache misses → higher latency
//!    = note:   - Memory bandwidth contention → throughput degradation
//!    = note:   - False sharing across adjacent capsules
//! ```

#![warn(clippy::missing_capsule_verification)]
#![allow(dead_code)]

use core::sync::atomic::AtomicU64;

// Mock derive macro
struct ComputationalCapsule;

// BAD: Atomic tier capsule exceeds 256B limit
#[derive(ComputationalCapsule)]  //~ WARNING: exceeds Atomic tier size limit
#[capsule(alignment = 64, tier = "Atomic")]
#[repr(C, align(64))]
struct OversizedAtomicCapsule {
    data: [AtomicU64; 64],  // 64 × 8 = 512 bytes (exceeds 256B)
}

fn main() {}
