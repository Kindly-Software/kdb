//! P0.1 Test: parking_lot::Mutex (FAIL)
//! Expected: CAPSULE_MUTEX_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_mutex_violation)]

extern crate rustc_span;

// Simulate parking_lot::Mutex (test should detect by name)
mod parking_lot {
    pub struct Mutex<T>(std::marker::PhantomData<T>);
}

#[repr(C, align(64))]
struct BadCapsule {
    lock: parking_lot::Mutex<u64>, //~ ERROR: Mutex is forbidden in computational capsules
    _padding: [u8; 56],
}

fn main() {}
