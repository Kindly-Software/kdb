//! # Atomic Capsule Serialize Derive Proc-Macro
//!
//! **Procedural macro for automatic fixed-point serialization in computational capsules.**
//!
//! This crate provides `#[derive(CapsuleSerialize)]` which automatically generates:
//! - FixedPointSerialize trait implementation
//! - Binary serialization (22-byte header + payload)
//! - Decimal string conversion (human-readable)
//! - Hash computation for audit trails (Q34 Auditability)
//! - Type-safe field detection (Q8_8, Q16_16, Q32_32)
//!
//! ## UCE34 Framework Application
//!
//! - **Q10 (Computational Capsule)**: Meta-infrastructure tier (generates code for T3 Fixed-Point capsules)
//! - **Q11 (Rust Transform)**: Proc-macros with syn/quote for zero-runtime-cost code generation
//! - **Q12 (Nightly)**: Stable Rust compatible (no nightly required)
//! - **Q28 (Simplicity)**: Single `#[derive]` replaces 100+ lines of manual trait implementation
//! - **Q31 (Rust)**: Type system ensures only valid fixed-point types are serialized
//! - **Q33 (Validation)**: Compile-time type checking + compile-fail tests
//! - **Q34 (Auditability)**: Automatic hash chain integration for compliance (SOX, SOC2, GDPR, HIPAA)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use atomic_capsule_derive_serialize::CapsuleSerialize;
//! use atomic_capsule::fixed_point::{Q16_16, FixedPointSerialize};
//!
//! #[derive(CapsuleSerialize)]
//! #[repr(C, align(128))]
//! struct PaymentCapsule {
//!     amount: Q16_16,
//!     fee: Q16_16,
//!     #[capsule_serialize(skip)]
//!     internal_id: u64,
//! }
//!
//! // Generated code (automatic):
//! impl FixedPointSerialize for PaymentCapsule {
//!     fn serialize_binary(&self) -> Vec<u8> { /* 22-byte header + payload */ }
//!     fn deserialize_binary(data: &[u8]) -> Result<Self, SerializeError> { /* ... */ }
//!     fn to_decimal_string(&self) -> String { /* human-readable */ }
//!     fn compute_hash(&self) -> u64 { /* audit trail integration */ }
//! }
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_FIELD_TYPES_VALID`: All fields are either fixed-point types or marked #[skip]
//! - `#VERIFY_FIELD_TYPES`: Enforced by generated const assertions at compile-time
//! - `#ASSUME_BINARY_FORMAT`: 22-byte header (magic + version + size + hash) + payload
//! - `#VERIFY_BINARY_FORMAT`: Deserialization validates header before parsing
//!
//! ## Design Philosophy (IMPL-2 V3.0)
//!
//! - **Zero runtime cost**: All verification at compile-time only
//! - **Clear error messages**: Actionable compile errors with field-level diagnostics
//! - **Minimal dependencies**: Only syn + quote + proc-macro2 (same as atomic_capsule_derive)
//! - **Stable Rust**: No nightly features required
//! - **Type safety**: Only valid fixed-point types (Q8_8, Q16_16, Q32_32) accepted
//!
//! ## Generated Binary Format
//!
//! ```text
//! Header (22 bytes):
//!   - Magic number (4 bytes): 0x43505346 ("CPSF" = CaPSule Fixed-point)
//!   - Version (2 bytes): 0x0001
//!   - Payload size (8 bytes): u64 little-endian
//!   - Hash (8 bytes): u64 FNV-1a hash of payload
//!
//! Payload (variable):
//!   - Field 1 (8 bytes): i64 raw fixed-point value
//!   - Field 2 (8 bytes): i64 raw fixed-point value
//!   - ...
//! ```
//!
//! ## Error Handling
//!
//! ### Compile-Time Errors
//!
//! ```text
//! error: CapsuleSerialize requires #[repr(C, align(...))]
//!   --> src/lib.rs:10:1
//!    |
//! 10 | struct BadCapsule {
//!    | ^^^^^^^^^^^^^^^^^^
//!    |
//!    = help: Add #[repr(C, align(64))] before struct definition
//!
//! error: Field 'price' has unsupported type 'f64'
//!   --> src/lib.rs:12:5
//!    |
//! 12 |     price: f64,
//!    |     ^^^^^^^^^^
//!    |
//!    = help: Use Q8_8, Q16_16, or Q32_32 instead of f64
//!    = help: Or mark field with #[capsule_serialize(skip)]
//! ```

extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod codegen;
mod default_value;
mod deserialize_codegen;
mod deserialize_with;
mod error_handler;
mod field_parser;
mod generic_constraint;
mod serialize_with;
mod internally_tagged;
mod rename_all;
mod skip_if;
mod type_detector;
mod untagged;
mod validator;

use codegen::generate_serialize_impl;
use deserialize_codegen::generate_deserialize_impl;
use field_parser::{parse_capsule_config, parse_capsule_fields};
use validator::validate_capsule_struct;

/// Derive macro for automatic fixed-point serialization.
///
/// # Attributes
///
/// - `#[capsule_serialize(skip)]`: Skip field during serialization (e.g., internal IDs)
/// - `#[capsule_serialize(hash_key)]`: Include field in hash but not serialization (e.g., audit keys)
/// - `#[capsule_serialize(auto_crc = true)]`: Auto-generate CRC32 verification methods (struct-level)
/// - `#[capsule_serialize(prev_hash)]`: Mark field as previous hash for hash chain integrity
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule_derive_serialize::CapsuleSerialize;
/// use atomic_capsule::fixed_point::{Q16_16, Q8_8};
///
/// #[derive(CapsuleSerialize)]
/// #[capsule_serialize(auto_crc = true)]  // ← NEW: Auto-generate CRC32 methods
/// #[repr(C, align(256))]
/// struct PaymentCapsule256 {
///     amount: Q16_16,        // Included in serialization + hash + CRC
///     fee: Q16_16,           // Included in serialization + hash + CRC
///
///     #[capsule_serialize(skip)]
///     internal_id: u64,      // Excluded from all
///
///     #[capsule_serialize(hash_key)]
///     audit_key: u64,        // Included in hash only (not serialized)
///
///     #[capsule_serialize(prev_hash)]
///     prev_hash: u64,        // Hash chain integration (Q34 Auditability)
/// }
/// ```
///
/// # Generated Code
///
/// The macro generates:
/// ```rust,ignore
/// impl FixedPointSerialize for PaymentCapsule256 {
///     fn serialize_binary(&self) -> Vec<u8> {
///         // 22-byte header
///         let mut buffer = Vec::with_capacity(38);
///         buffer.extend_from_slice(&MAGIC_NUMBER);
///         buffer.extend_from_slice(&VERSION);
///         // ... payload serialization
///     }
///
///     fn deserialize_binary(data: &[u8]) -> Result<Self, SerializeError> {
///         // Validate header
///         // Parse fields
///         // Verify hash
///     }
///
///     fn to_decimal_string(&self) -> String {
///         // Human-readable: "amount=$100.00,fee=$2.50"
///     }
///
///     fn compute_hash(&self) -> u64 {
///         // FNV-1a hash of all non-skipped fields + hash_key fields
///     }
/// }
/// ```
///
/// # Compile-Time Errors
///
/// ```text
/// error: CapsuleSerialize requires all fields to be fixed-point types or marked #[skip]
///   --> src/lib.rs:15:5
///    |
/// 15 |     price: f64,
///    |     ^^^^^^^^^^
///    |
///    = help: Change to Q8_8, Q16_16, or Q32_32
///    = help: Or add #[capsule_serialize(skip)]
/// ```
///
/// # ASSUM Framework
///
/// - `#ASSUME_REPR_C_VALID`: #[repr(C)] ensures deterministic field layout
/// - `#VERIFY_REPR_C`: Compile-time check enforces #[repr(C, align(N))]
/// - `#ASSUME_FIELD_ORDER`: Fields serialized in declaration order
/// - `#VERIFY_FIELD_ORDER`: syn parses fields in source order (guaranteed)
#[proc_macro_derive(CapsuleSerialize, attributes(capsule_serialize))]
pub fn derive_capsule_serialize(input: TokenStream) -> TokenStream {
    // #ASSUME_MACRO_INPUT_VALID: syn will parse or return compile error
    // #VERIFY_MACRO_INPUT: syn::parse_macro_input! validates syntax
    let input = parse_macro_input!(input as DeriveInput);

    // Validate struct has #[repr(C, align(N))] (UCE33 Q11: deterministic layout)
    if let Err(err) = validate_capsule_struct(&input) {
        return err.to_compile_error().into();
    }

    // Parse struct-level configuration
    let config = match parse_capsule_config(&input) {
        Ok(config) => config,
        Err(err) => return err.to_compile_error().into(),
    };

    // Parse struct fields and detect fixed-point types
    let fields = match parse_capsule_fields(&input) {
        Ok(fields) => fields,
        Err(err) => return err.to_compile_error().into(),
    };

    // Generate FixedPointSerialize trait implementation
    let serialize_impl = generate_serialize_impl(&input, &fields, &config);

    TokenStream::from(serialize_impl)
}

/// Derive macro for automatic fixed-point deserialization.
///
/// Generates `CapsuleDeserialize` trait implementation for reversing binary serialization.
///
/// # Attributes
///
/// - `#[capsule_deserialize(skip)]`: Skip field during deserialization (sets to default)
/// - `#[capsule_deserialize(default)]`: Use Default::default() for missing fields (T0 DefaultValueCapsule)
/// - `#[capsule_deserialize(default = "function_name")]`: Call custom function for missing fields
/// - `#[capsule_deserialize(default = "42")]`: Use literal value for missing fields
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule_derive_serialize::CapsuleDeserialize;
/// use atomic_capsule::fixed_point::Q16_16;
/// use atomic_capsule::serialize::CapsuleDeserialize;
///
/// #[derive(CapsuleDeserialize, Default)]
/// #[repr(C, align(128))]
/// struct PaymentCapsule {
///     amount: Q16_16,
///     #[capsule_deserialize(default)]
///     fee: Q16_16,
///
///     #[capsule_deserialize(skip)]
///     internal_id: u64,  // Set to 0 during deserialization
/// }
///
/// // Usage:
/// let bytes = serialize_payment(&payment)?;
/// let restored = PaymentCapsule::deserialize(&bytes)?;
/// ```
///
/// # Generated Code
///
/// The macro generates:
/// ```rust,ignore
/// impl CapsuleDeserialize for PaymentCapsule {
///     fn deserialize(bytes: &[u8]) -> Result<Self, FixedPointSerializeError> {
///         // Validate magic number: 0x43505346 ("CPSF")
///         // Validate version: 0x0001
///         // Parse payload fields (8 bytes each)
///         // Return constructed struct
///     }
/// }
/// ```
///
/// # Binary Format (Same as CapsuleSerialize)
///
/// ```text
/// Header (22 bytes):
///   - Magic number (4 bytes): 0x43505346 ("CPSF" = CaPSule Fixed-point)
///   - Version (2 bytes): 0x0001
///   - Payload size (8 bytes): u64 little-endian
///   - Hash (8 bytes): u64 FNV-1a hash of payload
///
/// Payload (variable):
///   - Field 1 (8 bytes): i64 raw fixed-point value
///   - Field 2 (8 bytes): i64 raw fixed-point value
///   - ...
/// ```
///
/// # ASSUM Framework
///
/// - `#ASSUME_BINARY_FORMAT_VALID`: Data follows magic/version/size/hash format
/// - `#VERIFY_BINARY_FORMAT`: Compile-time validation of header structure
/// - `#ASSUME_FIELD_COUNT_MATCH`: Binary payload has exactly N fields (N = struct field count)
/// - `#VERIFY_FIELD_COUNT`: Generated code checks payload size
/// - `#ASSUME_LITTLE_ENDIAN`: Binary data is little-endian (x86/x64 native)
/// - `#VERIFY_LITTLE_ENDIAN`: Unit tests validate on both endianness (if cross-platform)
///
/// # Errors
///
/// - `InsufficientData`: Buffer too small to contain all required fields
/// - `InvalidFormat`: Magic number mismatch (expected 0x43505346)
/// - `VersionMismatch`: Version not 0x0001
///
/// # Compile-Time Errors
///
/// ```text
/// error: CapsuleDeserialize requires #[repr(C, align(...))]
///   --> src/lib.rs:10:1
///    |
/// 10 | struct BadCapsule {
///    | ^^^^^^^^^^^^^^^^^^
///    |
///    = help: Add #[repr(C, align(64))] before struct definition
/// ```
#[proc_macro_derive(CapsuleDeserialize, attributes(capsule_deserialize))]
pub fn derive_capsule_deserialize(input: TokenStream) -> TokenStream {
    // #ASSUME_MACRO_INPUT_VALID: syn will parse or return compile error
    // #VERIFY_MACRO_INPUT: syn::parse_macro_input! validates syntax
    let input = parse_macro_input!(input as DeriveInput);

    // Validate struct has #[repr(C, align(N))]
    if let Err(err) = validate_capsule_struct(&input) {
        return err.to_compile_error().into();
    }

    // Parse struct-level configuration
    let config = match parse_capsule_config(&input) {
        Ok(config) => config,
        Err(err) => return err.to_compile_error().into(),
    };

    // Parse struct fields
    let fields = match parse_capsule_fields(&input) {
        Ok(fields) => fields,
        Err(err) => return err.to_compile_error().into(),
    };

    // Generate CapsuleDeserialize trait implementation
    let deserialize_impl = generate_deserialize_impl(&input, &fields, &config);

    TokenStream::from(deserialize_impl)
}

#[cfg(test)]
mod tests {
    // Unit tests for proc-macro logic (using syn directly)
    // Compile-pass/fail tests are in tests/ directory
}
