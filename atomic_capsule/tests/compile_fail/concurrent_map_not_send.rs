//! Compile-fail test: ConcurrentMapCapsule rejects !Send types
//!
//! This test validates that ConcurrentMapCapsule enforces Send + Sync bounds
//! at compile time, preventing thread-unsafe types from being used.

use atomic_capsule::collections::ConcurrentMapCapsule;
use std::rc::Rc;

fn main() {
    // Rc<String> is !Send (not thread-safe)
    let map: ConcurrentMapCapsule<u64, Rc<String>> = ConcurrentMapCapsule::new();
    //~^ ERROR `Rc<String>` cannot be sent between threads safely

    map.insert(1, Rc::new("hello".to_string()));
}
