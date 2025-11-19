//! Compile-fail test: Transparent field cannot be marked #[skip]
//!
//! The transparent field itself cannot have conflicting attributes like skip

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

// This should FAIL: transparent field marked with #[skip]
#[derive(CapsuleSerialize)]
#[capsule_serialize(transparent)]
#[repr(C, align(64))]
struct TransparentWithSkip {
    #[capsule_serialize(skip)]
    amount: Q16_16,
}

fn main() {}
