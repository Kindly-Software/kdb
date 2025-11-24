//! P0.2 Test: Correct DualAtomicU64 pattern with 64B alignment (PASS)
//! Expected: No errors, compiles successfully

#![feature(rustc_private)]
#![deny(clippy::capsule_unaligned_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

#[repr(C, align(64))]
struct GoodCapsule {
    primary: AtomicU64,   // 8 bytes
    secondary: AtomicU64, // 8 bytes
    _padding: [u8; 48],   // Correct: 16 + 48 = 64 bytes
}

fn main() {}
