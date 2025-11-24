//! P0.1 Test: Arc<Mutex<T>> wrapper (FAIL)
//! Expected: CAPSULE_MUTEX_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_mutex_violation)]

extern crate rustc_span;
use std::sync::{Arc, Mutex};

#[repr(C, align(64))]
struct BadCapsule {
    shared: Arc<Mutex<u64>>, //~ ERROR: Mutex is forbidden in computational capsules
    _padding: [u8; 48],
}

fn main() {}
