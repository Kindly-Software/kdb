//! Compile-fail test: All fields marked skip

use atomic_capsule_derive_serialize::CapsuleSerialize;

// All fields skipped - should fail (no serializable fields)
#[derive(CapsuleSerialize)]
#[repr(C, align(128))]
struct PaymentCapsule {
    #[capsule_serialize(skip)]
    amount: u64,

    #[capsule_serialize(skip)]
    fee: u64,
}

fn main() {}
