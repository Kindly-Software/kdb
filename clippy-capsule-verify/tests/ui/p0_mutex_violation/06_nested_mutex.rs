//! P0.1 Test: Nested Mutex in Option (FAIL)
//! Expected: CAPSULE_MUTEX_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_mutex_violation)]

extern crate rustc_span;
use std::sync::Mutex;

#[repr(C, align(64))]
struct BadCapsule {
    optional_lock: Option<Mutex<u64>>, //~ ERROR: Mutex is forbidden in computational capsules
    _padding: [u8; 40],
}

fn main() {}
