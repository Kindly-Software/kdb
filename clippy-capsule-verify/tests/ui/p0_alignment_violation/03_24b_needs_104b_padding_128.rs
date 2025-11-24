//! P0.2 Test: 24B struct with 128B alignment needs 104B padding (FAIL)
//! Expected: CAPSULE_UNALIGNED_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_unaligned_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

#[repr(C, align(128))]
struct BadCapsule {
    field1: AtomicU64, // 8 bytes
    field2: AtomicU64, // 8 bytes
    field3: AtomicU64, // 8 bytes, total 24, missing 104 bytes padding
    //~ ERROR: Capsule size (24) does not match alignment (128). Add _padding: [u8; 104]
}

fn main() {}
