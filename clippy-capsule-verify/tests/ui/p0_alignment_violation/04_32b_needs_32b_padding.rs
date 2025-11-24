//! P0.2 Test: 32B struct with 64B alignment needs 32B padding (FAIL)
//! Expected: CAPSULE_UNALIGNED_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_unaligned_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

#[repr(C, align(64))]
struct BadCapsule {
    field1: AtomicU64, // 8 bytes
    field2: AtomicU64, // 8 bytes
    field3: AtomicU64, // 8 bytes
    field4: AtomicU64, // 8 bytes, total 32, missing 32 bytes padding
    //~ ERROR: Capsule size (32) does not match alignment (64). Add _padding: [u8; 32]
}

fn main() {}
