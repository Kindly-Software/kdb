//! P0.2 Test: 8B struct with 64B alignment needs 56B padding (FAIL)
//! Expected: CAPSULE_UNALIGNED_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_unaligned_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

#[repr(C, align(64))]
struct BadCapsule {
    counter: AtomicU64, // 8 bytes, missing 56 bytes padding
    //~ ERROR: Capsule size (8) does not match alignment (64). Add _padding: [u8; 56]
}

fn main() {}
