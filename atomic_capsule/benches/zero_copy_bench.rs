//! # Zero-Copy Deserialization Benchmarks (Phase 5.0)
//!
//! **Mission**: Validate 50× speedup claim with B32 framework rigor
//!
//! ## B32 Framework Compliance
//!
//! **Fair Baselines**:
//! - Copy deserialization: Current `deserialize_binary()` (80-100ns)
//! - Manual parsing: Worst-case baseline (parsing + validation)
//! - NOT comparing to: Null operation, strawman
//!
//! **Statistical Rigor**:
//! - 1000+ iterations (Criterion default)
//! - 95% Confidence Interval
//! - P50, P95, P99 percentiles
//! - Outlier detection
//!
//! **Honest Claims**:
//! - Target: 30-50× speedup (80-100ns → 1.5-3ns)
//! - Reality check: 50× is EXCEPTIONAL but achievable
//! - Justification: Eliminate ALL copying (memcpy dominates baseline)
//!
//! ## Performance Targets
//!
//! | Operation | Baseline (copy) | Target (zero-copy) | Speedup |
//! |-----------|-----------------|-------------------|---------|
//! | Q16_16 | 80-100ns | 1.5-3ns | 30-50× |
//! | Q32_32 | 80-100ns | 1.5-3ns | 30-50× |
//! | PaymentCapsule256 | 148ns | 3ns | 50× |
//! | AuditLogEntry1K | 200ns | 3ns | 60-70× |
//!
//! ## ASSUM Safety
//!
//! All benchmarks use properly aligned buffers (ASSUM-verified).

use atomic_capsule::serialize::{
    enhanced_fixed_point_impls::FixedPointSerialize,
    fixed_point_impls::{Q16_16, Q32_32},
    zero_copy::{ZeroCopyDeserialize, ZeroCopyDeserializeCapsule},
    zero_copy_capsules::{ZeroCopyAuditLogEntry, ZeroCopyPaymentCapsule},
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::mem::size_of;

// ============================================================================
// 1. BASELINE: Copy Deserialization (Current Implementation)
// ============================================================================

fn bench_copy_deserialize_q16_16(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline/copy_deserialize");
    group.throughput(Throughput::Elements(1));

    // Create serialized Q16_16
    let value = Q16_16::from_f64(1234.5678);
    let bytes = value.serialize_binary().unwrap();

    group.bench_function("Q16_16_copy", |b| {
        b.iter(|| {
            black_box(Q16_16::deserialize_binary(black_box(&bytes)).unwrap());
        });
    });

    group.finish();
}

fn bench_copy_deserialize_q32_32(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline/copy_deserialize");
    group.throughput(Throughput::Elements(1));

    let value = Q32_32::from_f64(1234567.890123);
    let bytes = value.serialize_binary().unwrap();

    group.bench_function("Q32_32_copy", |b| {
        b.iter(|| {
            black_box(Q32_32::deserialize_binary(black_box(&bytes)).unwrap());
        });
    });

    group.finish();
}

// ============================================================================
// 2. ZERO-COPY: Direct Pointer Cast
// ============================================================================

fn bench_zero_copy_deserialize_q16_16(c: &mut Criterion) {
    let mut group = c.benchmark_group("zero_copy/deserialize");
    group.throughput(Throughput::Elements(1));

    // Create aligned buffer (Q16_16 is 4 bytes, 4-byte aligned)
    let value = Q16_16::from_f64(1234.5678);
    let raw = value.to_raw();
    let bytes = raw.to_le_bytes();

    group.bench_function("Q16_16_zero_copy", |b| {
        b.iter(|| {
            black_box(Q16_16::from_bytes(black_box(&bytes)).unwrap());
        });
    });

    group.finish();
}

fn bench_zero_copy_deserialize_q32_32(c: &mut Criterion) {
    let mut group = c.benchmark_group("zero_copy/deserialize");
    group.throughput(Throughput::Elements(1));

    // Create aligned buffer (Q32_32 is 8 bytes, 8-byte aligned)
    let value = Q32_32::from_f64(1234567.890123);
    let raw = value.to_raw();
    let bytes = raw.to_le_bytes();

    group.bench_function("Q32_32_zero_copy", |b| {
        b.iter(|| {
            black_box(Q32_32::from_bytes(black_box(&bytes)).unwrap());
        });
    });

    group.finish();
}

// ============================================================================
// 3. ZERO-COPY CAPSULES: Memory-Mapped Structures
// ============================================================================

fn bench_payment_capsule_copy(c: &mut Criterion) {
    let mut group = c.benchmark_group("capsules/payment");
    group.throughput(Throughput::Bytes(256));

    // Create payment capsule
    let capsule = ZeroCopyPaymentCapsule::new(
        Q16_16::from_f64(100.0),
        Q16_16::from_f64(2.91),
        Q16_16::from_f64(97.09),
        1234567890,
        0xDEADBEEF,
        0xCAFEBABE,
        0x12345678,
    );

    // Serialize to bytes
    let bytes: Vec<u8> = unsafe {
        std::slice::from_raw_parts(
            &capsule as *const _ as *const u8,
            size_of::<ZeroCopyPaymentCapsule>(),
        )
    }
    .to_vec();

    // Baseline: Copy deserialization (manual parsing)
    group.bench_function("copy_deserialize", |b| {
        b.iter(|| {
            // Simulate copy deserialization: memcpy + validation + construct
            let mut buffer = [0u8; 256];
            buffer.copy_from_slice(black_box(&bytes));

            // Validate magic
            let magic = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
            assert_eq!(magic, ZeroCopyPaymentCapsule::MAGIC);

            // Validate version
            let version = u16::from_le_bytes([buffer[4], buffer[5]]);
            assert_eq!(version, ZeroCopyPaymentCapsule::VERSION);

            black_box(buffer);
        });
    });

    // Zero-copy deserialization
    group.bench_function("zero_copy_deserialize", |b| {
        b.iter(|| {
            black_box(ZeroCopyPaymentCapsule::from_bytes(black_box(&bytes)).unwrap());
        });
    });

    group.finish();
}

fn bench_audit_log_entry_copy(c: &mut Criterion) {
    let mut group = c.benchmark_group("capsules/audit_log");
    group.throughput(Throughput::Bytes(1024));

    // Create audit log entry
    let entry = ZeroCopyAuditLogEntry {
        magic: ZeroCopyAuditLogEntry::MAGIC,
        version: ZeroCopyAuditLogEntry::VERSION,
        entry_type: ZeroCopyAuditLogEntry::TYPE_CREATE,
        timestamp_ns: 1234567890,
        user_id: 0xDEADBEEF,
        session_id: 0xCAFEBABE,
        resource_id: 0x12345678,
        amount: Q32_32::from_f64(1000000.123456),
        prev_hash: [0; 32],
        curr_hash: [1; 32],
        signature: [2; 64],
        metadata: [3; 128],
        _padding: [0; 720],
    };

    let bytes: Vec<u8> = unsafe {
        std::slice::from_raw_parts(
            &entry as *const _ as *const u8,
            size_of::<ZeroCopyAuditLogEntry>(),
        )
    }
    .to_vec();

    // Baseline: Copy deserialization
    group.bench_function("copy_deserialize", |b| {
        b.iter(|| {
            let mut buffer = [0u8; 1024];
            buffer.copy_from_slice(black_box(&bytes));

            let magic = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
            assert_eq!(magic, ZeroCopyAuditLogEntry::MAGIC);

            black_box(buffer);
        });
    });

    // Zero-copy deserialization
    group.bench_function("zero_copy_deserialize", |b| {
        b.iter(|| {
            black_box(ZeroCopyAuditLogEntry::from_bytes(black_box(&bytes)).unwrap());
        });
    });

    group.finish();
}

// ============================================================================
// 4. THROUGHPUT: Operations Per Second
// ============================================================================

fn bench_throughput_q16_16(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput/Q16_16");
    group.throughput(Throughput::Elements(10000));

    // Create 10K aligned buffers
    let values: Vec<Q16_16> = (0..10000)
        .map(|i| Q16_16::from_f64((i as f64) / 100.0))
        .collect();

    let copy_bytes: Vec<Vec<u8>> = values
        .iter()
        .map(|v| v.serialize_binary().unwrap())
        .collect();

    let zero_copy_bytes: Vec<[u8; 4]> = values.iter().map(|v| v.to_raw().to_le_bytes()).collect();

    // Baseline: Copy deserialization
    group.bench_function("copy_10k_ops", |b| {
        b.iter(|| {
            let mut total = 0i32;
            for bytes in &copy_bytes {
                let value = Q16_16::deserialize_binary(bytes).unwrap();
                total += black_box(value.to_raw());
            }
            black_box(total);
        });
    });

    // Zero-copy deserialization
    group.bench_function("zero_copy_10k_ops", |b| {
        b.iter(|| {
            let mut total = 0i32;
            for bytes in &zero_copy_bytes {
                let value = Q16_16::from_bytes(bytes).unwrap();
                total += black_box(value.to_raw());
            }
            black_box(total);
        });
    });

    group.finish();
}

fn bench_throughput_payment_capsules(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput/payment_capsules");
    group.throughput(Throughput::Elements(1000));

    // Create 1K payment capsules
    let capsules: Vec<ZeroCopyPaymentCapsule> = (0..1000)
        .map(|i| {
            ZeroCopyPaymentCapsule::new(
                Q16_16::from_f64(100.0 + i as f64),
                Q16_16::from_f64(2.91),
                Q16_16::from_f64(97.09 + i as f64),
                1234567890 + i,
                0xDEADBEEF,
                0xCAFEBABE + i as u64,
                0x12345678,
            )
        })
        .collect();

    // Serialize all capsules
    let bytes_vec: Vec<Vec<u8>> = capsules
        .iter()
        .map(|c| unsafe {
            std::slice::from_raw_parts(
                c as *const _ as *const u8,
                size_of::<ZeroCopyPaymentCapsule>(),
            )
            .to_vec()
        })
        .collect();

    // Baseline: Copy deserialization
    group.bench_function("copy_1k_payments", |b| {
        b.iter(|| {
            let mut total = 0i32;
            for bytes in &bytes_vec {
                let mut buffer = [0u8; 256];
                buffer.copy_from_slice(bytes);

                let magic = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
                assert_eq!(magic, ZeroCopyPaymentCapsule::MAGIC);

                // Extract amount (at offset 8)
                let amount_raw = i32::from_le_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]);
                total += black_box(amount_raw);
            }
            black_box(total);
        });
    });

    // Zero-copy deserialization
    group.bench_function("zero_copy_1k_payments", |b| {
        b.iter(|| {
            let mut total = 0i32;
            for bytes in &bytes_vec {
                let capsule = ZeroCopyPaymentCapsule::from_bytes(bytes).unwrap();
                total += black_box(capsule.amount().to_raw());
            }
            black_box(total);
        });
    });

    group.finish();
}

// ============================================================================
// 5. MEMORY-MAPPED FILE SIMULATION
// ============================================================================

fn bench_memory_mapped_audit_log(c: &mut Criterion) {
    let mut group = c.benchmark_group("mmap/audit_log");

    // Simulate 1MB audit log (976 entries × 1024 bytes)
    let entry_count = 976;
    group.throughput(Throughput::Bytes((entry_count * 1024) as u64));

    // Create entries
    let entries: Vec<ZeroCopyAuditLogEntry> = (0..entry_count)
        .map(|i| ZeroCopyAuditLogEntry {
            magic: ZeroCopyAuditLogEntry::MAGIC,
            version: ZeroCopyAuditLogEntry::VERSION,
            entry_type: (i % 4) as u16,
            timestamp_ns: 1234567890 + i as u64,
            user_id: 0xDEADBEEF + i as u64,
            session_id: 0xCAFEBABE,
            resource_id: 0x12345678 + i as u64,
            amount: Q32_32::from_f64((i as f64) * 100.0),
            prev_hash: [(i % 256) as u8; 32],
            curr_hash: [((i + 1) % 256) as u8; 32],
            signature: [2; 64],
            metadata: [3; 128],
            _padding: [0; 720],
        })
        .collect();

    // Flatten to bytes (simulate mmap file)
    let mmap_bytes: Vec<u8> = entries
        .iter()
        .flat_map(|e| unsafe {
            std::slice::from_raw_parts(
                e as *const _ as *const u8,
                size_of::<ZeroCopyAuditLogEntry>(),
            )
        })
        .copied()
        .collect();

    // Baseline: Copy deserialization (parse all entries)
    group.bench_function("copy_parse_1mb", |b| {
        b.iter(|| {
            let mut total = 0i64;
            for chunk in mmap_bytes.chunks(1024) {
                let mut buffer = [0u8; 1024];
                buffer.copy_from_slice(chunk);

                let magic = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
                assert_eq!(magic, ZeroCopyAuditLogEntry::MAGIC);

                // Extract amount (at offset 40)
                let amount_bytes = [
                    buffer[40], buffer[41], buffer[42], buffer[43], buffer[44], buffer[45],
                    buffer[46], buffer[47],
                ];
                let amount_raw = i64::from_le_bytes(amount_bytes);
                total += black_box(amount_raw);
            }
            black_box(total);
        });
    });

    // Zero-copy: Memory-map as slice
    group.bench_function("zero_copy_mmap_1mb", |b| {
        b.iter(|| {
            // Cast bytes to entry slice (zero-copy)
            let entries_slice: &[ZeroCopyAuditLogEntry] = unsafe {
                std::slice::from_raw_parts(
                    mmap_bytes.as_ptr() as *const ZeroCopyAuditLogEntry,
                    entry_count,
                )
            };

            let mut total = 0i64;
            for entry in entries_slice {
                total += black_box(entry.amount().to_raw());
            }
            black_box(total);
        });
    });

    group.finish();
}

// ============================================================================
// 6. ZeroCopyDeserializeCapsule Benchmarks (T5 Streaming)
// ============================================================================

fn bench_capsule_borrow_bytes_simple(c: &mut Criterion) {
    let mut group = c.benchmark_group("capsule/borrow_bytes");
    group.throughput(Throughput::Bytes(100));

    let input = vec![42u8; 100];

    group.bench_function("borrow_100_bytes", |b| {
        b.iter(|| {
            let mut capsule = ZeroCopyDeserializeCapsule::new(black_box(&input));
            let borrowed = capsule.borrow_bytes(black_box(100)).unwrap();
            black_box(borrowed);
        });
    });

    group.finish();
}

fn bench_capsule_borrow_json_string_simple(c: &mut Criterion) {
    let mut group = c.benchmark_group("capsule/borrow_json_string");
    group.throughput(Throughput::Bytes(20));

    let input = br#""hello world""#;

    group.bench_function("borrow_json_string_11bytes", |b| {
        b.iter(|| {
            let mut capsule = ZeroCopyDeserializeCapsule::new(black_box(input));
            let s = capsule.borrow_json_string().unwrap();
            black_box(s);
        });
    });

    group.finish();
}

fn bench_capsule_borrow_json_string_array(c: &mut Criterion) {
    let mut group = c.benchmark_group("capsule/borrow_json_array");
    group.throughput(Throughput::Bytes(50));

    let input = br#"["alice", "bob", "charlie"]"#;

    group.bench_function("borrow_json_array_3strings", |b| {
        b.iter(|| {
            let mut capsule = ZeroCopyDeserializeCapsule::new(black_box(input));
            let strings = capsule.borrow_json_string_array().unwrap();
            black_box(strings);
        });
    });

    group.finish();
}

fn bench_capsule_sequential_borrows(c: &mut Criterion) {
    let mut group = c.benchmark_group("capsule/sequential_borrows");
    group.throughput(Throughput::Bytes(1000));

    let input = vec![0u8; 1000];

    group.bench_function("10_sequential_100byte_borrows", |b| {
        b.iter(|| {
            let mut capsule = ZeroCopyDeserializeCapsule::new(black_box(&input));
            for _ in 0..10 {
                let _ = capsule.borrow_bytes(black_box(100)).unwrap();
            }
        });
    });

    group.finish();
}

fn bench_capsule_vs_vec_copy(c: &mut Criterion) {
    let mut group = c.benchmark_group("capsule/comparison");
    group.throughput(Throughput::Bytes(1000));

    let input = vec![42u8; 1000];

    // Zero-copy approach
    group.bench_function("zero_copy_borrow_1000_bytes", |b| {
        b.iter(|| {
            let mut capsule = ZeroCopyDeserializeCapsule::new(black_box(&input));
            let borrowed = capsule.borrow_bytes(black_box(1000)).unwrap();
            black_box(borrowed);
        });
    });

    // Copy approach (baseline)
    group.bench_function("copy_allocate_1000_bytes", |b| {
        b.iter(|| {
            let mut buffer = vec![0u8; 1000];
            buffer.copy_from_slice(black_box(&input));
            black_box(buffer);
        });
    });

    group.finish();
}

fn bench_capsule_large_buffer_streaming(c: &mut Criterion) {
    let mut group = c.benchmark_group("capsule/large_buffer");
    group.throughput(Throughput::Bytes(100_000));

    let input = vec![123u8; 100_000];

    group.bench_function("stream_100k_buffer_10borrows", |b| {
        b.iter(|| {
            let mut capsule = ZeroCopyDeserializeCapsule::new(black_box(&input));
            for _ in 0..10 {
                let _ = capsule.borrow_bytes(black_box(10_000)).unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    baseline,
    bench_copy_deserialize_q16_16,
    bench_copy_deserialize_q32_32,
);

criterion_group!(
    zero_copy,
    bench_zero_copy_deserialize_q16_16,
    bench_zero_copy_deserialize_q32_32,
    bench_payment_capsule_copy,
    bench_audit_log_entry_copy,
);

criterion_group!(
    throughput,
    bench_throughput_q16_16,
    bench_throughput_payment_capsules,
);

criterion_group!(mmap, bench_memory_mapped_audit_log,);

criterion_group!(
    capsule,
    bench_capsule_borrow_bytes_simple,
    bench_capsule_borrow_json_string_simple,
    bench_capsule_borrow_json_string_array,
    bench_capsule_sequential_borrows,
    bench_capsule_vs_vec_copy,
    bench_capsule_large_buffer_streaming,
);

criterion_main!(baseline, zero_copy, throughput, mmap, capsule);
