//! T0 Auditable Tier - Deterministic, Tamper-Evident Primitives
//!
//! Zero-cost, deterministic building blocks for compliance and audit trails
//! without external dependencies or hidden complexity.
//!
//! # Architecture
//!
//! **Tier 0 (Auditable)**: Simplest, most deterministic, zero cost
//! - Hex encoding/decoding for audit trail serialization (requires std)
//! - No external dependencies (stdlib only)
//! - Deterministic output (same input → identical output)
//! - Safe Rust (no unsafe code)
//!
//! # Module Structure
//!
//! - `hex` - Deterministic hex encoding/decoding (T0, requires std)
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q1-Q10 (T0 Auditable tier selection)
//! - **Chaos**: 100% safe, zero dependencies, no allocations beyond output
//! - **ASSUM**: 100% safe (no unsafe blocks)
//! - **T28**: 18 unit + edge case tests
//!
//! # Example
//!
//! ```rust,ignore
//! use atomic_capsule::auditable::hex;
//!
//! // Encode bytes to hex
//! let hex_string = hex::encode(b"hello");
//! assert_eq!(hex_string, "68656c6c6f");
//!
//! // Decode hex back to bytes
//! let decoded = hex::decode(&hex_string).unwrap();
//! assert_eq!(decoded, b"hello");
//! ```

#[cfg(feature = "std")]
pub mod hex;

#[cfg(feature = "audit-compression")]
pub mod audit_compression;

// Re-export commonly used items (only when std is available)
#[cfg(feature = "std")]
pub use hex::{decode as hex_decode, encode as hex_encode};

#[cfg(feature = "audit-compression")]
pub use audit_compression::{
    AuditCompressionCapsule, AuditEvent, AuditEventType, AuditCompressionError,
    MAX_AUDIT_EVENTS, COMPRESSED_EVENT_SIZE_ESTIMATE,
};
