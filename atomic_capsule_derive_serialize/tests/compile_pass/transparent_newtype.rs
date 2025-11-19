//! Compile-pass test: Transparent newtype serialization
//!
//! Verifies that #[capsule_serialize(transparent)] generates correct delegation
//! to inner field's serialization methods.

use atomic_capsule_derive_serialize::CapsuleSerialize;

// Mock types for testing (real types would come from atomic_capsule)
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

// Implement FixedPointSerialize for Q16_16 (for delegation testing)
impl FixedPointSerialize for Q16_16 {
    fn serialize_binary(&self) -> Vec<u8> {
        vec![0xC, 0xA, 0xF, 0xE]
    }

    fn deserialize_binary(data: &[u8]) -> Result<Self, SerializeError> {
        Ok(Q16_16::from_raw(42))
    }

    fn to_decimal_string(&self) -> String {
        self.to_decimal_string()
    }

    fn compute_hash(&self) -> u64 {
        0xDEADBEEF
    }
}

// Test 1: Transparent newtype with fixed-point inner type
#[derive(CapsuleSerialize)]
#[capsule_serialize(transparent)]
#[repr(C, align(64))]
struct UserId(u64);

// Test 2: Transparent newtype with Q16_16
#[derive(CapsuleSerialize)]
#[capsule_serialize(transparent)]
#[repr(C, align(64))]
struct Amount(Q16_16);

// Test 3: Transparent with custom type (as long as it implements FixedPointSerialize)
#[derive(CapsuleSerialize)]
#[capsule_serialize(transparent)]
#[repr(C, align(64))]
struct Identifier(u64);

// Test 4: Transparent newtype with String (if String implements FixedPointSerialize)
// This demonstrates that transparent works with any type, not just fixed-point
#[derive(CapsuleSerialize)]
#[capsule_serialize(transparent)]
#[repr(C, align(64))]
struct Name(String);

// Implement FixedPointSerialize for String for testing
impl FixedPointSerialize for String {
    fn serialize_binary(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    fn deserialize_binary(data: &[u8]) -> Result<Self, SerializeError> {
        Ok(String::from_utf8_lossy(data).into_owned())
    }

    fn to_decimal_string(&self) -> String {
        self.clone()
    }

    fn compute_hash(&self) -> u64 {
        0xCAFEBABE
    }
}

// Implement FixedPointSerialize for u64 (for testing)
impl FixedPointSerialize for u64 {
    fn serialize_binary(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }

    fn deserialize_binary(data: &[u8]) -> Result<Self, SerializeError> {
        if data.len() < 8 {
            return Err(SerializeError::InvalidPayloadSize);
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&data[..8]);
        Ok(u64::from_le_bytes(bytes))
    }

    fn to_decimal_string(&self) -> String {
        self.to_string()
    }

    fn compute_hash(&self) -> u64 {
        *self
    }
}

fn main() {
    // This test just needs to compile
    // The transparent derive macro should generate FixedPointSerialize implementations
    // that delegate to the inner field's implementations
    println!("Compile-pass: transparent_newtype");

    // Verify the types are created correctly
    let user_id = UserId(123);
    let amount = Amount(Q16_16::from_raw(456));
    let identifier = Identifier(789);
    let name = Name("Alice".to_string());

    // The generated impl should allow serialization delegation
    println!("UserId: {:?}", user_id);
    println!("Amount: {:?}", amount);
    println!("Identifier: {:?}", identifier);
    println!("Name: {:?}", name);
}
