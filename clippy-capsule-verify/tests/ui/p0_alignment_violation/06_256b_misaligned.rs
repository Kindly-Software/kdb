//! P0.2 Test: 256B alignment with insufficient size (FAIL)
//! Expected: CAPSULE_UNALIGNED_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_unaligned_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

#[repr(C, align(256))]
struct BadCapsule {
    data: [AtomicU64; 16], // 128 bytes, missing 128 bytes padding for 256B alignment
    //~ ERROR: Capsule size (128) does not match alignment (256). Add _padding: [u8; 128]
}

fn main() {}
