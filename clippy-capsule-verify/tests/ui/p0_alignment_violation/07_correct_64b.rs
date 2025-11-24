//! P0.2 Test: Correct 64B alignment (PASS)
//! Expected: No errors, compiles successfully

#![feature(rustc_private)]
#![deny(clippy::capsule_unaligned_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

#[repr(C, align(64))]
struct GoodCapsule {
    counter: AtomicU64,
    _padding: [u8; 56], // Correct: 8 + 56 = 64 bytes
}

fn main() {}
