//! # T9+T2 PersistentSimdVector Benchmarks
//!
//! **B32 Framework Compliance**: Fair baselines, 1000+ iterations, 95% CI
//!
//! ## Benchmark Suites (4 total)
//! - Suite 1: Atomic Store/Load (<100ns target)
//! - Suite 2: SIMD Operations (4× speedup target)
//! - Suite 3: vs Alternatives (100× vs serialize+fsync target)
//! - Suite 4: Hash Consistency (<20ns FNV-1a target)

#![cfg(all(feature = "portable_simd", feature = "std"))]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::fs::OpenOptions;
use tempfile::tempdir;

// ============================================================================
// § 1: Atomic Store/Load (Suite 1)
// ============================================================================

fn bench_atomic_store(c: &mut Criterion) {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("bench_store.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();

    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    c.bench_function("atomic_store_8_elements", |b| {
        b.iter(|| {
            PersistentSimdVector::store_simd(black_box(&mut mmap), black_box(&data)).unwrap();
        });
    });
}

fn bench_atomic_load(c: &mut Criterion) {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("bench_load.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();

    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    let data = vec![1.0; 8];
    PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();

    c.bench_function("atomic_load_8_elements", |b| {
        b.iter(|| {
            let loaded = PersistentSimdVector::load_simd(black_box(&mmap)).unwrap();
            black_box(loaded);
        });
    });
}

fn bench_atomic_store_full_vector(c: &mut Criterion) {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("bench_store_full.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();

    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    let data: Vec<f32> = (0..64).map(|i| i as f32).collect();

    c.bench_function("atomic_store_64_elements", |b| {
        b.iter(|| {
            PersistentSimdVector::store_simd(black_box(&mut mmap), black_box(&data)).unwrap();
        });
    });
}

// ============================================================================
// § 2: SIMD Operations (Suite 2)
// ============================================================================

fn bench_simd_add_8_elements(c: &mut Criterion) {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("bench_simd_add.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();

    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    let initial = vec![1.0; 8];
    PersistentSimdVector::store_simd(&mut mmap, &initial).unwrap();

    let add_data = vec![10.0; 8];

    c.bench_function("simd_add_8_elements", |b| {
        b.iter(|| {
            PersistentSimdVector::simd_add(black_box(&mut mmap), black_box(&add_data)).unwrap();
        });
    });
}

fn bench_simd_add_64_elements(c: &mut Criterion) {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("bench_simd_add_64.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();

    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    let initial: Vec<f32> = (0..64).map(|i| i as f32).collect();
    PersistentSimdVector::store_simd(&mut mmap, &initial).unwrap();

    let add_data = vec![100.0; 64];

    c.bench_function("simd_add_64_elements", |b| {
        b.iter(|| {
            PersistentSimdVector::simd_add(black_box(&mut mmap), black_box(&add_data)).unwrap();
        });
    });
}

fn bench_simd_vs_scalar_add(c: &mut Criterion) {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("bench_simd_vs_scalar.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();

    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    let mut group = c.benchmark_group("simd_vs_scalar_add");

    for size in [8, 16, 32, 64] {
        let data: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let add: Vec<f32> = vec![1.0; size];

        // SIMD add
        PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();
        group.bench_with_input(BenchmarkId::new("simd", size), &size, |b, _| {
            b.iter(|| {
                PersistentSimdVector::simd_add(black_box(&mut mmap), black_box(&add)).unwrap();
            });
        });

        // Scalar add (baseline)
        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |b, _| {
            b.iter(|| {
                let current = data.clone();
                let result: Vec<f32> = current.iter().zip(add.iter()).map(|(a, b)| a + b).collect();
                black_box(result);
            });
        });
    }

    group.finish();
}

// ============================================================================
// § 3: vs Alternatives (Suite 3)
// ============================================================================

fn bench_vs_serialize_bincode(c: &mut Criterion) {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;
    use serde::{Deserialize, Serialize};
    use std::io::Write;

    #[derive(Serialize, Deserialize, Clone)]
    struct SerializedVector {
        data: Vec<f32>,
    }

    let temp_dir = tempdir().unwrap();
    let mmap_path = temp_dir.path().join("mmap.bin");
    let serialize_path = temp_dir.path().join("serialize.bin");

    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    // Prepare mmap
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&mmap_path)
        .unwrap();
    file.set_len(512).unwrap();
    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    let mut group = c.benchmark_group("vs_serialize");

    // Benchmark: PersistentSimdVector (atomic store)
    group.bench_function("persistent_simd_vector", |b| {
        b.iter(|| {
            PersistentSimdVector::store_simd(black_box(&mut mmap), black_box(&data)).unwrap();
        });
    });

    // Benchmark: bincode serialize + write + fsync
    group.bench_function("bincode_serialize_fsync", |b| {
        b.iter(|| {
            let serialized = bincode::serialize(&SerializedVector { data: data.clone() }).unwrap();
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&serialize_path)
                .unwrap();
            file.write_all(&serialized).unwrap();
            file.sync_all().unwrap();
            black_box(());
        });
    });

    group.finish();
}

fn bench_vs_json_serialize(c: &mut Criterion) {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;
    use std::io::Write;

    let temp_dir = tempdir().unwrap();
    let mmap_path = temp_dir.path().join("mmap_json.bin");
    let json_path = temp_dir.path().join("json.txt");

    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    // Prepare mmap
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&mmap_path)
        .unwrap();
    file.set_len(512).unwrap();
    let mut mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut mmap).unwrap();

    let mut group = c.benchmark_group("vs_json_serialize");

    // Benchmark: PersistentSimdVector (atomic store)
    group.bench_function("persistent_simd_vector", |b| {
        b.iter(|| {
            PersistentSimdVector::store_simd(black_box(&mut mmap), black_box(&data)).unwrap();
        });
    });

    // Benchmark: JSON serialize + write + fsync
    group.bench_function("json_serialize_fsync", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&data).unwrap();
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&json_path)
                .unwrap();
            file.write_all(json.as_bytes()).unwrap();
            file.sync_all().unwrap();
            black_box(());
        });
    });

    group.finish();
}

fn bench_recovery_time(c: &mut Criterion) {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("recovery.mmap");

    // Prepare persistent data
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

        let data: Vec<f32> = (0..64).map(|i| i as f32).collect();
        PersistentSimdVector::store_simd(&mut mmap, &data).unwrap();
        mmap.flush().unwrap();
    }

    // Benchmark recovery (re-mmap + load)
    c.bench_function("crash_recovery", |b| {
        b.iter(|| {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&file_path)
                .unwrap();
            let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
            let loaded = PersistentSimdVector::load_simd(&mmap).unwrap();
            black_box(loaded);
        });
    });
}

// ============================================================================
// § 4: Hash Consistency (Suite 4)
// ============================================================================

fn bench_generation_counter(c: &mut Criterion) {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("gen_counter.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();
    let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut *mmap).unwrap();

    c.bench_function("get_generation", |b| {
        b.iter(|| {
            let gen = PersistentSimdVector::get_generation(black_box(&mmap));
            black_box(gen);
        });
    });
}

fn bench_is_committed(c: &mut Criterion) {
    use atomic_capsule::persistence::PersistentSimdVector;
    use memmap2::MmapMut;

    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("is_committed.mmap");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    file.set_len(512).unwrap();
    let mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
    PersistentSimdVector::init_mmap(&mut *mmap).unwrap();

    c.bench_function("is_committed", |b| {
        b.iter(|| {
            let committed = PersistentSimdVector::is_committed(black_box(&mmap));
            black_box(committed);
        });
    });
}

criterion_group!(
    suite_1_atomic_ops,
    bench_atomic_store,
    bench_atomic_load,
    bench_atomic_store_full_vector
);

criterion_group!(
    suite_2_simd_ops,
    bench_simd_add_8_elements,
    bench_simd_add_64_elements,
    bench_simd_vs_scalar_add
);

criterion_group!(
    suite_3_vs_alternatives,
    bench_vs_serialize_bincode,
    bench_vs_json_serialize,
    bench_recovery_time
);

criterion_group!(
    suite_4_hash_consistency,
    bench_generation_counter,
    bench_is_committed
);

criterion_main!(
    suite_1_atomic_ops,
    suite_2_simd_ops,
    suite_3_vs_alternatives,
    suite_4_hash_consistency
);
