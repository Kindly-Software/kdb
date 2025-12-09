//! LSH Backend Integration Tests (T28 Framework)
//!
//! **Tier**: T0 (Auditable trait abstraction)
//!
//! ## Test Coverage
//!
//! - Hash Table backend basic operations
//! - Bloom filter backend basic operations
//! - Memory comparison (4,885× reduction verification)
//! - Backend interchangeability
//!
//! ## Framework Compliance
//!
//! - **T28**: Integration tests (Q15-Q21)
//! - **UCE34**: T0 trait abstraction validation
//! - **ASSUM**: Zero unsafe code in backends
//! - **B32**: Memory usage verification

use kindly_dedup::lsh::{LshBackend, LshQueryResult};
use kindly_dedup::universal::lsh_bucket::{BandHash, MmapLshBucketCapsule};
use kindly_dedup::LshBloomCapsule;

#[test]
fn test_mmap_backend_basic_operations() {
    use kindly_dedup::lsh::LshBackend; // Import trait explicitly

    let temp_dir = std::env::temp_dir().join("test_mmap_backend_basic");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut backend = MmapLshBucketCapsule::new(&temp_dir, 1000).unwrap();

    // Insert 1250 band hashes for document 42
    let band_hashes = (0..1250)
        .map(|i| BandHash::new(0, (i % 25) as u8, 0xABCD_0000_0000_0000 + i as u64))
        .collect::<Vec<_>>();

    LshBackend::insert(&mut backend, 42, &band_hashes).unwrap();

    // Query should return candidates containing doc 42
    match LshBackend::query(&backend, &band_hashes).unwrap() {
        LshQueryResult::Candidates(docs) => {
            assert!(!docs.is_empty(), "Should find at least one candidate");
            assert!(docs.contains(&42), "Should find document 42");
        }
        _ => panic!("Expected Candidates result from MmapHashTable backend"),
    }

    // Verify backend name
    assert_eq!(LshBackend::backend_name(&backend), "MmapHashTable");

    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_bloom_backend_basic_operations() {
    use kindly_dedup::lsh::LshBackend; // Import trait explicitly

    let mut backend = LshBloomCapsule::new(4);

    // Insert 32 band hashes for document 99
    let band_hashes = (0..32)
        .map(|i| BandHash::new(0, (i % 25) as u8, 0x1111_0000_0000_0000 + i as u64))
        .collect::<Vec<_>>();

    LshBackend::insert(&mut backend, 99, &band_hashes).unwrap();

    // Query should return matching band count
    match LshBackend::query(&backend, &band_hashes).unwrap() {
        LshQueryResult::MatchingBands(count) => {
            assert_eq!(count, 32, "All 32 bands should match (just inserted)");
        }
        _ => panic!("Expected MatchingBands result from LshBloom backend"),
    }

    // Verify backend name
    assert_eq!(LshBackend::backend_name(&backend), "LshBloom");
}

#[test]
fn test_memory_usage_comparison() {
    use kindly_dedup::lsh::LshBackend; // Import trait explicitly

    let temp_dir = std::env::temp_dir().join("test_memory_comparison");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mmap_backend = MmapLshBucketCapsule::new(&temp_dir, 1000).unwrap();
    let bloom_backend = LshBloomCapsule::new(4);

    let mmap_memory = LshBackend::memory_usage(&mmap_backend);
    let bloom_memory = LshBackend::memory_usage(&bloom_backend);

    println!("Mmap memory: {} bytes ({} MB)", mmap_memory, mmap_memory / 1_000_000);
    println!("Bloom memory: {} bytes ({} KB)", bloom_memory, bloom_memory / 1024);

    // Verify 4885× reduction: 136 MB / 262 KB ≈ 4885×
    let reduction_factor = mmap_memory / bloom_memory;
    println!("Reduction factor: {}×", reduction_factor);

    assert!(
        reduction_factor >= 4800 && reduction_factor <= 5000,
        "Expected ~4885× reduction, got {}× (mmap: {} bytes, bloom: {} bytes)",
        reduction_factor,
        mmap_memory,
        bloom_memory
    );

    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_bloom_zero_false_negatives() {
    use kindly_dedup::lsh::LshBackend; // Import trait explicitly

    let mut backend = LshBloomCapsule::new(4);

    // Insert 10 documents with different band hashes
    for i in 0..10 {
        let band_hashes = (0..32)
            .map(|j| BandHash::new(0, (j % 25) as u8, (i * 1000 + j) as u64))
            .collect::<Vec<_>>();
        LshBackend::insert(&mut backend, i as u32, &band_hashes).unwrap();
    }

    // All inserted documents must be found (zero false negatives)
    for i in 0..10 {
        let band_hashes = (0..32)
            .map(|j| BandHash::new(0, (j % 25) as u8, (i * 1000 + j) as u64))
            .collect::<Vec<_>>();

        match LshBackend::query(&backend, &band_hashes).unwrap() {
            LshQueryResult::MatchingBands(count) => {
                assert!(
                    count > 0,
                    "False negative for doc {} (0 matching bands, expected >0)",
                    i
                );
            }
            _ => panic!("Expected MatchingBands result"),
        }
    }
}

#[test]
fn test_backend_interchangeability() {
    use kindly_dedup::lsh::LshBackend; // Import trait explicitly

    // Both backends implement the same trait
    fn test_backend<B: LshBackend>(mut backend: B, expected_result_type: &str) {
        let band_hashes = (0..32)
            .map(|i| BandHash::new(0, (i % 25) as u8, 0xFFFF_0000_0000_0000 + i as u64))
            .collect::<Vec<_>>();

        LshBackend::insert(&mut backend, 123, &band_hashes).unwrap();

        let result = LshBackend::query(&backend, &band_hashes).unwrap();
        match (result, expected_result_type) {
            (LshQueryResult::Candidates(_), "Candidates") => {}
            (LshQueryResult::MatchingBands(_), "MatchingBands") => {}
            _ => panic!("Unexpected result type"),
        }
    }

    // Test Bloom backend
    let bloom_backend = LshBloomCapsule::new(4);
    test_backend(bloom_backend, "MatchingBands");

    // Test Mmap backend
    let temp_dir = std::env::temp_dir().join("test_backend_interchangeability");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let mmap_backend = MmapLshBucketCapsule::new(&temp_dir, 1000).unwrap();
    test_backend(mmap_backend, "Candidates");
    std::fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_bloom_false_positive_rate() {
    use kindly_dedup::lsh::LshBackend; // Import trait explicitly

    let mut backend = LshBloomCapsule::new(4); // K=4 hash functions

    // Insert 100 unique documents
    for i in 0..100 {
        let band_hashes = (0..32)
            .map(|j| BandHash::new(0, (j % 25) as u8, (i * 1000 + j) as u64))
            .collect::<Vec<_>>();
        LshBackend::insert(&mut backend, i as u32, &band_hashes).unwrap();
    }

    // Query 1000 documents NOT in the set
    let mut false_positives = 0;
    for i in 1000..2000 {
        let band_hashes = (0..32)
            .map(|j| BandHash::new(0, (j % 25) as u8, (i * 1000 + j) as u64))
            .collect::<Vec<_>>();

        match LshBackend::query(&backend, &band_hashes).unwrap() {
            LshQueryResult::MatchingBands(count) if count > 0 => {
                false_positives += 1;
            }
            _ => {}
        }
    }

    let fpr = false_positives as f64 / 1000.0;
    println!("False positive rate: {:.2}% ({}/1000)", fpr * 100.0, false_positives);

    // Bloom filter with K=4 should have FPR < 10% for 100 documents
    // (theoretical: (1 - e^(-4*100/65536))^4 ≈ 0.000024 = 0.0024%)
    assert!(
        fpr < 0.10,
        "FPR too high: {:.2}% (expected <10%)",
        fpr * 100.0
    );
}
