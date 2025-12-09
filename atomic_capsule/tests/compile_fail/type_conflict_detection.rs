//! Compile-fail test: Type conflict detection
//!
//! This test verifies that mixing different fixed-point types produces clear error messages.

use atomic_capsule::serialize::fixed_point_type_detection::{check_type_conflict, FixedPointType};

fn main() {
    // This should fail with type conflict error
    check_type_conflict(FixedPointType::Q8_8, FixedPointType::Q16_16, "amount").unwrap();
    //~^ ERROR: Fixed-point type conflict
}
