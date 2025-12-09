//! Compile-fail test: Missing #[repr(C, align(N))]

use atomic_capsule_derive_serialize::CapsuleSerialize;

// Mock types
struct Q16_16 {
    raw: i64,
}

// Missing #[repr(C, align(N))] - should fail
#[derive(CapsuleSerialize)]
struct PaymentCapsule {
    amount: Q16_16,
    fee: Q16_16,
}

fn main() {}
