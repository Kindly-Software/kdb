//! T9+T3 Persistent Fixed-Point State - Unit Tests
//!
//! **Framework**: T28 Testing (Tier 1: Unit Tests)
//! **Coverage**: 20 unit tests (120 LOC)
//! **Target**: Creation, alignment, store/load, arithmetic, audit trails
//!
//! # Test Categories
//!
//! 1. **Structure Tests** (5 tests): Alignment, size, initialization
//! 2. **Store/Load Tests** (5 tests): Atomic operations, roundtrip
//! 3. **Arithmetic Tests** (5 tests): Q16.16 precision, determinism
//! 4. **Audit Trail Tests** (5 tests): Hash chaining, Q34 compliance

use atomic_capsule::persistent::fixed_point_state::PersistentFixedPointState;
use atomic_capsule::primitives::fixed_point::{FixedPoint, Q16_16};
use tempfile::NamedTempFile;

// ============================================================================
// § 1: Structure Tests (5 tests)
// ============================================================================

#[test]
fn test_alignment_512_bytes() {
    assert_eq!(
        std::mem::align_of::<PersistentFixedPointState>(),
        512,
        "Must be 512-byte aligned for page alignment"
    );
}

#[test]
fn test_size_512_bytes() {
    assert_eq!(
        std::mem::size_of::<PersistentFixedPointState>(),
        512,
        "Must be exactly 512 bytes for mmap"
    );
}

#[test]
fn test_create_new_file() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    // Verify initial state
    assert_eq!(
        state.generation(),
        0,
        "Initial generation must be 0 (committed)"
    );
    assert_eq!(state.operation_count(), 0, "Initial op count must be 0");
    assert_eq!(
        state.atomic_load_fixed().to_f64(),
        0.0,
        "Initial value must be 0.0"
    );
}

#[test]
fn test_open_existing_file() {
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();

    // Create and close
    {
        let state = PersistentFixedPointState::create(path).unwrap();
        let value = Q16_16::from_f64(123.45);
        state.atomic_store_fixed(value).unwrap();
        state.flush(path).unwrap();
    }

    // Reopen and verify
    let state = PersistentFixedPointState::open(path).unwrap();
    let loaded = state.atomic_load_fixed();
    assert!(
        (loaded.to_f64() - 123.45).abs() < 0.001,
        "Value must persist across file open/close"
    );
}

#[test]
fn test_generation_counter_even_on_create() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let gen = state.generation();
    assert_eq!(gen % 2, 0, "Generation must be even (committed) on create");
}

// ============================================================================
// § 2: Store/Load Tests (5 tests)
// ============================================================================

#[test]
fn test_atomic_store_positive() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let value = Q16_16::from_f64(999.99);
    state.atomic_store_fixed(value).unwrap();

    let loaded = state.atomic_load_fixed();
    assert!((loaded.to_f64() - 999.99).abs() < 0.001);
}

#[test]
fn test_atomic_store_negative() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let value = Q16_16::from_f64(-500.50);
    state.atomic_store_fixed(value).unwrap();

    let loaded = state.atomic_load_fixed();
    assert!((loaded.to_f64() + 500.50).abs() < 0.001);
}

#[test]
fn test_atomic_store_zero() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let value = Q16_16::ZERO;
    state.atomic_store_fixed(value).unwrap();

    let loaded = state.atomic_load_fixed();
    assert_eq!(loaded.to_f64(), 0.0);
}

#[test]
fn test_roundtrip_precision() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let original = Q16_16::from_f64(123.456789);
    state.atomic_store_fixed(original).unwrap();

    let loaded = state.atomic_load_fixed();
    let error = (loaded.to_f64() - 123.456789).abs();
    assert!(
        error < 0.001,
        "Roundtrip error must be <0.001, got {}",
        error
    );
}

#[test]
fn test_generation_increments_on_store() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let initial_gen = state.generation();
    let value = Q16_16::from_f64(100.0);
    state.atomic_store_fixed(value).unwrap();

    let new_gen = state.generation();
    assert_eq!(
        new_gen,
        initial_gen + 2,
        "Generation must increment by 2 (odd in-progress, even committed)"
    );
}

// ============================================================================
// § 3: Arithmetic Tests (5 tests)
// ============================================================================

#[test]
fn test_fixed_add_positive() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let initial = Q16_16::from_f64(100.0);
    state.atomic_store_fixed(initial).unwrap();

    let delta = Q16_16::from_f64(23.45);
    let result = state.fixed_add(delta).unwrap();

    assert!((result.to_f64() - 123.45).abs() < 0.001);
}

#[test]
fn test_fixed_add_negative() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let initial = Q16_16::from_f64(100.0);
    state.atomic_store_fixed(initial).unwrap();

    let delta = Q16_16::from_f64(-50.0);
    let result = state.fixed_add(delta).unwrap();

    assert!((result.to_f64() - 50.0).abs() < 0.001);
}

#[test]
fn test_fixed_add_determinism() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    // Test: 100 * $0.01 = $1.00 exactly (no floating-point drift)
    state.atomic_store_fixed(Q16_16::ZERO).unwrap();

    for _ in 0..100 {
        state.fixed_add(Q16_16::from_f64(0.01)).unwrap();
    }

    let result = state.atomic_load_fixed();
    assert_eq!(
        result.to_f64(),
        1.0,
        "100 * $0.01 must equal $1.00 exactly (no drift)"
    );
}

#[test]
fn test_fixed_add_small_amounts() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    state.atomic_store_fixed(Q16_16::ZERO).unwrap();

    // Add 0.001 repeatedly
    for _ in 0..1000 {
        state.fixed_add(Q16_16::from_f64(0.001)).unwrap();
    }

    let result = state.atomic_load_fixed();
    let error = (result.to_f64() - 1.0).abs();
    assert!(error < 0.001, "1000 * 0.001 precision error: {}", error);
}

#[test]
fn test_fixed_add_mixed_operations() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    state.atomic_store_fixed(Q16_16::from_f64(1000.0)).unwrap();

    state.fixed_add(Q16_16::from_f64(250.50)).unwrap();
    state.fixed_add(Q16_16::from_f64(-100.25)).unwrap();
    state.fixed_add(Q16_16::from_f64(49.75)).unwrap();

    let result = state.atomic_load_fixed();
    assert!((result.to_f64() - 1200.0).abs() < 0.001);
}

// ============================================================================
// § 4: Audit Trail Tests (5 tests)
// ============================================================================

#[test]
fn test_audit_hash_initial() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let hash = state.audit_hash();
    assert_ne!(hash, 0, "Initial audit hash must be FNV-1a offset basis");
}

#[test]
fn test_audit_hash_updates_on_store() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let initial_hash = state.audit_hash();
    let value = Q16_16::from_f64(123.45);
    state.atomic_store_fixed(value).unwrap();

    let new_hash = state.audit_hash();
    assert_ne!(new_hash, initial_hash, "Audit hash must change after store");
}

#[test]
fn test_audit_hash_chain() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    let hash1 = state.audit_hash();
    state.atomic_store_fixed(Q16_16::from_f64(100.0)).unwrap();

    let hash2 = state.audit_hash();
    state.fixed_add(Q16_16::from_f64(50.0)).unwrap();

    let hash3 = state.audit_hash();

    // Each operation must produce unique hash
    assert_ne!(hash1, hash2);
    assert_ne!(hash2, hash3);
    assert_ne!(hash1, hash3);
}

#[test]
fn test_operation_count_increments() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    assert_eq!(state.operation_count(), 0);

    state.atomic_store_fixed(Q16_16::from_f64(100.0)).unwrap();
    assert_eq!(state.operation_count(), 1);

    state.fixed_add(Q16_16::from_f64(50.0)).unwrap();
    assert_eq!(state.operation_count(), 2);

    state.fixed_add(Q16_16::from_f64(-25.0)).unwrap();
    assert_eq!(state.operation_count(), 3);
}

#[test]
fn test_export_decimal() {
    let temp = NamedTempFile::new().unwrap();
    let state = PersistentFixedPointState::create(temp.path()).unwrap();

    state.atomic_store_fixed(Q16_16::from_f64(123.45)).unwrap();

    let (decimal, gen, hash, ops) = state.export_decimal();
    assert!(decimal.contains("123.45"), "Decimal export: {}", decimal);
    assert!(gen > 0, "Generation must be non-zero after store");
    assert!(hash > 0, "Hash must be non-zero");
    assert_eq!(ops, 1, "Operation count must be 1");
}
