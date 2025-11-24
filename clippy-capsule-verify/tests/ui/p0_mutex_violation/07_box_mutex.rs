//! P0.1 Test: Box<Mutex<T>> wrapper (FAIL)
//! Expected: CAPSULE_MUTEX_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_mutex_violation)]

extern crate rustc_span;
use std::sync::Mutex;

#[repr(C, align(64))]
struct BadCapsule {
    boxed: Box<Mutex<u64>>, //~ ERROR: Mutex is forbidden in computational capsules
    _padding: [u8; 56],
}

fn main() {}
