//! P0.2 Test: Wrong padding size (FAIL)
//! Expected: CAPSULE_UNALIGNED_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_unaligned_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

#[repr(C, align(64))]
struct BadCapsule {
    counter: AtomicU64, // 8 bytes
    _padding: [u8; 32], // Wrong! Should be 56 bytes, total = 40 instead of 64
    //~ ERROR: Capsule size (40) does not match alignment (64). Add _padding: [u8; 24]
}

fn main() {}
