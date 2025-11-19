//! Compile-fail test: Transparent requires exactly 1 field
//!
//! #[capsule_serialize(transparent)] should fail if struct has multiple fields

use atomic_capsule_derive_serialize::CapsuleSerialize;

#[derive(Debug, Clone, Copy)]
struct Q16_16 {
    raw: i64,
}

impl Q16_16 {
    fn raw_value(&self) -> i64 {
        self.raw
    }
}

impl Default for Q16_16 {
    fn default() -> Self {
        Self { raw: 0 }
    }
}

// This should FAIL: transparent with 2 fields
#[derive(CapsuleSerialize)]
#[capsule_serialize(transparent)]
#[repr(C, align(64))]
struct TwoFieldsStruct {
    amount: Q16_16,
    fee: Q16_16,
}

fn main() {}
