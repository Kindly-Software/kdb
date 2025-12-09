//! Tier 2 (SIMD) Benchmark: Hebbian 19× Speedup Validation
//!
//! B32 Compliance:
//! - B1: Fair baseline (optimized scalar Hebbian learning)
//! - B2: Statistical rigor (1000+ samples, 95% CI)
//! - B3: Realistic workloads (6-element connection batch)
//! - K9: SIMD reality (19× is EXCEPTIONAL, requires proof)
//! - K27: Honest gains (validate extraordinary claim)
//!
//! Proven: 19× speedup in kindly_hft Hebbian learning
//! Innovation: Batch 6 connections in single f32x8 SIMD operation
//! Target: 15-20× speedup (validate documented claim)

#![feature(portable_simd)]

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::simd::{f32x8, num::SimdFloat};
use std::time::Duration;

/// Hebbian learning: Δw = η × pre × post
/// Where: η = learning rate, pre = presynaptic, post = postsynaptic

/// B32 B1: Optimized scalar Hebbian update (6 connections)
fn scalar_hebbian_update(
    weights: &mut [f32; 6],
    pre: &[f32; 6],
    post: &[f32; 6],
    learning_rate: f32,
) {
    for i in 0..6 {
        weights[i] += learning_rate * pre[i] * post[i];
    }
}

/// SIMD Hebbian update (6 connections in f32x8)
/// Innovation: Pack 6 weights + 2 padding into single SIMD vector
fn simd_hebbian_update(
    weights: &mut [f32; 8],  // 6 weights + 2 padding
    pre: &[f32; 8],
    post: &[f32; 8],
    learning_rate: f32,
) {
    let w = f32x8::from_array(*weights);
    let p = f32x8::from_array(*pre);
    let o = f32x8::from_array(*post);
    let lr = f32x8::splat(learning_rate);

    // Δw = η × pre × post
    let delta = lr * p * o;
    let new_w = w + delta;

    *weights = new_w.to_array();
}

/// B32 B1-B3: Single connection update
fn bench_single_connection_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_connection");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(3));

    let learning_rate = 0.01;

    // Baseline: Scalar (single connection)
    group.bench_function("scalar", |b| {
        let mut weight = 0.5;
        let pre = 0.8;
        let post = 0.6;

        b.iter(|| {
            weight += black_box(learning_rate * pre * post);
            black_box(weight)
        });
    });

    // SIMD: Single connection (overhead visible)
    group.bench_function("simd_single", |b| {
        let mut weights = [0.5f32; 8];
        let pre = [0.8f32; 8];
        let post = [0.6f32; 8];

        b.iter(|| {
            simd_hebbian_update(&mut weights, &pre, &post, black_box(learning_rate));
            black_box(weights[0])
        });
    });

    group.finish();
}

/// B32 B1-B3: 6-connection batch (the breakthrough)
/// Target: 15-20× speedup (validate 19× claim)
fn bench_6_connection_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("6_connection_batch");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(3));

    let learning_rate = 0.01;

    // Baseline: Scalar (6 connections, sequential)
    group.bench_function("scalar", |b| {
        let mut weights = [0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let pre = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        let post = [0.9, 0.8, 0.7, 0.6, 0.5, 0.4];

        b.iter(|| {
            scalar_hebbian_update(
                &mut weights,
                &pre,
                &post,
                black_box(learning_rate),
            );
            black_box(weights)
        });
    });

    // SIMD: 6 connections in single f32x8 operation
    group.bench_function("simd", |b| {
        let mut weights = [0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.0, 0.0];
        let pre = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.0, 0.0];
        let post = [0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.0, 0.0];

        b.iter(|| {
            simd_hebbian_update(
                &mut weights,
                &pre,
                &post,
                black_box(learning_rate),
            );
            black_box(weights)
        });
    });

    group.finish();
}

/// B32 B3: Realistic neuron update (100 connections per neuron)
fn bench_neuron_update_100_connections(c: &mut Criterion) {
    let mut group = c.benchmark_group("neuron_100_connections");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    let learning_rate = 0.01;
    let num_connections = 100;

    // Generate test data
    let mut weights: Vec<f32> = (0..num_connections).map(|i| i as f32 * 0.01).collect();
    let pre: Vec<f32> = (0..num_connections).map(|i| (i % 10) as f32 * 0.1).collect();
    let post: Vec<f32> = (0..num_connections).map(|i| ((i + 5) % 10) as f32 * 0.1).collect();

    // Baseline: Scalar (100 connections)
    group.bench_function("scalar", |b| {
        b.iter(|| {
            for i in 0..num_connections {
                weights[i] += learning_rate * pre[i] * post[i];
            }
            black_box(&weights)
        });
    });

    // SIMD: 100 connections in batches of 8
    group.bench_function("simd", |b| {
        // Pad to multiple of 8
        let mut weights_padded = weights.clone();
        weights_padded.resize(104, 0.0);
        let mut pre_padded = pre.clone();
        pre_padded.resize(104, 0.0);
        let mut post_padded = post.clone();
        post_padded.resize(104, 0.0);

        b.iter(|| {
            for i in (0..104).step_by(8) {
                let mut w_chunk = [0.0f32; 8];
                let mut p_chunk = [0.0f32; 8];
                let mut o_chunk = [0.0f32; 8];

                w_chunk.copy_from_slice(&weights_padded[i..i + 8]);
                p_chunk.copy_from_slice(&pre_padded[i..i + 8]);
                o_chunk.copy_from_slice(&post_padded[i..i + 8]);

                simd_hebbian_update(&mut w_chunk, &p_chunk, &o_chunk, learning_rate);

                weights_padded[i..i + 8].copy_from_slice(&w_chunk);
            }
            black_box(&weights_padded)
        });
    });

    group.finish();
}

/// B32 B3: Scaling analysis (vary connection count)
/// Validate SIMD threshold and speedup scaling
fn bench_scaling_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_analysis");

    group
        .confidence_level(0.95)
        .sample_size(500)
        .warm_up_time(Duration::from_secs(2));

    let learning_rate = 0.01;

    for num_connections in [6, 12, 24, 48, 96, 192] {
        let mut weights: Vec<f32> = (0..num_connections).map(|i| i as f32 * 0.01).collect();
        let pre: Vec<f32> = (0..num_connections).map(|i| (i % 10) as f32 * 0.1).collect();
        let post: Vec<f32> = (0..num_connections).map(|i| ((i + 5) % 10) as f32 * 0.1).collect();

        // Scalar baseline
        group.bench_with_input(
            BenchmarkId::new("scalar", num_connections),
            &num_connections,
            |b, &n| {
                b.iter(|| {
                    for i in 0..n {
                        weights[i] += learning_rate * pre[i] * post[i];
                    }
                    black_box(&weights)
                });
            },
        );

        // SIMD (f32x8)
        group.bench_with_input(
            BenchmarkId::new("simd", num_connections),
            &num_connections,
            |b, &n| {
                // Pad to multiple of 8
                let padded = ((n + 7) / 8) * 8;
                let mut weights_padded = weights.clone();
                weights_padded.resize(padded, 0.0);
                let mut pre_padded = pre.clone();
                pre_padded.resize(padded, 0.0);
                let mut post_padded = post.clone();
                post_padded.resize(padded, 0.0);

                b.iter(|| {
                    for i in (0..padded).step_by(8) {
                        let mut w_chunk = [0.0f32; 8];
                        let mut p_chunk = [0.0f32; 8];
                        let mut o_chunk = [0.0f32; 8];

                        w_chunk.copy_from_slice(&weights_padded[i..i + 8]);
                        p_chunk.copy_from_slice(&pre_padded[i..i + 8]);
                        o_chunk.copy_from_slice(&post_padded[i..i + 8]);

                        simd_hebbian_update(&mut w_chunk, &p_chunk, &o_chunk, learning_rate);

                        weights_padded[i..i + 8].copy_from_slice(&w_chunk);
                    }
                    black_box(&weights_padded)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_connection_update,
    bench_6_connection_batch,
    bench_neuron_update_100_connections,
    bench_scaling_analysis,
);
criterion_main!(benches);
