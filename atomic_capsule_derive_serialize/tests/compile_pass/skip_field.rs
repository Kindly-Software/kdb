//! Compile-pass test: Capsule with #[capsule_serialize(skip)]

use atomic_capsule_derive_serialize::CapsuleSerialize;

// Mock types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Q16_16 {
    raw: i64,
}

impl Q16_16 {
    fn raw_value(&self) -> i64 {
        self.raw
    }

    fn from_raw(raw: i64) -> Self {
        Self { raw }
    }

    fn to_decimal_string(&self) -> String {
        format!("{}", self.raw)
    }
}

impl Default for Q16_16 {
    fn default() -> Self {
        Self { raw: 0 }
    }
}

trait FixedPointSerialize {
    fn serialize_binary(&self) -> Vec<u8>;
    fn deserialize_binary(data: &[u8]) -> Result<Self, SerializeError>
    where
        Self: Sized;
    fn to_decimal_string(&self) -> String;
    fn compute_hash(&self) -> u64;
}

#[derive(Debug)]
enum SerializeError {
    InvalidHeader,
    InvalidMagic,
    UnsupportedVersion,
    InvalidPayloadSize,
    InvalidPayload,
    HashMismatch,
}

// Test capsule with skip field
#[derive(CapsuleSerialize)]
#[repr(C, align(128))]
struct PaymentCapsule {
    amount: Q16_16,
    fee: Q16_16,

    #[capsule_serialize(skip)]
    internal_id: u64,
}

fn main() {
    println!("Compile-pass: skip_field");
}
