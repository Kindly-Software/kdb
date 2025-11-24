//! P0.1 Test: Multiple valid atomics (PASS)
//! Expected: No errors, compiles successfully

#![feature(rustc_private)]
#![deny(clippy::capsule_mutex_violation)]

extern crate rustc_span;
use std::sync::atomic::{AtomicU64, AtomicU32, AtomicU16};

#[repr(C, align(64))]
struct GoodCapsule {
    counter: AtomicU64,
    state: AtomicU32,
    flags: AtomicU16,
    _padding: [u8; 42],
}

fn main() {}
