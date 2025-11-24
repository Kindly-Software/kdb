//! P0.1 Test: RwLock usage (FAIL)
//! Expected: CAPSULE_MUTEX_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_mutex_violation)]

extern crate rustc_span;
use std::sync::RwLock;

#[repr(C, align(64))]
struct BadCapsule {
    data: RwLock<u64>, //~ ERROR: RwLock is forbidden in computational capsules
    _padding: [u8; 48],
}

fn main() {}
