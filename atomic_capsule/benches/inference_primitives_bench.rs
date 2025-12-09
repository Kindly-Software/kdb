//! # B32-Compliant Inference Primitives Benchmarks
//!
//! **Wave 6: Comprehensive B32 benchmarks for neural network inference capsules.**
//!
//! ## B32 Framework Compliance
//!
//! - Fair baselines (scalar/naive implementations)
//! - 1000+ iterations for statistical significance
//! - 95% confidence intervals via Criterion
//! - Realistic workloads (production-scale data)
//!
//! ## Capsules Benchmarked
//!
//! 1. **QuantizationCapsule** (T3) - INT8 quantization with fixed-point
//! 2. **SIMDMatMulCapsule** (T2+T4) - Vectorized matrix multiplication
//! 3. **FlashAttentionCapsule** (T2+T5) - Memory-efficient attention
//! 4. **Q4KMSuperBlockCapsule** (T3+T4) - GGUF-compatible 4-bit quantization
//! 5. **VramCacheCapsule** (T1) - GPU memory cache with CLOCK eviction
//! 6. **WeightAuditCapsule** (T0+T1) - Q34 weight integrity verification
//!
//! ## Run Instructions
//!
//! ```bash
//! # Run on kindly-hub (MANDATORY for B32 compliance)
//! ssh samuel@kindly-hub "cd ~/Primitives/atomic_capsule && \
//!     cargo bench --bench inference_primitives_bench \
//!     --features inference-all"
//! ```

#![feature(portable_simd)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::simd::f32x8;
use std::time::Duration;

use atomic_capsule::primitives::inference::{
    FlashAttentionCapsule, Q4KMSuperBlockCapsule, Q4KMTensor, QuantizationCapsule,
    SIMDMatMulCapsule, VramCacheCapsule, WeightAuditCapsule, fnv1a_hash,
};

// ============================================================================
// B32 K1-K10: QUANTIZATION CAPSULE BENCHMARKS (T3)
// ============================================================================

/// Scalar baseline for quantization (naive implementation)
fn baseline_quantize_scalar(weights: &[f32], scale: f32) -> Vec<i16> {
    weights
        .iter()
        .map(|&w| {
            let scaled = (w / scale).round();
            let clamped = scaled.clamp(-128.0, 127.0);
            (clamped * 256.0).round() as i16
        })
        .collect()
}

/// Scalar baseline for dequantization
fn baseline_dequantize_scalar(quantized: &[i16], scale: f32) -> Vec<f32> {
    quantized
        .iter()
        .map(|&q| {
            let fp = q as f32 / 256.0;
            fp * scale
        })
        .collect()
}

fn quantization_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantization_capsule");

    // B32 K2: Statistical rigor - 1000+ iterations, 95% CI
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));

    // Test varying input sizes (B32 K3: Realistic workloads)
    for size in [64, 256, 1024, 4096, 16384] {
        let weights: Vec<f32> = (0..size).map(|i| (i as f32 / size as f32) * 20.0 - 10.0).collect();
        let scale = 10.0 / 127.0;
        let quant = QuantizationCapsule::from_range(-10.0, 10.0);

        group.throughput(Throughput::Elements(size as u64));

        // B32 K1: Fair scalar baseline
        group.bench_with_input(BenchmarkId::new("scalar_quantize", size), &size, |b, _| {
            b.iter(|| black_box(baseline_quantize_scalar(&weights, scale)))
        });

        // Capsule quantize (scalar path)
        group.bench_with_input(BenchmarkId::new("capsule_quantize", size), &size, |b, _| {
            b.iter(|| black_box(quant.quantize(&weights)))
        });

        // SIMD quantize (8-wide vectorized)
        if size >= 8 && size % 8 == 0 {
            group.bench_with_input(BenchmarkId::new("simd_quantize", size), &size, |b, _| {
                b.iter(|| black_box(quant.quantize_simd(&weights)))
            });
        }

        // Dequantization benchmarks
        let quantized = quant.quantize(&weights);

        group.bench_with_input(BenchmarkId::new("scalar_dequantize", size), &size, |b, _| {
            b.iter(|| black_box(baseline_dequantize_scalar(&quantized, scale)))
        });

        group.bench_with_input(BenchmarkId::new("capsule_dequantize", size), &size, |b, _| {
            b.iter(|| black_box(quant.dequantize(&quantized)))
        });

        if size >= 8 && size % 8 == 0 {
            group.bench_with_input(BenchmarkId::new("simd_dequantize", size), &size, |b, _| {
                b.iter(|| black_box(quant.dequantize_simd(&quantized)))
            });
        }
    }

    group.finish();
}

// ============================================================================
// B32 K11-K20: SIMD MATMUL CAPSULE BENCHMARKS (T2+T4)
// ============================================================================

/// Naive nested loop matrix-vector multiply baseline
fn baseline_matmul_naive(weights: &[f32], input: &[f32], rows: usize, cols: usize) -> Vec<f32> {
    let mut output = vec![0.0f32; rows];
    for row in 0..rows {
        let mut sum = 0.0f32;
        for col in 0..cols {
            sum += weights[row * cols + col] * input[col];
        }
        output[row] = sum;
    }
    output
}

/// Optimized scalar baseline (cache-friendly iteration)
fn baseline_matmul_optimized(weights: &[f32], input: &[f32], rows: usize, cols: usize) -> Vec<f32> {
    let mut output = vec![0.0f32; rows];
    // Column-major iteration for better cache behavior
    for col in 0..cols {
        let input_val = input[col];
        for row in 0..rows {
            output[row] += weights[row * cols + col] * input_val;
        }
    }
    output
}

fn simd_matmul_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_matmul_capsule");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));

    // Test varying matrix sizes (must be multiple of 8 for SIMD)
    for (rows, cols) in [(8, 8), (64, 64), (256, 256), (512, 512), (1024, 1024)] {
        let weights: Vec<f32> = (0..rows * cols).map(|i| (i as f32).sin()).collect();
        let input: Vec<f32> = (0..cols).map(|i| (i as f32) / cols as f32).collect();

        group.throughput(Throughput::Elements((rows * cols) as u64));

        // B32 K1: Naive baseline (strawman comparison - for context only)
        group.bench_with_input(BenchmarkId::new("naive_baseline", format!("{}x{}", rows, cols)), &(rows, cols), |b, _| {
            b.iter(|| black_box(baseline_matmul_naive(&weights, &input, rows, cols)))
        });

        // B32 K1: Optimized scalar baseline (fair comparison)
        group.bench_with_input(BenchmarkId::new("optimized_scalar", format!("{}x{}", rows, cols)), &(rows, cols), |b, _| {
            b.iter(|| black_box(baseline_matmul_optimized(&weights, &input, rows, cols)))
        });

        // SIMD MatMul Capsule
        let matmul = SIMDMatMulCapsule::from_weights(weights.clone(), rows, cols);
        group.bench_with_input(BenchmarkId::new("simd_capsule", format!("{}x{}", rows, cols)), &(rows, cols), |b, _| {
            b.iter(|| black_box(matmul.forward(&input)))
        });

        // SIMD MatMul with fused ReLU
        group.bench_with_input(BenchmarkId::new("simd_capsule_relu", format!("{}x{}", rows, cols)), &(rows, cols), |b, _| {
            b.iter(|| black_box(matmul.forward_relu(&input)))
        });
    }

    // Batch processing benchmarks (T4 tier)
    let rows = 64;
    let cols = 64;
    let weights: Vec<f32> = (0..rows * cols).map(|i| (i as f32).sin()).collect();
    let matmul = SIMDMatMulCapsule::from_weights(weights.clone(), rows, cols);

    for batch_size in [4, 16, 64, 256] {
        let inputs: Vec<Vec<f32>> = (0..batch_size)
            .map(|b| (0..cols).map(|i| ((b * cols + i) as f32) / (batch_size * cols) as f32).collect())
            .collect();

        group.throughput(Throughput::Elements((batch_size * rows * cols) as u64));

        group.bench_with_input(BenchmarkId::new("batch_forward", batch_size), &batch_size, |b, _| {
            b.iter(|| black_box(matmul.forward_batch(&inputs)))
        });
    }

    group.finish();
}

// ============================================================================
// B32 K21-K30: FLASH ATTENTION CAPSULE BENCHMARKS (T2+T5)
// ============================================================================

/// Standard O(N^2) attention baseline
fn baseline_attention_standard(q: &[f32], k: &[f32], v: &[f32], seq_len: usize) -> Vec<f32> {
    let d_model = q.len() / seq_len;
    let scale = (d_model as f32).sqrt().recip();

    // Compute attention scores: Q @ K^T
    let mut scores = vec![0.0f32; seq_len * seq_len];
    for i in 0..seq_len {
        for j in 0..seq_len {
            let mut dot = 0.0f32;
            for d in 0..d_model {
                dot += q[i * d_model + d] * k[j * d_model + d];
            }
            scores[i * seq_len + j] = dot * scale;
        }
    }

    // Softmax per row
    for i in 0..seq_len {
        let row_start = i * seq_len;
        let max_score = scores[row_start..row_start + seq_len]
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);

        let mut sum_exp = 0.0f32;
        for j in 0..seq_len {
            scores[row_start + j] = (scores[row_start + j] - max_score).exp();
            sum_exp += scores[row_start + j];
        }
        for j in 0..seq_len {
            scores[row_start + j] /= sum_exp;
        }
    }

    // Apply attention to values: softmax(scores) @ V
    let mut output = vec![0.0f32; seq_len * d_model];
    for i in 0..seq_len {
        for j in 0..seq_len {
            let attn_weight = scores[i * seq_len + j];
            for d in 0..d_model {
                output[i * d_model + d] += attn_weight * v[j * d_model + d];
            }
        }
    }

    output
}

fn flash_attention_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("flash_attention_capsule");

    group
        .confidence_level(0.95)
        .sample_size(500) // Reduced due to heavier computation
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));

    // Test varying sequence lengths (B32 K3: Realistic LLM workloads)
    for seq_len in [8, 16, 32, 64, 128] {
        let d_model = 64; // Head dimension (must be multiple of 8)
        let total_elements = seq_len * d_model;

        // Generate realistic attention inputs
        let q: Vec<f32> = (0..total_elements).map(|i| (i as f32 * 0.01).sin()).collect();
        let k: Vec<f32> = (0..total_elements).map(|i| (i as f32 * 0.02).cos()).collect();
        let v: Vec<f32> = (0..total_elements).map(|i| (i as f32 * 0.03).sin()).collect();

        group.throughput(Throughput::Elements((seq_len * seq_len) as u64)); // Attention scores

        // B32 K1: Standard O(N^2) attention baseline
        group.bench_with_input(BenchmarkId::new("standard_attention", seq_len), &seq_len, |b, _| {
            b.iter(|| black_box(baseline_attention_standard(&q, &k, &v, seq_len)))
        });

        // Flash Attention Capsule (streaming API with SIMD packing)
        let attention = FlashAttentionCapsule::new(128);
        group.bench_with_input(BenchmarkId::new("flash_attention_streaming", seq_len), &seq_len, |b, _| {
            b.iter(|| black_box(attention.forward_streaming(&q, &k, &v)))
        });

        // Flash Attention with pre-packed SIMD vectors
        if total_elements % 8 == 0 {
            let q_simd: Vec<f32x8> = q.chunks(8).map(|c| f32x8::from_slice(c)).collect();
            let k_simd: Vec<f32x8> = k.chunks(8).map(|c| f32x8::from_slice(c)).collect();
            let v_simd: Vec<f32x8> = v.chunks(8).map(|c| f32x8::from_slice(c)).collect();

            group.bench_with_input(BenchmarkId::new("flash_attention_simd", seq_len), &seq_len, |b, _| {
                b.iter(|| black_box(attention.forward(&q_simd, &k_simd, &v_simd)))
            });
        }
    }

    // Block size comparison
    let seq_len = 64;
    let d_model = 64;
    let total_elements = seq_len * d_model;
    let q: Vec<f32> = (0..total_elements).map(|i| (i as f32 * 0.01).sin()).collect();
    let k: Vec<f32> = (0..total_elements).map(|i| (i as f32 * 0.02).cos()).collect();
    let v: Vec<f32> = (0..total_elements).map(|i| (i as f32 * 0.03).sin()).collect();

    for block_size in [32, 64, 128, 256] {
        let attention = FlashAttentionCapsule::new(block_size);
        group.bench_with_input(BenchmarkId::new("block_size", block_size), &block_size, |b, _| {
            b.iter(|| black_box(attention.forward_streaming(&q, &k, &v)))
        });
    }

    group.finish();
}

// ============================================================================
// B32 K31-K40: Q4_K_M SUPER BLOCK CAPSULE BENCHMARKS (T3+T4)
// ============================================================================

/// Naive 4-bit packing baseline
fn baseline_quantize_4bit(weights: &[f32; 256]) -> [u8; 128] {
    let mut result = [0u8; 128];

    // Simple uniform quantization
    let min = weights.iter().copied().fold(f32::INFINITY, f32::min);
    let max = weights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let scale = (max - min) / 15.0;

    for i in 0..128 {
        let w0 = weights[i * 2];
        let w1 = weights[i * 2 + 1];

        let q0 = ((w0 - min) / scale).round().clamp(0.0, 15.0) as u8;
        let q1 = ((w1 - min) / scale).round().clamp(0.0, 15.0) as u8;

        result[i] = q0 | (q1 << 4);
    }

    result
}

/// Naive 4-bit dequantization baseline
fn baseline_dequantize_4bit(packed: &[u8; 128], scale: f32, min: f32) -> [f32; 256] {
    let mut result = [0.0f32; 256];

    for i in 0..128 {
        let q0 = (packed[i] & 0x0F) as f32;
        let q1 = ((packed[i] >> 4) & 0x0F) as f32;

        result[i * 2] = q0 * scale + min;
        result[i * 2 + 1] = q1 * scale + min;
    }

    result
}

fn q4km_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("q4km_superblock_capsule");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));

    // Single super-block (256 weights)
    let weights: [f32; 256] = core::array::from_fn(|i| (i as f32 / 256.0) * 10.0 - 5.0);
    let scale = 10.0 / 15.0;
    let min = -5.0f32;

    group.throughput(Throughput::Elements(256));

    // B32 K1: Naive 4-bit packing baseline
    group.bench_function("naive_quantize_256", |b| {
        b.iter(|| black_box(baseline_quantize_4bit(&weights)))
    });

    // Q4KM capsule quantize
    group.bench_function("capsule_quantize_256", |b| {
        b.iter(|| black_box(Q4KMSuperBlockCapsule::from_f32_weights(&weights)))
    });

    // Dequantization benchmarks
    let block = Q4KMSuperBlockCapsule::from_f32_weights(&weights);
    let packed = baseline_quantize_4bit(&weights);

    group.bench_function("naive_dequantize_256", |b| {
        b.iter(|| black_box(baseline_dequantize_4bit(&packed, scale, min)))
    });

    group.bench_function("capsule_dequantize_256", |b| {
        b.iter(|| black_box(block.dequantize_256()))
    });

    group.bench_function("capsule_dequantize_256_f32", |b| {
        b.iter(|| black_box(block.dequantize_256_f32()))
    });

    // SIMD dequantization
    group.bench_function("capsule_dequantize_256_simd", |b| {
        b.iter(|| black_box(block.dequantize_256_simd()))
    });

    // Sub-block benchmarks (32 weights)
    group.throughput(Throughput::Elements(32));

    for sub_block in [0, 3, 7] {
        group.bench_with_input(BenchmarkId::new("dequantize_sub_block", sub_block), &sub_block, |b, &sb| {
            b.iter(|| black_box(block.dequantize_sub_block(sb)))
        });
    }

    // Single weight random access
    group.throughput(Throughput::Elements(1));
    group.bench_function("dequantize_single", |b| {
        let mut idx = 0usize;
        b.iter(|| {
            idx = (idx + 7) % 256; // Pseudo-random access
            black_box(block.dequantize_one(idx))
        })
    });

    // Tensor-level benchmarks (multiple super-blocks)
    for num_weights in [1024, 4096, 16384, 65536] {
        let tensor_weights: Vec<f32> = (0..num_weights).map(|i| (i as f32 / num_weights as f32) * 10.0 - 5.0).collect();
        let tensor = Q4KMTensor::from_f32(&tensor_weights);

        group.throughput(Throughput::Elements(num_weights as u64));

        group.bench_with_input(BenchmarkId::new("tensor_dequantize_all", num_weights), &num_weights, |b, _| {
            b.iter(|| black_box(tensor.dequantize_all()))
        });
    }

    group.finish();
}

// ============================================================================
// B32 K41-K50: VRAM CACHE CAPSULE BENCHMARKS (T1)
// ============================================================================

fn vram_cache_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("vram_cache_capsule");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));

    // Lookup benchmarks
    let cache = VramCacheCapsule::new(16);

    // Populate cache
    for i in 0..16 {
        let _ = cache.insert(i * 100);
    }

    // B32 K1: HashMap baseline
    let mut hashmap_baseline: HashMap<u64, usize> = HashMap::new();
    for i in 0..16u64 {
        hashmap_baseline.insert(i * 100, i as usize);
    }

    group.bench_function("hashmap_lookup", |b| {
        let mut key = 0u64;
        b.iter(|| {
            key = (key + 100) % 1600;
            black_box(hashmap_baseline.get(&key))
        })
    });

    group.bench_function("capsule_lookup_hit", |b| {
        let mut key = 0u64;
        b.iter(|| {
            key = (key + 100) % 1600;
            black_box(cache.lookup(key))
        })
    });

    group.bench_function("capsule_lookup_miss", |b| {
        b.iter(|| black_box(cache.lookup(9999)))
    });

    // Insert benchmarks
    group.bench_function("capsule_insert_existing", |b| {
        let mut key = 0u64;
        b.iter(|| {
            key = (key + 100) % 1600;
            black_box(cache.insert(key))
        })
    });

    // Pin/unpin benchmarks
    let pin_cache = VramCacheCapsule::new(16);
    let _ = pin_cache.insert(42);

    group.bench_function("capsule_is_pinned", |b| {
        b.iter(|| black_box(pin_cache.is_pinned(42)))
    });

    // Metrics snapshot
    group.bench_function("capsule_metrics", |b| {
        b.iter(|| black_box(cache.metrics()))
    });

    group.bench_function("capsule_snapshot", |b| {
        b.iter(|| black_box(cache.snapshot()))
    });

    // Eviction benchmark (need full cache)
    let evict_cache = VramCacheCapsule::new(4);
    for i in 0..4 {
        let _ = evict_cache.insert(i * 10);
    }

    group.bench_function("capsule_evict_one", |b| {
        b.iter(|| {
            // Insert triggers eviction since cache is full
            let _ = evict_cache.insert(black_box(99999));
        })
    });

    group.finish();
}

// ============================================================================
// B32 K51-K60: WEIGHT AUDIT CAPSULE BENCHMARKS (T0+T1)
// ============================================================================

/// Manual FNV-1a hash computation baseline
fn baseline_fnv1a(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Manual hash chain update baseline
fn baseline_chain_update(current: u64, block_hash: u64) -> u64 {
    const FNV_PRIME: u64 = 1099511628211;
    (current ^ block_hash).wrapping_mul(FNV_PRIME)
}

fn weight_audit_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("weight_audit_capsule");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));

    // Hash throughput benchmarks
    for size in [64, 256, 1024, 4096, 16384] {
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

        group.throughput(Throughput::Bytes(size as u64));

        // B32 K1: Manual baseline
        group.bench_with_input(BenchmarkId::new("manual_fnv1a", size), &size, |b, _| {
            b.iter(|| black_box(baseline_fnv1a(&data)))
        });

        // Capsule FNV-1a (const fn)
        group.bench_with_input(BenchmarkId::new("capsule_fnv1a", size), &size, |b, _| {
            b.iter(|| black_box(fnv1a_hash(&data)))
        });
    }

    // Chain hash update benchmarks
    let audit = WeightAuditCapsule::new();

    group.bench_function("manual_chain_update", |b| {
        let mut current = 14695981039346656037u64;
        let block_hash = 0x123456789ABCDEF0u64;
        b.iter(|| {
            current = baseline_chain_update(current, block_hash);
            black_box(current)
        })
    });

    group.bench_function("capsule_chain_update", |b| {
        let block_hash = 0x123456789ABCDEF0u64;
        b.iter(|| black_box(audit.update_chain_hash(block_hash)))
    });

    // Verification benchmarks
    let mut verify_audit = WeightAuditCapsule::new();
    let block_data: Vec<Vec<u8>> = (0..64)
        .map(|i| {
            (0..4096).map(|j| ((i * 1000 + j) % 256) as u8).collect()
        })
        .collect();
    let expected_hashes: Vec<u64> = block_data.iter().map(|d| fnv1a_hash(d)).collect();
    verify_audit.set_expected_hashes(&expected_hashes).unwrap();

    group.bench_function("capsule_verify_block_4kb", |b| {
        let mut idx = 0usize;
        b.iter(|| {
            idx = (idx + 1) % 64;
            black_box(verify_audit.verify_block(idx as u64, &block_data[idx]))
        })
    });

    group.bench_function("capsule_mark_verified", |b| {
        b.iter(|| black_box(verify_audit.mark_verified(0)))
    });

    group.bench_function("capsule_is_verified", |b| {
        let mut idx = 0u64;
        b.iter(|| {
            idx = (idx + 1) % 64;
            black_box(verify_audit.is_verified(idx))
        })
    });

    // Metrics and snapshot
    group.bench_function("capsule_metrics", |b| {
        b.iter(|| black_box(verify_audit.metrics()))
    });

    group.bench_function("capsule_snapshot", |b| {
        b.iter(|| black_box(verify_audit.snapshot()))
    });

    // Merkle root verification
    let mut merkle_audit = WeightAuditCapsule::new();
    let root: u128 = 0x123456789ABCDEF0_FEDCBA9876543210;
    merkle_audit.set_merkle_root(root);

    group.bench_function("capsule_verify_merkle_root", |b| {
        b.iter(|| black_box(merkle_audit.verify_merkle_root(root)))
    });

    group.finish();
}

// ============================================================================
// B32 K61-K70: COMBINED PIPELINE BENCHMARKS
// ============================================================================

fn inference_pipeline_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("inference_pipeline");

    group
        .confidence_level(0.95)
        .sample_size(500)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));

    // Full pipeline: Quantize -> MatMul -> Attention -> Dequantize
    let quant = QuantizationCapsule::from_range(-10.0, 10.0);
    let weights: Vec<f32> = (0..64 * 64).map(|i| (i as f32 / 4096.0) * 20.0 - 10.0).collect();
    let matmul = SIMDMatMulCapsule::from_weights(weights.clone(), 64, 64);
    let attention = FlashAttentionCapsule::new(128);

    let input: Vec<f32> = (0..64).map(|i| (i as f32) / 64.0).collect();

    group.bench_function("full_forward_pass", |b| {
        b.iter(|| {
            // Step 1: Quantize input (simulating weight loading)
            let quantized = quant.quantize(&input);

            // Step 2: Dequantize for computation
            let dequantized = quant.dequantize(&quantized);

            // Step 3: Matrix multiply
            let hidden = matmul.forward(&dequantized);

            // Step 4: Self-attention (simplified: Q=K=V=hidden)
            let output = attention.forward_streaming(&hidden, &hidden, &hidden);

            black_box(output)
        })
    });

    // Q4KM quantized inference pipeline
    let q4_weights: [f32; 256] = core::array::from_fn(|i| (i as f32 / 256.0) * 10.0 - 5.0);
    let q4_block = Q4KMSuperBlockCapsule::from_f32_weights(&q4_weights);

    group.bench_function("q4km_dequant_forward", |b| {
        b.iter(|| {
            // Step 1: Dequantize Q4KM weights
            let weights_f32 = q4_block.dequantize_256_f32();

            // Step 2: Use in computation (simplified dot product)
            let input_256: [f32; 256] = core::array::from_fn(|i| (i as f32) / 256.0);
            let mut sum = 0.0f32;
            for i in 0..256 {
                sum += weights_f32[i] * input_256[i];
            }
            black_box(sum)
        })
    });

    // Audit + Cache + Inference pipeline
    group.bench_function("audited_cached_inference", |b| {
        let cache = VramCacheCapsule::new(16);
        let audit = WeightAuditCapsule::new();
        let block_data = [0u8; 4096];

        b.iter(|| {
            // Step 1: Check cache
            let cached = cache.lookup(42);

            // Step 2: If not cached, audit and insert
            if cached.is_none() {
                let _hash = fnv1a_hash(&block_data);
                let _ = cache.insert(42);
            }

            // Step 3: Update chain hash
            let chain_hash = audit.update_chain_hash(0x12345678);

            black_box(chain_hash)
        })
    });

    group.finish();
}

// ============================================================================
// CRITERION MAIN
// ============================================================================

criterion_group!(
    name = quantization;
    config = Criterion::default();
    targets = quantization_benchmarks
);

criterion_group!(
    name = matmul;
    config = Criterion::default();
    targets = simd_matmul_benchmarks
);

criterion_group!(
    name = attention;
    config = Criterion::default();
    targets = flash_attention_benchmarks
);

criterion_group!(
    name = q4km;
    config = Criterion::default();
    targets = q4km_benchmarks
);

criterion_group!(
    name = vram_cache;
    config = Criterion::default();
    targets = vram_cache_benchmarks
);

criterion_group!(
    name = weight_audit;
    config = Criterion::default();
    targets = weight_audit_benchmarks
);

criterion_group!(
    name = pipeline;
    config = Criterion::default();
    targets = inference_pipeline_benchmarks
);

criterion_main!(quantization, matmul, attention, q4km, vram_cache, weight_audit, pipeline);
