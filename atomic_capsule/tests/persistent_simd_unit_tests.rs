//! # T9+T2 PersistentSimdVector Unit Tests
//!
//! **T28 Testing Framework - Tier 1: Unit Tests (Tests 1-25)**
//!
//! ## Coverage
//! - Tests 1-5: Creation and alignment
//! - Tests 6-10: Store/Load atomic operations
//! - Tests 11-15: Two-phase commit pattern
//! - Tests 16-20: SIMD operations
//! - Tests 21-25: Error handling

#![cfg(all(feature = "portable_simd", feature = "std"))]

use std::fs::OpenOptions;
use std::io::Write as _;
use tempfile::NamedTempFile;

// ============================================================================
// § 1: Creation and Alignment (Tests 1-5)
// ============================================================================

#[test]
fn test_1_compile_time_size() {
    use atomic_capsule::persistence::PersistentSimdVector;

    assert_eq!(
        core::mem::size_of::<PersistentSimdVector>(),
        512,
        "PersistentSimdVector must be 512 bytes"
    );
}

#[test]
fn test_2_compile_time_alignment() {
    use atomic_capsule::persistence::PersistentSimdVector;

    assert_eq!(
        core::mem::align_of::<PersistentSimdVector>(),
        512,
        "PersistentSimdVector must be 512-byte aligned"
    );
}

#[test]
fn test_3_init_mmap() {
    use atomic_capsule::persistence::PersistentSimdVector;

    // Create aligned buffer (512 bytes)
    let mut buffer = vec![0u8; 4096]; // Page-aligned
    let aligned_slice = &mut buffer[..512];

    // Initialize
    let result = PersistentSimdVector::init_mmap(aligned_slice);
    assert!(result.is_ok(), "init_mmap should succeed");
}

#[test]
fn test_4_init_too_small() {
    use atomic_capsule::persistence::PersistentSimdVector;

    // Too small buffer
    let mut buffer = vec![0u8; 256];

    let result = PersistentSimdVector::init_mmap(&mut buffer);
    assert!(result.is_err(), "init_mmap should fail for small buffer");
    assert_eq!(result.unwrap_err(), "mmap too small (need 512 bytes)");
}

#[test]
fn test_5_init_generation_zero() {
    use atomic_capsule::persistence::PersistentSimdVector;

    // Create aligned buffer
    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    // Initialize
    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Verify generation is 0 (committed state)
    assert_eq!(
        PersistentSimdVector::get_generation(aligned_slice),
        0,
        "Initial generation must be 0 (even = committed)"
    );
    assert!(
        PersistentSimdVector::is_committed(aligned_slice),
        "Initial state must be committed"
    );
}

// ============================================================================
// § 2: Store/Load Atomic Operations (Tests 6-10)
// ============================================================================

#[test]
fn test_6_store_simd_basic() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Store SIMD data
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let result = PersistentSimdVector::store_simd(aligned_slice, &data);
    assert!(result.is_ok(), "store_simd should succeed");
}

#[test]
fn test_7_load_simd_basic() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Store then load
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    PersistentSimdVector::store_simd(aligned_slice, &data).unwrap();

    let loaded = PersistentSimdVector::load_simd(aligned_slice).unwrap();
    assert_eq!(loaded, data, "Loaded data must match stored data");
}

#[test]
fn test_8_store_max_length() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Store maximum length (64 elements)
    let data: Vec<f32> = (0..64).map(|i| i as f32).collect();
    let result = PersistentSimdVector::store_simd(aligned_slice, &data);
    assert!(result.is_ok(), "store_simd should succeed for max length");

    let loaded = PersistentSimdVector::load_simd(aligned_slice).unwrap();
    assert_eq!(loaded, data, "Loaded data must match stored data");
}

#[test]
fn test_9_store_too_large() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Try to store too many elements (65 > 64 max)
    let data: Vec<f32> = (0..65).map(|i| i as f32).collect();
    let result = PersistentSimdVector::store_simd(aligned_slice, &data);
    assert!(result.is_err(), "store_simd should fail for oversized data");
    assert_eq!(result.unwrap_err(), "data too large (max 64 elements)");
}

#[test]
fn test_10_load_empty() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Load without storing (should return empty vector)
    let loaded = PersistentSimdVector::load_simd(aligned_slice).unwrap();
    assert_eq!(loaded.len(), 0, "Initial load should return empty vector");
}

// ============================================================================
// § 3: Two-Phase Commit Pattern (Tests 11-15)
// ============================================================================

#[test]
fn test_11_generation_increments() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    let gen_before = PersistentSimdVector::get_generation(aligned_slice);

    // Store data (increments generation by 2: odd -> even)
    let data = vec![1.0; 8];
    PersistentSimdVector::store_simd(aligned_slice, &data).unwrap();

    let gen_after = PersistentSimdVector::get_generation(aligned_slice);

    assert_eq!(
        gen_after,
        gen_before + 2,
        "Generation should increment by 2"
    );
    assert!(gen_after & 1 == 0, "Generation should be even (committed)");
}

#[test]
fn test_12_committed_state_after_store() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Store data
    let data = vec![1.0; 8];
    PersistentSimdVector::store_simd(aligned_slice, &data).unwrap();

    // Verify committed state
    assert!(
        PersistentSimdVector::is_committed(aligned_slice),
        "State should be committed after store"
    );
}

#[test]
fn test_13_is_committed_check() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Initial state: committed (generation 0 = even)
    assert!(PersistentSimdVector::is_committed(aligned_slice));

    // After store: still committed (generation 2 = even)
    let data = vec![1.0; 8];
    PersistentSimdVector::store_simd(aligned_slice, &data).unwrap();
    assert!(PersistentSimdVector::is_committed(aligned_slice));
}

#[test]
fn test_14_generation_even_after_multiple_stores() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Multiple stores
    for i in 0..10 {
        let data: Vec<f32> = vec![i as f32; 8];
        PersistentSimdVector::store_simd(aligned_slice, &data).unwrap();

        let gen = PersistentSimdVector::get_generation(aligned_slice);
        assert!(gen & 1 == 0, "Generation should always be even after store");
    }
}

#[test]
fn test_15_generation_monotonic() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    let mut prev_gen = PersistentSimdVector::get_generation(aligned_slice);

    // Verify generation is strictly increasing
    for i in 0..5 {
        let data: Vec<f32> = vec![i as f32; 8];
        PersistentSimdVector::store_simd(aligned_slice, &data).unwrap();

        let curr_gen = PersistentSimdVector::get_generation(aligned_slice);
        assert!(
            curr_gen > prev_gen,
            "Generation must be monotonically increasing"
        );
        prev_gen = curr_gen;
    }
}

// ============================================================================
// § 4: SIMD Operations (Tests 16-20)
// ============================================================================

#[test]
fn test_16_simd_add_basic() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Store initial data
    let data1 = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    PersistentSimdVector::store_simd(aligned_slice, &data1).unwrap();

    // SIMD add
    let data2 = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0];
    let result = PersistentSimdVector::simd_add(aligned_slice, &data2);
    assert!(result.is_ok(), "simd_add should succeed");

    // Verify result
    let loaded = PersistentSimdVector::load_simd(aligned_slice).unwrap();
    let expected = vec![11.0, 22.0, 33.0, 44.0, 55.0, 66.0, 77.0, 88.0];
    assert_eq!(loaded, expected, "SIMD add result incorrect");
}

#[test]
fn test_17_simd_add_full_lane() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Store full SIMD lane (64 elements = 8 lanes of 8)
    let data1: Vec<f32> = (0..64).map(|i| i as f32).collect();
    PersistentSimdVector::store_simd(aligned_slice, &data1).unwrap();

    // SIMD add full lane
    let data2: Vec<f32> = vec![100.0; 64];
    PersistentSimdVector::simd_add(aligned_slice, &data2).unwrap();

    // Verify result
    let loaded = PersistentSimdVector::load_simd(aligned_slice).unwrap();
    let expected: Vec<f32> = (0..64).map(|i| i as f32 + 100.0).collect();
    assert_eq!(loaded, expected, "SIMD add full lane result incorrect");
}

#[test]
fn test_18_simd_add_partial_lane() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Store partial lane (5 elements < 8)
    let data1 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    PersistentSimdVector::store_simd(aligned_slice, &data1).unwrap();

    // SIMD add (scalar fallback for partial lane)
    let data2 = vec![10.0, 20.0, 30.0, 40.0, 50.0];
    PersistentSimdVector::simd_add(aligned_slice, &data2).unwrap();

    // Verify result
    let loaded = PersistentSimdVector::load_simd(aligned_slice).unwrap();
    let expected = vec![11.0, 22.0, 33.0, 44.0, 55.0];
    assert_eq!(loaded, expected, "SIMD add partial lane result incorrect");
}

#[test]
fn test_19_simd_add_length_mismatch() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Store 8 elements
    let data1 = vec![1.0; 8];
    PersistentSimdVector::store_simd(aligned_slice, &data1).unwrap();

    // Try to add 4 elements (length mismatch)
    let data2 = vec![10.0; 4];
    let result = PersistentSimdVector::simd_add(aligned_slice, &data2);
    assert!(result.is_err(), "simd_add should fail for length mismatch");
    assert_eq!(result.unwrap_err(), "length mismatch");
}

#[test]
fn test_20_simd_add_preserves_generation_increment() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Store initial data
    let data1 = vec![1.0; 8];
    PersistentSimdVector::store_simd(aligned_slice, &data1).unwrap();

    let gen_before = PersistentSimdVector::get_generation(aligned_slice);

    // SIMD add
    let data2 = vec![10.0; 8];
    PersistentSimdVector::simd_add(aligned_slice, &data2).unwrap();

    let gen_after = PersistentSimdVector::get_generation(aligned_slice);

    assert_eq!(
        gen_after,
        gen_before + 2,
        "SIMD add should increment generation by 2"
    );
    assert!(
        gen_after & 1 == 0,
        "Generation should remain even after SIMD add"
    );
}

// ============================================================================
// § 5: Error Handling (Tests 21-25)
// ============================================================================

#[test]
fn test_21_error_mmap_too_small() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 256]; // Too small

    let result = PersistentSimdVector::init_mmap(&mut buffer);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "mmap too small (need 512 bytes)");
}

#[test]
fn test_22_error_store_too_large() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    let data: Vec<f32> = (0..65).map(|i| i as f32).collect(); // 65 > 64 max
    let result = PersistentSimdVector::store_simd(aligned_slice, &data);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "data too large (max 64 elements)");
}

#[test]
fn test_23_error_add_length_mismatch() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    let data1 = vec![1.0; 8];
    PersistentSimdVector::store_simd(aligned_slice, &data1).unwrap();

    let data2 = vec![10.0; 16]; // Different length
    let result = PersistentSimdVector::simd_add(aligned_slice, &data2);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "length mismatch");
}

#[test]
fn test_24_load_committed_state_only() {
    use atomic_capsule::persistence::PersistentSimdVector;

    let mut buffer = vec![0u8; 4096];
    let aligned_slice = &mut buffer[..512];

    PersistentSimdVector::init_mmap(aligned_slice).unwrap();

    // Store data normally (ends in committed state)
    let data = vec![1.0; 8];
    PersistentSimdVector::store_simd(aligned_slice, &data).unwrap();

    // Load should succeed (committed state)
    let result = PersistentSimdVector::load_simd(aligned_slice);
    assert!(result.is_ok(), "Load should succeed in committed state");
}

#[test]
fn test_25_constants() {
    use atomic_capsule::persistence::PersistentSimdVector;

    assert_eq!(PersistentSimdVector::MAX_LEN, 64);
    assert_eq!(PersistentSimdVector::SIZE, 512);
    assert_eq!(PersistentSimdVector::ALIGNMENT, 512);
}
