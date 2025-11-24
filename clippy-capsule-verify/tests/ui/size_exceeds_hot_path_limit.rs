//! # UI Test: Hot Path Capsule Exceeds 128B Limit
//!
//! **Test Objective**: Verify lint fires when HotPath capsule exceeds 128B size limit.
//!
//! **Expected Lint Message**:
//! ```text
//! warning: capsule struct `OversizedHotPathCapsule` exceeds HotPath tier size limit (256 bytes > 128 bytes max)
//!   --> tests/ui/size_exceeds_hot_path_limit.rs:XX:1
//!    |
//! XX | / #[derive(ComputationalCapsule)]
//! XX | | #[capsule(alignment = 64, tier = "HotPath")]
//! XX | | #[repr(C, align(64))]
//! XX | | struct OversizedHotPathCapsule {
//!    | |_^
//!    |
//!    = help: reduce struct size to 128 bytes or use a larger tier
//!    = note: HotPath tier limit: 128 bytes (actual: 256 bytes)
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

// BAD: HotPath tier capsule exceeds 128B limit
#[derive(ComputationalCapsule)]  //~ WARNING: exceeds HotPath tier size limit
#[capsule(alignment = 64, tier = "HotPath")]
#[repr(C, align(64))]
struct OversizedHotPathCapsule {
    data: [AtomicU64; 32],  // 32 × 8 = 256 bytes (exceeds 128B)
}

fn main() {}
