//! Compile-fail test: assert_eq_size with mismatched types
//!
//! This test verifies that assert_eq_size!(u32, u64) causes a compilation error.

use atomic_capsule::verification::assert_eq_size;

fn main() {
    // This should fail to compile (u32 is 4 bytes, u64 is 8 bytes)
    assert_eq_size!(u32, u64);
    //~^ ERROR: Size mismatch
}
