//! Compile-fail test: Fuzzy matching for typos
//!
//! This test verifies that typos in type names produce helpful suggestions.

use atomic_capsule::serialize::fixed_point_type_detection::detect_fixed_point_type;

fn main() {
    // Typo: "Q16_15" should be "Q16_16"
    let _result = detect_fixed_point_type("Q16_15").unwrap();
    //~^ ERROR: Unknown fixed-point type
    //~| HELP: Did you mean one of these?
    //~| HELP:   - Q16_16
}
