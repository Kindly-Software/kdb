//! P0.1 Test: Direct Mutex usage (FAIL)
//! Expected: CAPSULE_MUTEX_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_mutex_violation)]

extern crate rustc_span;
use std::sync::Mutex;

#[repr(C, align(64))]
struct BadCapsule {
    lock: Mutex<u64>, //~ ERROR: Mutex is forbidden in computational capsules
    _padding: [u8; 48],
}

fn main() {}
