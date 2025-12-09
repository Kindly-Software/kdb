//! Compile-fail test: Invalid field type (f64 instead of fixed-point)

use atomic_capsule_derive_serialize::CapsuleSerialize;

// Invalid field type (f64) - should fail
#[derive(CapsuleSerialize)]
#[repr(C, align(128))]
struct PaymentCapsule {
    amount: f64,  // ERROR: Should be Q8_8, Q16_16, or Q32_32
    fee: f64,
}

fn main() {}
