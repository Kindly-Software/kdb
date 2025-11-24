//! P0.1 Test: parking_lot::RwLock (FAIL)
//! Expected: CAPSULE_MUTEX_VIOLATION error

#![feature(rustc_private)]
#![deny(clippy::capsule_mutex_violation)]

extern crate rustc_span;

// Simulate parking_lot::RwLock
mod parking_lot {
    pub struct RwLock<T>(std::marker::PhantomData<T>);
}

#[repr(C, align(64))]
struct BadCapsule {
    data: parking_lot::RwLock<u64>, //~ ERROR: RwLock is forbidden in computational capsules
    _padding: [u8; 56],
}

fn main() {}
