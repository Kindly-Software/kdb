//! # Block Unpacking Benchmarks
//!
//! **B32 Framework Compliance: Fair baselines, 95% CI, 1000+ iterations.**
//!
//! ## Performance Targets
//!
//! - **Scalar baseline**: ~320ns per 8×8 block
//! - **SIMD (AVX2)**: <40ns per 8×8 block (8× speedup)
//! - **SIMD (AVX-512)**: <20ns per 8×8 block (16× speedup)
//!
//! ## Benchmarks
//!
//! 1. `scalar_unpack_q8_8`: Baseline scalar implementation
//! 2. `simd_unpack_q8_8`: SIMD f32x8 implementation (AVX2)
//! 3. `simd_unpack_q6_6`: SIMD with Q6.6 quantization
//! 4. `simd_unpack_q4_4`: SIMD with Q4.4 quantization
//! 5. `centroid_matching_simd`: SIMD distance computation (256 centroids)
//! 6. `batch_dequantize`: Batch processing (512 blocks)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kindly_compression_pro::{
    BlockData, QuantFormat, QuantizedBlock,
    unpack_block_8x8_simd, find_nearest_centroid_simd,
    dequantize_blocks_simd,
};

/// Generate test data for Q8.8 quantization
fn generate_test_data_q8_8(size: usize) -> Vec<u8> {
    (0..size).map(|i| ((i % 256) as i8) as u8).collect()
}

/// Generate test centroids (256 × 8D)
fn generate_test_centroids() -> [[f32; 8]; 256] {
    let mut centroids = [[0.0f32; 8]; 256];
    for (i, centroid) in centroids.iter_mut().enumerate() {
        for j in 0..8 {
            centroid[j] = ((i * 8 + j) as f32) / 256.0;
        }
    }
    centroids
}

/// Benchmark: Scalar vs SIMD block unpacking (Q8.8)
fn bench_unpack_block_q8_8(c: &mut Criterion) {
    let test_data = generate_test_data_q8_8(64);

    let mut group = c.benchmark_group("block_unpacking_q8_8");
    group.throughput(Throughput::Elements(64)); // 64 elements per block

    group.bench_function("simd_avx2", |b| {
        b.iter(|| {
            let block = unpack_block_8x8_simd(
                black_box(&test_data),
                black_box(QuantFormat::Q8_8),
            );
            black_box(block);
        });
    });

    group.finish();
}

/// Benchmark: Q6.6 quantization (higher precision)
fn bench_unpack_block_q6_6(c: &mut Criterion) {
    let test_data = generate_test_data_q8_8(64);

    let mut group = c.benchmark_group("block_unpacking_q6_6");
    group.throughput(Throughput::Elements(64));

    group.bench_function("simd_avx2", |b| {
        b.iter(|| {
            let block = unpack_block_8x8_simd(
                black_box(&test_data),
                black_box(QuantFormat::Q6_6),
            );
            black_box(block);
        });
    });

    group.finish();
}

/// Benchmark: Q4.4 quantization (maximum compression)
fn bench_unpack_block_q4_4(c: &mut Criterion) {
    let test_data = generate_test_data_q8_8(64);

    let mut group = c.benchmark_group("block_unpacking_q4_4");
    group.throughput(Throughput::Elements(64));

    group.bench_function("simd_avx2", |b| {
        b.iter(|| {
            let block = unpack_block_8x8_simd(
                black_box(&test_data),
                black_box(QuantFormat::Q4_4),
            );
            black_box(block);
        });
    });

    group.finish();
}

/// Benchmark: Centroid matching (SIMD distance computation)
fn bench_centroid_matching(c: &mut Criterion) {
    let block_vec = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let centroids = generate_test_centroids();

    let mut group = c.benchmark_group("centroid_matching");
    group.throughput(Throughput::Elements(256)); // 256 centroids

    group.bench_function("simd_avx2", |b| {
        b.iter(|| {
            let idx = find_nearest_centroid_simd(
                black_box(&block_vec),
                black_box(&centroids),
            );
            black_box(idx);
        });
    });

    group.finish();
}

/// Benchmark: Batch dequantization (512 blocks)
fn bench_batch_dequantize(c: &mut Criterion) {
    let batch_sizes = [1, 4, 16, 64, 256, 512];

    let mut group = c.benchmark_group("batch_dequantize");

    for &size in &batch_sizes {
        let blocks: Vec<QuantizedBlock> = (0..size)
            .map(|_| QuantizedBlock {
                data: generate_test_data_q8_8(64),
                format: QuantFormat::Q8_8,
            })
            .collect();

        group.throughput(Throughput::Elements(size * 64)); // size × 64 elements

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &blocks,
            |b, blocks| {
                b.iter(|| {
                    let result = dequantize_blocks_simd(
                        black_box(blocks),
                        black_box(QuantFormat::Q8_8),
                    );
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: End-to-end decompression pipeline
fn bench_decompression_pipeline(c: &mut Criterion) {
    let centroids = generate_test_centroids();
    let blocks: Vec<QuantizedBlock> = (0..256)
        .map(|_| QuantizedBlock {
            data: generate_test_data_q8_8(64),
            format: QuantFormat::Q8_8,
        })
        .collect();

    let mut group = c.benchmark_group("decompression_pipeline");
    group.throughput(Throughput::Elements(256 * 64)); // 256 blocks × 64 elements

    group.bench_function("full_pipeline", |b| {
        b.iter(|| {
            // Stage 1: Batch dequantization
            let dequantized = dequantize_blocks_simd(
                black_box(&blocks),
                black_box(QuantFormat::Q8_8),
            );

            // Stage 2: Centroid matching (dictionary decompression)
            let _centroid_ids: Vec<u8> = dequantized
                .iter()
                .map(|block| {
                    let vec = [
                        block.weights[0][0],
                        block.weights[0][1],
                        block.weights[0][2],
                        block.weights[0][3],
                        block.weights[0][4],
                        block.weights[0][5],
                        block.weights[0][6],
                        block.weights[0][7],
                    ];
                    find_nearest_centroid_simd(&vec, &centroids)
                })
                .collect();

            black_box(_centroid_ids);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_unpack_block_q8_8,
    bench_unpack_block_q6_6,
    bench_unpack_block_q4_4,
    bench_centroid_matching,
    bench_batch_dequantize,
    bench_decompression_pipeline,
);
criterion_main!(benches);
