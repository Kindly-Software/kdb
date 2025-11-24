//! P0.2 Test: Correct 128B alignment (PASS)
//! Expected: No errors, compiles successfully

#![feature(rustc_private)]
#![deny(clippy::capsule_unaligned_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

#[repr(C, align(128))]
struct GoodCapsule {
    primary: AtomicU64,
    secondary: AtomicU64,
    _padding: [u8; 112], // Correct: 16 + 112 = 128 bytes
}

fn main() {}
