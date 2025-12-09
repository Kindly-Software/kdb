//! Compile-fail test: assert_pow2_size with non-power-of-2 size
//!
//! This test verifies that assert_pow2_size!([u8; 3]) causes a compilation error.

use atomic_capsule::verification::assert_pow2_size;

fn main() {
    // This should fail to compile ([u8; 3] is 3 bytes, not a power of 2)
    assert_pow2_size!([u8; 3]);
    //~^ ERROR: is not a power of 2
}
