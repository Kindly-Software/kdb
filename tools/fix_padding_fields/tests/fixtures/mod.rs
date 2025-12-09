//! Test fixtures for fix_padding_fields tests
//!
//! These fixtures contain real-world examples from atomic_capsule
//! to ensure the tool works on production code.

/// Simple 64-byte aligned capsule with one field
pub const SIMPLE_CAPSULE: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct SimpleCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
"#;

/// 64-byte capsule with incorrect padding (needs fixing)
pub const INCORRECT_PADDING: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct IncorrectCapsule {
    state: AtomicU64,
    _padding: [u8; 32],  // Wrong! Should be 56
}
"#;

/// 64-byte capsule with missing padding
pub const MISSING_PADDING: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct MissingPaddingCapsule {
    state: AtomicU64,
}
"#;

/// 128-byte capsule with DualAtomicU64
pub const DUAL_ATOMIC_CAPSULE: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use atomic_capsule::primitives::DualAtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct DualCapsule {
    dual: DualAtomicU64,
    _padding: [u8; 112],
}
"#;

/// 64-byte capsule with multiple data fields
pub const MULTI_FIELD_CAPSULE: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicU64, AtomicU32};

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct MultiFieldCapsule {
    counter: AtomicU64,
    flags: AtomicU32,
    timestamp: AtomicU64,
    _padding: [u8; 40],
}
"#;

/// 256-byte cold tier capsule
pub const COLD_TIER_CAPSULE: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
struct ColdTierCapsule {
    state: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 240],
}
"#;

/// Capsule with multiple padding fields (needs consolidation)
pub const MULTI_PADDING_CAPSULE: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct MultiPaddingCapsule {
    state: AtomicU64,
    _padding1: [u8; 8],
    _padding2: [u8; 112],
}
"#;

/// Capsule with array field
pub const ARRAY_FIELD_CAPSULE: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct ArrayFieldCapsule {
    state: AtomicU64,
    buffer: [u8; 32],
    _padding: [u8; 88],
}
"#;

/// Capsule with generic type (edge case)
pub const GENERIC_CAPSULE: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;
use core::marker::PhantomData;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct GenericCapsule<T> {
    state: AtomicU64,
    _phantom: PhantomData<T>,
    _padding: [u8; 56],
}
"#;

/// Multiple capsules in one file
pub const MULTI_CAPSULE_FILE: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct FirstCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct SecondCapsule {
    state: AtomicU64,
    counter: AtomicU64,
    _padding: [u8; 112],
}
"#;

/// Invalid capsule (not a ComputationalCapsule)
pub const NON_CAPSULE_STRUCT: &str = r#"
use core::sync::atomic::AtomicU64;

#[repr(C, align(64))]
struct NotACapsule {
    state: AtomicU64,
}
"#;

/// Real-world circuit breaker capsule (from atomic_capsule)
pub const CIRCUIT_BREAKER_CAPSULE: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct CircuitBreakerCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
"#;

/// Expected correct output for INCORRECT_PADDING after fixing
pub const INCORRECT_PADDING_FIXED: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct IncorrectCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
"#;

/// Expected correct output for MISSING_PADDING after fixing
pub const MISSING_PADDING_FIXED: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct MissingPaddingCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}
"#;

/// Expected correct output for MULTI_PADDING_CAPSULE after consolidation
pub const MULTI_PADDING_FIXED: &str = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct MultiPaddingCapsule {
    state: AtomicU64,
    _padding: [u8; 120],
}
"#;
