//! Compile-fail test: Unknown fixed-point type detection
//!
//! This test verifies that unknown types produce clear error messages with suggestions.

use atomic_capsule::serialize::fixed_point_type_detection::detect_fixed_point_type;

fn main() {
    // This should fail with a helpful error message
    let _result = detect_fixed_point_type("UnknownFixedPointType").unwrap();
    //~^ ERROR: Unknown fixed-point type
}
