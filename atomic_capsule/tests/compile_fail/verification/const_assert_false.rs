//! Compile-fail test: const_assert with false condition
//!
//! This test verifies that const_assert!(false) causes a compilation error.

use atomic_capsule::verification::const_assert;

fn main() {
    // This should fail to compile
    const_assert!(false);
    //~^ ERROR: assertion failed: false
}
