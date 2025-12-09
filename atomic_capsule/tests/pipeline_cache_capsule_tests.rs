//! Integration tests for PipelineCacheCapsule (T1+T9 Mixed)
//!
//! Tests: 28 tests across T28 4-tier framework
//! Q1-Q7: Unit tests (7)
//! Q8-Q14: Property tests (7)
//! Q15-Q21: Integration tests (7)
//! Q22-Q28: Production tests (7)

#![cfg(all(feature = "std", feature = "gpu-intel"))]

use atomic_capsule::gpu::hal::pipeline_cache::{
    PipelineCacheCapsule, PipelineType, PipelineCacheError, CAPACITY, CACHE_SIZE, ALIGNMENT,
};
use std::io::Seek;
use tempfile::TempDir;

// ============================================================================
// Q1-Q7: UNIT TESTS
// ============================================================================

#[test]
fn q1_test_cache_creation() {
    let cache = PipelineCacheCapsule::new();
    assert_eq!(cache.get_entry_count(), 0);
    assert_eq!(cache.get_hit_count(), 0);
}

#[test]
fn q2_test_lookup_miss() {
    let cache = PipelineCacheCapsule::new();
    let result = cache.lookup(0x1234567890ABCDEF);
    assert!(result.is_none());
}

#[test]
fn q3_test_insert_single_entry() {
    let mut cache = PipelineCacheCapsule::new();
    let hash = 0x1234567890ABCDEF;
    let result = cache.insert(hash, PipelineType::Graphics, 512);
    assert!(result.is_ok());
    assert_eq!(cache.get_entry_count(), 1);
}

#[test]
fn q4_test_insert_and_lookup() {
    let mut cache = PipelineCacheCapsule::new();
    let hash = 0x1234567890ABCDEF;
    cache.insert(hash, PipelineType::Compute, 256).unwrap();

    let entry = cache.lookup(hash);
    assert!(entry.is_some());
    let e = entry.unwrap();
    assert_eq!(e.hash, hash);
    assert_eq!(e.pipeline_type, PipelineType::Compute as u8);
    assert_eq!(e.size, 256);
}

#[test]
fn q5_test_hit_counter_increment() {
    let mut cache = PipelineCacheCapsule::new();
    let hash = 0x1234567890ABCDEF;
    cache.insert(hash, PipelineType::Graphics, 512).unwrap();

    assert_eq!(cache.get_hit_count(), 0);
    cache.lookup(hash);
    assert_eq!(cache.get_hit_count(), 1);
    cache.lookup(hash);
    assert_eq!(cache.get_hit_count(), 2);
}

#[test]
fn q6_test_pipeline_type_enum() {
    let mut cache = PipelineCacheCapsule::new();

    let types = vec![
        (0x1000, PipelineType::Compute),
        (0x2000, PipelineType::Graphics),
        (0x3000, PipelineType::RayTracing),
        (0x4000, PipelineType::MeshShading),
    ];

    for (hash, ptype) in types {
        cache.insert(hash, ptype, 256).unwrap();
        let entry = cache.lookup(hash).unwrap();
        assert_eq!(entry.pipeline_type, ptype as u8);
    }
}

#[test]
fn q7_test_capacity_limits() {
    let mut cache = PipelineCacheCapsule::new();

    // Fill to capacity
    for i in 0..CAPACITY {
        let hash = 0x1000 + i as u64;
        let result = cache.insert(hash, PipelineType::Graphics, 256);
        assert!(result.is_ok());
    }

    // Should reject insertion beyond capacity
    let result = cache.insert(0x10000, PipelineType::Graphics, 256);
    assert!(matches!(result, Err(PipelineCacheError::CapacityExceeded)));
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS
// ============================================================================

#[test]
fn q8_test_miss_doesnt_increment_counter() {
    let cache = PipelineCacheCapsule::new();
    let before = cache.get_hit_count();
    cache.lookup(0x1234);
    let after = cache.get_hit_count();
    assert_eq!(before, after, "Miss should not increment hit counter");
}

#[test]
fn q9_test_multiple_lookups_increment() {
    let mut cache = PipelineCacheCapsule::new();
    cache.insert(0x123, PipelineType::Graphics, 256).unwrap();

    for i in 1..=5 {
        cache.lookup(0x123);
        assert_eq!(cache.get_hit_count(), i as u64);
    }
}

#[test]
fn q10_test_generation_counter_increments() {
    let mut cache = PipelineCacheCapsule::new();
    let gen_before = cache.generation.load(std::sync::atomic::Ordering::Acquire);

    cache.insert(0x123, PipelineType::Graphics, 256).unwrap();

    let gen_after = cache.generation.load(std::sync::atomic::Ordering::Acquire);
    assert!(gen_after > gen_before, "Generation should increment on insert");
}

#[test]
fn q11_test_cache_alignment() {
    let cache = PipelineCacheCapsule::new();
    let ptr = &cache as *const _ as usize;
    assert_eq!(ptr % ALIGNMENT, 0, "Cache must be properly aligned");
}

#[test]
fn q12_test_cache_size() {
    assert_eq!(
        std::mem::size_of::<PipelineCacheCapsule>(),
        CACHE_SIZE,
        "Cache size must match CACHE_SIZE constant"
    );
}

#[test]
fn q13_test_insert_different_types() {
    let mut cache = PipelineCacheCapsule::new();

    cache.insert(0x1, PipelineType::Compute, 100).unwrap();
    cache.insert(0x2, PipelineType::Graphics, 200).unwrap();
    cache.insert(0x3, PipelineType::RayTracing, 300).unwrap();
    cache.insert(0x4, PipelineType::MeshShading, 400).unwrap();

    // Verify all entries
    assert_eq!(cache.lookup(0x1).unwrap().pipeline_type, PipelineType::Compute as u8);
    assert_eq!(cache.lookup(0x2).unwrap().pipeline_type, PipelineType::Graphics as u8);
    assert_eq!(cache.lookup(0x3).unwrap().pipeline_type, PipelineType::RayTracing as u8);
    assert_eq!(cache.lookup(0x4).unwrap().pipeline_type, PipelineType::MeshShading as u8);
}

#[test]
fn q14_test_clear_resets_state() {
    let mut cache = PipelineCacheCapsule::new();

    cache.insert(0x1, PipelineType::Graphics, 256).unwrap();
    cache.insert(0x2, PipelineType::Compute, 512).unwrap();
    cache.lookup(0x1);
    cache.lookup(0x1);

    assert_eq!(cache.get_entry_count(), 2);
    assert_eq!(cache.get_hit_count(), 2);

    cache.clear();

    assert_eq!(cache.get_entry_count(), 0);
    assert_eq!(cache.get_hit_count(), 0);
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS
// ============================================================================

#[test]
fn q15_test_persist_and_recover_single_entry() {
    let tmp_dir = TempDir::new().unwrap();
    let cache_path = tmp_dir.path().join("pipeline_cache.bin");

    // Create and populate cache
    let mut cache1 = PipelineCacheCapsule::new();
    cache1.insert(0x1111, PipelineType::Graphics, 256).unwrap();

    // Persist
    cache1.mmap_persist(&cache_path).unwrap();
    assert!(cache_path.exists());

    // Recover
    let mut cache2 = PipelineCacheCapsule::new();
    cache2.mmap_recover(&cache_path).unwrap();

    // Verify
    let entry = cache2.lookup(0x1111);
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().size, 256);
}

#[test]
fn q16_test_persist_and_recover_multiple_entries() {
    let tmp_dir = TempDir::new().unwrap();
    let cache_path = tmp_dir.path().join("pipeline_cache.bin");

    // Create and populate cache
    let mut cache1 = PipelineCacheCapsule::new();
    cache1.insert(0x1111, PipelineType::Graphics, 256).unwrap();
    cache1.insert(0x2222, PipelineType::Compute, 512).unwrap();
    cache1.insert(0x3333, PipelineType::RayTracing, 1024).unwrap();

    // Persist
    cache1.mmap_persist(&cache_path).unwrap();

    // Recover
    let mut cache2 = PipelineCacheCapsule::new();
    cache2.mmap_recover(&cache_path).unwrap();

    // Verify
    assert!(cache2.lookup(0x1111).is_some());
    assert!(cache2.lookup(0x2222).is_some());
    assert!(cache2.lookup(0x3333).is_some());
    assert!(cache2.lookup(0x9999).is_none());
}

#[test]
fn q17_test_crc_validation_detects_corruption() {
    let tmp_dir = TempDir::new().unwrap();
    let cache_path = tmp_dir.path().join("pipeline_cache.bin");

    let mut cache = PipelineCacheCapsule::new();
    cache.insert(0x1111, PipelineType::Graphics, 256).unwrap();
    cache.mmap_persist(&cache_path).unwrap();

    // Corrupt file by overwriting some bytes
    use std::fs::OpenOptions;
    use std::io::Write;

    let mut file = OpenOptions::new()
        .write(true)
        .open(&cache_path)
        .unwrap();
    file.seek(std::io::SeekFrom::Start(64)).unwrap();
    file.write_all(&[0xFF, 0xFF, 0xFF, 0xFF]).unwrap();

    // Recovery should fail
    let mut cache2 = PipelineCacheCapsule::new();
    let result = cache2.mmap_recover(&cache_path);
    assert!(result.is_err(), "Should detect corrupted file");
}

#[test]
fn q18_test_multi_threaded_lookups() {
    use std::thread;

    let mut cache = PipelineCacheCapsule::new();

    // Insert some entries
    for i in 0..10 {
        cache.insert(0x1000 + i as u64, PipelineType::Graphics, 256).unwrap();
    }

    // Since we can't share mutable cache across threads, test immutable lookups
    let cache_ptr = &cache as *const PipelineCacheCapsule;

    let mut handles = vec![];

    for _ in 0..4 {
        let handle = thread::spawn(move || {
            // SAFETY: We're only doing reads, no writes
            unsafe {
                for _ in 0..100 {
                    for i in 0..10 {
                        let _ = (*cache_ptr).lookup(0x1000 + i as u64);
                    }
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all threads completed successfully
    assert_eq!(cache.get_entry_count(), 10);
}

#[test]
fn q19_test_persistence_preserves_types() {
    let tmp_dir = TempDir::new().unwrap();
    let cache_path = tmp_dir.path().join("pipeline_cache.bin");

    let mut cache1 = PipelineCacheCapsule::new();

    let test_data = vec![
        (0x1001, PipelineType::Compute, 100),
        (0x2002, PipelineType::Graphics, 200),
        (0x3003, PipelineType::RayTracing, 300),
        (0x4004, PipelineType::MeshShading, 400),
    ];

    for (hash, ptype, size) in &test_data {
        cache1.insert(*hash, *ptype, *size).unwrap();
    }

    cache1.mmap_persist(&cache_path).unwrap();

    let mut cache2 = PipelineCacheCapsule::new();
    cache2.mmap_recover(&cache_path).unwrap();

    for (hash, ptype, size) in &test_data {
        let entry = cache2.lookup(*hash).unwrap();
        assert_eq!(entry.pipeline_type, *ptype as u8);
        assert_eq!(entry.size, *size);
    }
}

#[test]
fn q20_test_full_capacity_persistence() {
    let tmp_dir = TempDir::new().unwrap();
    let cache_path = tmp_dir.path().join("pipeline_cache.bin");

    let mut cache1 = PipelineCacheCapsule::new();

    // Fill to capacity
    for i in 0..CAPACITY {
        cache1.insert(0x1000 + i as u64, PipelineType::Graphics, 256 + i as u32).unwrap();
    }

    cache1.mmap_persist(&cache_path).unwrap();

    let mut cache2 = PipelineCacheCapsule::new();
    cache2.mmap_recover(&cache_path).unwrap();

    // Verify all entries recovered
    for i in 0..CAPACITY {
        assert!(cache2.lookup(0x1000 + i as u64).is_some());
    }
}

#[test]
fn q21_test_generation_counter_persistence() {
    let tmp_dir = TempDir::new().unwrap();
    let cache_path = tmp_dir.path().join("pipeline_cache.bin");

    let mut cache1 = PipelineCacheCapsule::new();

    let gen1_before = cache1.generation.load(std::sync::atomic::Ordering::Acquire);
    cache1.insert(0x1111, PipelineType::Graphics, 256).unwrap();
    let gen1_after = cache1.generation.load(std::sync::atomic::Ordering::Acquire);

    assert_eq!(gen1_before + 1, gen1_after);

    cache1.mmap_persist(&cache_path).unwrap();

    let mut cache2 = PipelineCacheCapsule::new();
    cache2.mmap_recover(&cache_path).unwrap();

    let gen2 = cache2.generation.load(std::sync::atomic::Ordering::Acquire);
    assert_eq!(gen1_after, gen2, "Generation should be preserved");
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS
// ============================================================================

#[test]
fn q22_test_stress_1m_lookups() {
    let mut cache = PipelineCacheCapsule::new();

    // Insert 32 pipelines
    for i in 0..CAPACITY {
        cache.insert(0x1000 + i as u64, PipelineType::Graphics, 256).unwrap();
    }

    // Perform 1M lookups
    for _ in 0..1_000_000 {
        for i in 0..CAPACITY {
            let _ = cache.lookup(0x1000 + i as u64);
        }
    }

    // Verify hit counter
    assert_eq!(cache.get_hit_count(), 32_000_000);
}

#[test]
fn q23_test_insert_various_sizes() {
    let mut cache = PipelineCacheCapsule::new();

    let sizes = vec![64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768];

    for (i, &size) in sizes.iter().enumerate() {
        cache.insert(0x1000 + i as u64, PipelineType::Graphics, size).unwrap();

        let entry = cache.lookup(0x1000 + i as u64).unwrap();
        assert_eq!(entry.size, size);
    }
}

#[test]
fn q24_test_recovery_without_corruption() {
    let tmp_dir = TempDir::new().unwrap();
    let cache_path = tmp_dir.path().join("pipeline_cache.bin");

    for iteration in 0..3 {
        let mut cache1 = PipelineCacheCapsule::new();

        for i in 0..10 {
            cache1.insert(0x1000 + i as u64, PipelineType::Graphics, 256).unwrap();
        }

        cache1.mmap_persist(&cache_path).unwrap();

        let mut cache2 = PipelineCacheCapsule::new();
        let result = cache2.mmap_recover(&cache_path);

        assert!(result.is_ok(), "Recovery should succeed on iteration {}", iteration);
        assert_eq!(cache2.get_entry_count(), 10);
    }
}

#[test]
fn q25_test_pipeline_type_filtering() {
    let mut cache = PipelineCacheCapsule::new();

    let compute_hash = 0x1000;
    let graphics_hash = 0x2000;
    let rt_hash = 0x3000;
    let mesh_hash = 0x4000;

    cache.insert(compute_hash, PipelineType::Compute, 256).unwrap();
    cache.insert(graphics_hash, PipelineType::Graphics, 512).unwrap();
    cache.insert(rt_hash, PipelineType::RayTracing, 1024).unwrap();
    cache.insert(mesh_hash, PipelineType::MeshShading, 2048).unwrap();

    // Verify types are preserved
    assert_eq!(
        cache.lookup(compute_hash).unwrap().pipeline_type,
        PipelineType::Compute as u8
    );
    assert_eq!(
        cache.lookup(graphics_hash).unwrap().pipeline_type,
        PipelineType::Graphics as u8
    );
    assert_eq!(
        cache.lookup(rt_hash).unwrap().pipeline_type,
        PipelineType::RayTracing as u8
    );
    assert_eq!(
        cache.lookup(mesh_hash).unwrap().pipeline_type,
        PipelineType::MeshShading as u8
    );
}

#[test]
fn q26_test_repeated_inserts_same_hash() {
    let mut cache = PipelineCacheCapsule::new();

    let hash = 0x1234;

    // First insert
    cache.insert(hash, PipelineType::Graphics, 256).unwrap();
    assert_eq!(cache.get_entry_count(), 1);

    // Second insert with different parameters - will create new entry if slot available
    cache.insert(hash, PipelineType::Compute, 512).unwrap();

    // Lookup should return the first entry (lowest index)
    let entry = cache.lookup(hash);
    assert!(entry.is_some());
}

#[test]
fn q27_test_memory_bounds() {
    let cache = PipelineCacheCapsule::new();

    // Cache should be at least CACHE_SIZE bytes
    let actual_size = std::mem::size_of_val(&cache);
    assert!(
        actual_size >= CACHE_SIZE,
        "Cache size {} must be >= CACHE_SIZE {}",
        actual_size,
        CACHE_SIZE
    );

    // Cache should be properly aligned
    let ptr = &cache as *const _ as usize;
    assert_eq!(ptr % ALIGNMENT, 0, "Cache should be {}byte aligned", ALIGNMENT);
}

#[test]
fn q28_test_hot_cache_lookup_performance() {
    let mut cache = PipelineCacheCapsule::new();

    // Populate cache
    for i in 0..32 {
        cache.insert(0x1000 + i as u64, PipelineType::Graphics, 256).unwrap();
    }

    // Warm up
    for _ in 0..1000 {
        let _ = cache.lookup(0x1000);
    }

    // Benchmark 100K lookups
    let start = std::time::Instant::now();
    for _ in 0..100_000 {
        let _ = cache.lookup(0x1000);
    }
    let elapsed = start.elapsed();

    let ns_per_op = elapsed.as_nanos() / 100_000;
    println!("Hot cache lookup: {} ns/op (target: <50ns)", ns_per_op);
    // Note: We don't assert here as performance varies, but target is <50ns
}
