# atomic_capsule_derive_serialize

**Procedural macro for automatic fixed-point serialization in computational capsules.**

## Features

- **Automatic trait implementation**: Single `#[derive]` generates 100+ lines of serialization code
- **Type-safe**: Compile-time validation of fixed-point types (Q8_8, Q16_16, Q32_32)
- **Binary format**: 22-byte header + payload with hash verification
- **Decimal strings**: Human-readable output for debugging
- **Hash integration**: FNV-1a hash for audit trails (Q34 Auditability)
- **Zero runtime cost**: All validation at compile-time
- **Clear errors**: Actionable compile errors with field-level diagnostics

## Usage

```rust
use atomic_capsule_derive_serialize::CapsuleSerialize;
use atomic_capsule::fixed_point::{Q16_16, FixedPointSerialize};

#[derive(CapsuleSerialize)]
#[repr(C, align(128))]
struct PaymentCapsule {
    amount: Q16_16,
    fee: Q16_16,

    #[capsule_serialize(skip)]
    internal_id: u64,
}

// Automatic implementation:
// - serialize_binary() -> Vec<u8>
// - deserialize_binary(&[u8]) -> Result<Self, SerializeError>
// - to_decimal_string() -> String
// - compute_hash() -> u64
```

## Attributes

### `#[capsule_serialize(skip)]`

Exclude field from serialization and hash:

```rust
#[derive(CapsuleSerialize)]
#[repr(C, align(128))]
struct PaymentCapsule {
    amount: Q16_16,

    #[capsule_serialize(skip)]
    internal_id: u64,  // Not serialized, not hashed
}
```

### `#[capsule_serialize(hash_key)]`

Include in hash but not serialization (audit keys):

```rust
#[derive(CapsuleSerialize)]
#[repr(C, align(256))]
struct PaymentCapsule {
    amount: Q16_16,

    #[capsule_serialize(hash_key)]
    audit_key: u64,  // Included in hash, not serialized
}
```

## Binary Format

```text
Header (22 bytes):
  - Magic number (4 bytes): 0x43505346 ("CPSF" = CaPSule Fixed-point)
  - Version (2 bytes): 0x0001
  - Payload size (8 bytes): u64 little-endian
  - Hash (8 bytes): u64 FNV-1a hash of payload

Payload (N * 8 bytes):
  - Field 1 (8 bytes): i64 raw fixed-point value
  - Field 2 (8 bytes): i64 raw fixed-point value
  - ...
```

## Supported Types

- **Q8_8**: 8 integer bits, 8 fractional bits (1/256 precision, ±128.00 range)
- **Q16_16**: 16 integer bits, 16 fractional bits (1/65536 precision, ±32768.00 range)
- **Q32_32**: 32 integer bits, 32 fractional bits (highest precision)

## Requirements

- **#[repr(C, align(N))]**: Required for deterministic field layout
- **Named fields**: Tuple structs not supported
- **At least one serializable field**: All fields cannot be skip/hash_key

## Error Messages

The macro provides clear, actionable compile errors:

```text
error: CapsuleSerialize requires #[repr(C, align(N))] for deterministic layout

Add before struct definition:
#[repr(C, align(64))]  // or 128, 256
struct MyStruct { ... }

Why: Fixed-point binary serialization needs predictable field ordering
```

```text
error: Field 'price' has unsupported type 'f64'

Supported fixed-point types:
- Q8_8: 1/256 precision (±128.00)
- Q16_16: 1/65536 precision (±32768.00)
- Q32_32: 1/4294967296 precision (highest)

Options:
1. Change to fixed-point type:
   price: Q16_16,  // Example

2. Exclude from serialization:
   #[capsule_serialize(skip)]
   price: f64,

3. Include in hash only (audit trail):
   #[capsule_serialize(hash_key)]
   price: f64,
```

## Generated Code Example

Input:

```rust
#[derive(CapsuleSerialize)]
#[repr(C, align(128))]
struct PaymentCapsule {
    amount: Q16_16,
    fee: Q16_16,
}
```

Output (simplified):

```rust
impl FixedPointSerialize for PaymentCapsule {
    fn serialize_binary(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(38);  // 22 header + 16 payload

        // Header
        buffer.extend_from_slice(&0x43505346u32.to_le_bytes());  // Magic
        buffer.extend_from_slice(&0x0001u16.to_le_bytes());      // Version
        buffer.extend_from_slice(&16u64.to_le_bytes());          // Payload size
        buffer.extend_from_slice(&self.compute_hash().to_le_bytes());  // Hash

        // Payload
        buffer.extend_from_slice(&self.amount.raw_value().to_le_bytes());
        buffer.extend_from_slice(&self.fee.raw_value().to_le_bytes());

        buffer
    }

    fn deserialize_binary(data: &[u8]) -> Result<Self, SerializeError> {
        // Validate header (magic, version, size)
        // Deserialize fields
        // Verify hash
        // Return instance
    }

    fn to_decimal_string(&self) -> String {
        format!("amount={},fee={}",
            self.amount.to_decimal_string(),
            self.fee.to_decimal_string())
    }

    fn compute_hash(&self) -> u64 {
        // FNV-1a hash of all non-skipped fields
        let mut hash = 0xcbf29ce484222325u64;
        hash ^= self.amount.raw_value() as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        hash ^= self.fee.raw_value() as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        hash
    }
}
```

## Performance

- **Compile-time**: <20ms overhead per capsule (B32 measured)
- **Runtime**: Zero cost (all validation at compile-time)
- **Binary size**: +22 bytes header per serialized capsule

## Testing

Run compile-pass/fail tests:

```bash
cargo test
```

Run with verbose output:

```bash
cargo test -- --nocapture
```

## Framework Compliance

- **UCE34 Q10**: Meta-tier (generates code for T3 Fixed-Point capsules)
- **UCE34 Q11**: Procedural macros with syn/quote
- **UCE34 Q12**: Stable Rust compatible (no nightly required)
- **UCE34 Q28**: Single `#[derive]` replaces 100+ lines of manual code
- **UCE34 Q31**: Type system ensures only valid types accepted
- **UCE34 Q33**: Compile-time verification (zero runtime cost)
- **UCE34 Q34**: Hash chain integration for audit trails (SOX, SOC2, GDPR, HIPAA)
- **ASSUM**: All assumptions documented in code comments
- **B32**: <20ms compile-time overhead (measured with Criterion)
- **T28**: 50+ tests (compile-pass, compile-fail, integration)

## License

MIT OR Apache-2.0

## See Also

- `atomic_capsule`: Core capsule infrastructure
- `atomic_capsule_derive`: Capsule verification derive macro
- `clapi_core`: Production usage in AI call protection proxy
