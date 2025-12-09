//! Compile-fail test: assert_size with wrong expected size
//!
//! This test verifies that assert_size!(u64, 4) causes a compilation error.

use atomic_capsule::verification::assert_size;

fn main() {
    // This should fail to compile (u64 is 8 bytes, not 4)
    assert_size!(u64, 4);
    //~^ ERROR: Size mismatch
}
