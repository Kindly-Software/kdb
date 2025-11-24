//! P0.2 Test: 16B struct with 64B alignment needs 48B padding (FAIL)
//! Expected: CAPSULE_UNALIGNED_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_unaligned_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

#[repr(C, align(64))]
struct BadCapsule {
    primary: AtomicU64,   // 8 bytes
    secondary: AtomicU64, // 8 bytes, total 16, missing 48 bytes padding
    //~ ERROR: Capsule size (16) does not match alignment (64). Add _padding: [u8; 48]
}

fn main() {}
