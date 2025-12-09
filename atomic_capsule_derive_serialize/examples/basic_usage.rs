//! Basic usage example for #[derive(CapsuleSerialize)]

// NOTE: This is a demonstration. Real usage requires atomic_capsule crate.

use atomic_capsule_derive_serialize::CapsuleSerialize;

// Mock fixed-point type (real version in atomic_capsule)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Q16_16 {
    raw: i64,
}

impl Q16_16 {
    fn new(value: f64) -> Self {
        Self {
            raw: (value * 65536.0) as i64,
        }
    }

    fn raw_value(&self) -> i64 {
        self.raw
    }

    fn from_raw(raw: i64) -> Self {
        Self { raw }
    }

    fn to_decimal_string(&self) -> String {
        let value = self.raw as f64 / 65536.0;
        format!("${:.2}", value)
    }
}

impl Default for Q16_16 {
    fn default() -> Self {
        Self { raw: 0 }
    }
}

// Mock trait (real version in atomic_capsule)
trait FixedPointSerialize {
    fn serialize_binary(&self) -> Vec<u8>;
    fn deserialize_binary(data: &[u8]) -> Result<Self, SerializeError>
    where
        Self: Sized;
    fn to_decimal_string(&self) -> String;
    fn compute_hash(&self) -> u64;
}

// Mock error type
#[derive(Debug)]
enum SerializeError {
    InvalidHeader,
    InvalidMagic,
    UnsupportedVersion,
    InvalidPayloadSize,
    InvalidPayload,
    HashMismatch,
}

// Example 1: Basic payment capsule
#[derive(CapsuleSerialize)]
#[repr(C, align(128))]
struct PaymentCapsule {
    amount: Q16_16,
    fee: Q16_16,
}

// Example 2: Capsule with skip field
#[derive(CapsuleSerialize)]
#[repr(C, align(256))]
struct PaymentCapsuleWithId {
    amount: Q16_16,
    fee: Q16_16,

    // Internal ID not serialized
    #[capsule_serialize(skip)]
    internal_id: u64,
}

// Example 3: Capsule with hash_key field
#[derive(CapsuleSerialize)]
#[repr(C, align(256))]
struct PaymentCapsuleWithAudit {
    amount: Q16_16,
    fee: Q16_16,

    // Audit key included in hash but not serialized
    #[capsule_serialize(hash_key)]
    audit_key: u64,
}

fn main() {
    println!("=== CapsuleSerialize Examples ===\n");

    // Example 1: Basic payment
    let payment = PaymentCapsule {
        amount: Q16_16::new(100.00),
        fee: Q16_16::new(2.50),
    };

    println!("Example 1: Basic Payment");
    println!("  Decimal: {}", payment.to_decimal_string());
    println!("  Hash: 0x{:016x}", payment.compute_hash());

    let binary = payment.serialize_binary();
    println!("  Binary size: {} bytes (22 header + 16 payload)", binary.len());

    // Example 2: Payment with ID
    let payment_with_id = PaymentCapsuleWithId {
        amount: Q16_16::new(100.00),
        fee: Q16_16::new(2.50),
        internal_id: 12345,  // Not serialized
    };

    println!("\nExample 2: Payment with Internal ID (skip)");
    println!("  Decimal: {}", payment_with_id.to_decimal_string());
    println!("  Hash: 0x{:016x}", payment_with_id.compute_hash());
    println!("  Internal ID: {} (not in serialization)", payment_with_id.internal_id);

    let binary_with_id = payment_with_id.serialize_binary();
    println!("  Binary size: {} bytes (same as Example 1)", binary_with_id.len());

    // Example 3: Payment with audit key
    let payment_with_audit = PaymentCapsuleWithAudit {
        amount: Q16_16::new(100.00),
        fee: Q16_16::new(2.50),
        audit_key: 0xDEADBEEF,  // In hash, not serialized
    };

    println!("\nExample 3: Payment with Audit Key (hash_key)");
    println!("  Decimal: {}", payment_with_audit.to_decimal_string());
    println!("  Hash: 0x{:016x}", payment_with_audit.compute_hash());
    println!("  Audit key: 0x{:016x} (in hash only)", payment_with_audit.audit_key);

    let binary_with_audit = payment_with_audit.serialize_binary();
    println!("  Binary size: {} bytes (same as Example 1)", binary_with_audit.len());

    println!("\n=== Binary Format Details ===");
    println!("Header (22 bytes):");
    println!("  - Magic: 0x43505346 ('CPSF')");
    println!("  - Version: 0x0001");
    println!("  - Payload size: {} bytes", binary.len() - 22);
    println!("  - Hash: 0x{:016x}", payment.compute_hash());
    println!("\nPayload (16 bytes):");
    println!("  - amount (8 bytes): {} raw", payment.amount.raw_value());
    println!("  - fee (8 bytes): {} raw", payment.fee.raw_value());
}
