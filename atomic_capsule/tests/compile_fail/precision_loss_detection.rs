//! Compile-fail test: Precision loss detection
//!
//! This test verifies that unsafe downcasts produce clear warning messages.

use atomic_capsule::serialize::fixed_point_type_detection::{check_precision_loss, FixedPointType};

fn main() {
    // This should fail with precision loss warning
    check_precision_loss(FixedPointType::Q32_32, FixedPointType::Q8_8, "downcast").unwrap();
    //~^ ERROR: Unsafe precision loss
}
