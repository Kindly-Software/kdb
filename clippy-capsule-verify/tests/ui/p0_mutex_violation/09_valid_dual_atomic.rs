//! P0.1 Test: Valid DualAtomicU64 usage (PASS)
//! Expected: No errors, compiles successfully

#![feature(rustc_private)]
#![deny(clippy::capsule_mutex_violation)]

extern crate rustc_span;
use std::sync::atomic::AtomicU64;

// Simulate DualAtomicU64 pattern
#[repr(C, align(64))]
struct DualAtomicU64 {
    primary: AtomicU64,
    secondary: AtomicU64,
}

#[repr(C, align(64))]
struct GoodCapsule {
    state: DualAtomicU64,
    _padding: [u8; 48],
}

fn main() {}
