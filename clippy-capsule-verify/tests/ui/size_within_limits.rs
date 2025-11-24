//! # UI Test: Capsule Within Size Limits
//!
//! **Test Objective**: Verify NO lint fires when capsule is within tier size limits.
//!
//! **Expected**: No warnings (compile-pass)

#![warn(clippy::missing_capsule_verification)]
#![allow(dead_code)]

use core::sync::atomic::AtomicU64;

// Mock derive macro
struct ComputationalCapsule;

// GOOD: Atomic tier capsule within 256B limit (128B actual)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 128, tier = "Atomic")]
#[repr(C, align(64))]
struct AtomicCapsule128 {
    state: AtomicU64,
    _padding: [u8; 120],
}

// GOOD: HotPath tier capsule within 128B limit (64B actual)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, tier = "HotPath")]
#[repr(C, align(64))]
struct HotPathCapsule64 {
    state: AtomicU64,
    _padding: [u8; 56],
}

// GOOD: SIMD tier capsule within 512B limit (256B actual)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 256, tier = "SIMD")]
#[repr(C, align(64))]
struct SimdCapsule256 {
    data: [AtomicU64; 32],  // 32 × 8 = 256 bytes (within 512B limit)
}

fn main() {}
