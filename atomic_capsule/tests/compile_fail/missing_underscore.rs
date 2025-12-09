//! Compile-fail test: Missing underscore detection
//!
//! This test verifies that missing underscores produce helpful suggestions.

use atomic_capsule::serialize::fixed_point_type_detection::detect_fixed_point_type;

fn main() {
    // Missing underscore: "Q1616" should be "Q16_16"
    let _result = detect_fixed_point_type("Q1616").unwrap();
    //~^ ERROR: Unknown fixed-point type
    //~| HELP: Did you mean one of these?
    //~| HELP:   - Q16_16
}
