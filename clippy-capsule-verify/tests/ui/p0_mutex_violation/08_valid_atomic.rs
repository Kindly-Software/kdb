//! P0.1 Test: Valid AtomicU64 usage (PASS)
//! Expected: No errors, compiles successfully

#![feature(rustc_private)]
#![deny(clippy::capsule_mutex_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

#[repr(C, align(64))]
struct GoodCapsule {
    counter: AtomicU64,
    _padding: [u8; 56],
}

fn main() {}
