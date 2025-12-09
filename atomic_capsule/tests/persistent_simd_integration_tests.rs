//! # T9+T2 PersistentSimdVector Integration Tests
//!
//! **T28 Testing Framework - Tier 3: Integration Tests (Tests 41-50)**
//!
//! ## Coverage
//! - Tests 41-45: End-to-end mmap workflows
//! - Tests 46-50: Crash recovery scenarios

#![cfg(all(feature = "portable_simd", feature = "std"))]

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use tempfile::{tempdir, NamedTempFile};

// ============================================================================
// § 1: End-to-End mmap Workflows (Tests 41-45)
// ============================================================================

#[test]
fn test_41_mmap_file_lifecycle() {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.mmap");

    // Create file
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();

    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

    // Initialize
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    // Store data
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();

    // Flush to disk
    mmap.flush().unwrap();

    // Drop mmap
    drop(mmap);
    drop(file);

    // Reopen and verify
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&file_path)
        .unwrap();
    let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

    let loaded = PersistentSimdVector::load_simd(&mmap).unwrap();
    assert_eq!(loaded, data);
}

#[test]
fn test_42_multiple_updates_with_flush() {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();

    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    // Multiple updates with flush
    for i in 0..10 {
        let data: Vec<f32> = vec![i as f32; 8];
        PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();
        mmap.flush().unwrap();

        // Verify persistence
        let loaded = PersistentSimdVector::load_simd(&mmap).unwrap();
        assert_eq!(loaded, data);
    }
}

#[test]
fn test_43_simd_add_workflow() {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();

    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    // Initial data
    let data = vec![1.0; 8];
    PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();

    // Accumulate via SIMD add
    for _ in 0..10 {
        let add_data = vec![5.0; 8];
        PersistentSimdVector::simd_add(&mut mmap, &add_data).unwrap();
    }

    mmap.flush().unwrap();

    // Verify final result
    let final_data = PersistentSimdVector::load_simd(&mmap).unwrap();
    assert_eq!(final_data, vec![51.0; 8]); // 1.0 + 10 * 5.0
}

#[test]
fn test_44_full_vector_workflow() {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();

    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    // Store full vector (64 elements)
    let data: Vec<f32> = (0..64).map(|i| i as f32).collect();
    PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();

    // SIMD add
    let add_data = vec![100.0; 64];
    PersistentSimdVector::simd_add(&mut mmap, &add_data).unwrap();

    mmap.flush().unwrap();

    // Reload and verify
    drop(mmap);
    let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

    let loaded = PersistentSimdVector::load_simd(&mmap).unwrap();
    let expected: Vec<f32> = (0..64).map(|i| i as f32 + 100.0).collect();
    assert_eq!(loaded, expected);
}

#[test]
fn test_45_generation_tracking() {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();

    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    let mut expected_gen = 0u64;

    for i in 0..10 {
        let data = vec![i as f32; 8];
        PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();

        expected_gen += 2; // Each store increments by 2
        let actual_gen = PersistentSimdVector::get_generation(&mmap);
        assert_eq!(actual_gen, expected_gen);
    }

    mmap.flush().unwrap();

    // Reload and verify generation persisted
    drop(mmap);
    let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

    let final_gen = PersistentSimdVector::get_generation(&mmap);
    assert_eq!(final_gen, expected_gen);
}

// ============================================================================
// § 2: Crash Recovery Scenarios (Tests 46-50)
// ============================================================================

#[test]
fn test_46_crash_recovery_basic() {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.mmap");

    // Write phase
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&file_path)
            .unwrap();
        file.set_len(512).unwrap();

        let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
        PersistentSimdVector::init_mmap(&mut mmap).unwrap();

        let data = vec![42.0; 8];
        PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();
        mmap.flush().unwrap();

        // Simulate crash (drop without explicit close)
    }

    // Recovery phase
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .unwrap();
        let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

        // Verify data survived
        assert!(PersistentSimdVector::is_committed(&mmap));
        let loaded = PersistentSimdVector::load_simd(&mmap).unwrap();
        assert_eq!(loaded, vec![42.0; 8]);
    }
}

#[test]
fn test_47_committed_state_recovery() {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.mmap");

    // Write multiple updates
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&file_path)
            .unwrap();
        file.set_len(512).unwrap();

        let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
        PersistentSimdVector::init_mmap(&mut mmap).unwrap();

        for i in 0..5 {
            let data = vec![i as f32; 8];
            PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();
        }

        mmap.flush().unwrap();
    }

    // Recovery
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .unwrap();
        let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

        // Verify committed state
        assert!(PersistentSimdVector::is_committed(&mmap));
        let gen = PersistentSimdVector::get_generation(&mmap);
        assert!(gen & 1 == 0);
        assert_eq!(gen, 10); // 5 stores × 2 increments each

        let loaded = PersistentSimdVector::load_simd(&mmap).unwrap();
        assert_eq!(loaded, vec![4.0; 8]); // Last committed value
    }
}

#[test]
fn test_48_generation_counter_recovery() {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.mmap");

    let gen_before;

    // Write phase
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&file_path)
            .unwrap();
        file.set_len(512).unwrap();

        let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
        PersistentSimdVector::init_mmap(&mut mmap).unwrap();

        let data = vec![123.0; 8];
        PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();
        mmap.flush().unwrap();

        gen_before = PersistentSimdVector::get_generation(&mmap);
    }

    // Recovery phase
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .unwrap();
        let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

        let gen_after = PersistentSimdVector::get_generation(&mmap);
        assert_eq!(gen_after, gen_before);
    }
}

#[test]
fn test_49_simd_add_recovery() {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.mmap");

    // Write phase: Accumulate via SIMD add
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&file_path)
            .unwrap();
        file.set_len(512).unwrap();

        let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
        PersistentSimdVector::init_mmap(&mut mmap).unwrap();

        let data = vec![10.0; 8];
        PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();

        for _ in 0..3 {
            let add = vec![5.0; 8];
            PersistentSimdVector::simd_add(&mut mmap, &add).unwrap();
        }

        mmap.flush().unwrap();
    }

    // Recovery: Verify accumulated result
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .unwrap();
        let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

        let loaded = PersistentSimdVector::load_simd(&mmap).unwrap();
        assert_eq!(loaded, vec![25.0; 8]); // 10.0 + 3 * 5.0
    }
}

#[test]
fn test_50_full_vector_recovery() {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.mmap");

    let expected_data: Vec<f32> = (0..64).map(|i| i as f32 * 2.5).collect();

    // Write phase
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&file_path)
            .unwrap();
        file.set_len(512).unwrap();

        let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
        PersistentSimdVector::init_mmap(&mut mmap).unwrap();

        PersistentSimdVector::store_simd(&mut mmap, &expected_data).unwrap();
        mmap.flush().unwrap();
    }

    // Recovery phase
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .unwrap();
        let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };

        // Verify full vector recovery
        assert!(PersistentSimdVector::is_committed(&mmap));
        let loaded = PersistentSimdVector::load_simd(&mmap).unwrap();
        assert_eq!(loaded.len(), 64);
        assert_eq!(loaded, expected_data);
    }
}
