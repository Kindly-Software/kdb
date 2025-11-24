//! P0.2 Test: Correct 256B alignment (PASS)
//! Expected: No errors, compiles successfully

#![feature(rustc_private)]
#![deny(clippy::capsule_unaligned_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

#[repr(C, align(256))]
struct GoodCapsule {
    data: [AtomicU64; 16], // 128 bytes
    _padding: [u8; 128],   // Correct: 128 + 128 = 256 bytes
}

fn main() {}
