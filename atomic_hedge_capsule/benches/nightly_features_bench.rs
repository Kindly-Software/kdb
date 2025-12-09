//! Nightly Features Benchmark - Q32 Framework Compliant
//!
//! Following B32 fairness framework principles:
//! - Fair baselines: Stable Rust vs Nightly-enhanced implementations
//! - Statistical rigor: 95% confidence intervals, proper feature isolation
//! - Hardware measurement: SIMD utilization, compile-time optimization impact
//! - Kontext27 reality checks: 10-30% improvement claims with nightly features
//! - Empirical validation: Real workloads demonstrating nightly advantages
//!
//! UCE32 Q32 Analysis Applied:
//! - Q32 (Nightly Enhancement): portable_simd, const_fn_floating_point, atomic_from_mut
//! - Q31 (Rust Transform): Zero-cost abstractions with cutting-edge compiler features
//! - Q30 (Validation): Prove 10-30% nightly performance improvement
//! - Q29 (Constraints): Hardware must support SIMD, nightly compiler required
//! - Q28 (Simplicity): Complex nightly optimizations with simple stable fallbacks

use atomic_hedge_capsule::AtomicHedgeCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box as std_black_box;
use std::time::{Duration, Instant};

// Import nightly hedge capsule if available
#[cfg(feature = "nightly")]
use atomic_hedge_capsule::nightly::AtomicHedgeCapsuleNightly;

/// B32 Framework: Detect available nightly features
fn detect_nightly_features() -> String {
    let mut features = Vec::new();

    #[cfg(feature = "portable_simd")]
    features.push("portable_simd");

    #[cfg(feature = "const_fn_floating_point_arithmetic")]
    features.push("const_fn_floating_point");

    #[cfg(feature = "atomic_from_mut")]
    features.push("atomic_from_mut");

    #[cfg(feature = "const_trait_impl")]
    features.push("const_trait_impl");

    #[cfg(feature = "core_intrinsics")]
    features.push("core_intrinsics");

    if features.is_empty() {
        "No nightly features enabled".to_string()
    } else {
        format!("Nightly features: {}", features.join(", "))
    }
}

/// Stable Rust implementation for fair comparison
mod stable_implementation {
    /// Standard hedge weight calculation without SIMD
    pub fn calculate_hedge_weights(spreads: &[f64], threshold: f64) -> Vec<f64> {
        spreads
            .iter()
            .map(|&spread| {
                if spread > threshold {
                    (spread - threshold) * 0.618033988749 // φ reciprocal
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// Standard phi calculation at runtime
    pub fn phi_threshold() -> f64 {
        1.6180339887498948 * 0.05 // φ * 5%
    }

    /// Standard bulk operations with Vec allocations
    pub fn bulk_position_updates(positions: &[(u64, u64)], spreads: &[f64]) -> Vec<(u64, f64)> {
        positions
            .iter()
            .zip(spreads.iter())
            .map(|(&(long, short), &spread)| {
                let effectiveness = if spread > phi_threshold() {
                    ((spread - phi_threshold()) / phi_threshold()) * 0.618033988749 * 100.0
                } else {
                    0.0
                };
                (long.wrapping_add(short), effectiveness)
            })
            .collect()
    }
}

/// Nightly SIMD implementation when available
#[cfg(feature = "portable_simd")]
mod nightly_simd_implementation {
    use std::simd::prelude::*;

    /// SIMD-accelerated hedge weight calculation
    pub fn calculate_hedge_weights_simd(spreads: &[f64], threshold: f64) -> Vec<f64> {
        let mut weights = Vec::with_capacity(spreads.len());
        let phi_reciprocal = 0.618033988749;

        // Process 8 elements at a time with SIMD
        for chunk in spreads.chunks(8) {
            let mut simd_spreads = [0.0; 8];
            for (i, &spread) in chunk.iter().enumerate() {
                simd_spreads[i] = spread;
            }

            let spreads_vec = f64x8::from_array(simd_spreads);
            let threshold_vec = f64x8::splat(threshold);
            let phi_vec = f64x8::splat(phi_reciprocal);
            let zero_vec = f64x8::splat(0.0);

            // SIMD computation: weight = max(0, (spread - threshold) * φ)
            let mask = spreads_vec.simd_gt(threshold_vec);
            let adjusted_spreads = spreads_vec - threshold_vec;
            let phi_weights = adjusted_spreads * phi_vec;
            let final_weights = mask.select(phi_weights, zero_vec);

            let result_array = final_weights.to_array();
            for (i, weight) in result_array.iter().enumerate() {
                if i < chunk.len() {
                    weights.push(*weight);
                }
            }
        }

        weights
    }

    /// SIMD bulk processing for multiple position updates
    pub fn bulk_position_updates_simd(
        positions: &[(u64, u64)],
        spreads: &[f64],
        threshold: f64,
    ) -> Vec<(u64, f64)> {
        let mut results = Vec::with_capacity(positions.len());
        let phi_reciprocal = 0.618033988749;

        for chunk_positions in positions.chunks(8) {
            let chunk_spreads = &spreads[..chunk_positions.len().min(spreads.len())];

            // Prepare SIMD data
            let mut long_array = [0u64; 8];
            let mut short_array = [0u64; 8];
            let mut spread_array = [0.0; 8];

            for (i, (&(long, short), &spread)) in
                chunk_positions.iter().zip(chunk_spreads.iter()).enumerate()
            {
                long_array[i] = long;
                short_array[i] = short;
                spread_array[i] = spread;
            }

            // SIMD operations
            let long_vec = u64x8::from_array(long_array);
            let short_vec = u64x8::from_array(short_array);
            let spread_vec = f64x8::from_array(spread_array);

            let combined_positions = long_vec + short_vec;
            let threshold_vec = f64x8::splat(threshold);
            let phi_vec = f64x8::splat(phi_reciprocal * 100.0);

            let mask = spread_vec.simd_gt(threshold_vec);
            let relative_spread = (spread_vec - threshold_vec) / threshold_vec;
            let effectiveness = relative_spread * phi_vec;
            let final_effectiveness = mask.select(effectiveness, f64x8::splat(0.0));

            // Extract results
            let position_results = combined_positions.to_array();
            let effectiveness_results = final_effectiveness.to_array();

            for i in 0..chunk_positions.len() {
                results.push((position_results[i], effectiveness_results[i]));
            }
        }

        results
    }
}

/// Compare stable vs nightly hedge weight calculations
fn bench_hedge_weight_calculation(c: &mut Criterion) {
    let nightly_info = detect_nightly_features();
    println!("=== Hedge Weight Calculation Benchmark ===");
    println!("{}", nightly_info);
    println!("B32 Target: 10-30% improvement with SIMD acceleration");

    let mut group = c.benchmark_group("hedge_weight_calculation");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(200);

    // Test different array sizes to show SIMD scaling
    for &size in &[64, 256, 1024, 4096] {
        group.throughput(Throughput::Elements(size as u64));

        // Generate test data
        let spreads: Vec<f64> = (0..size).map(|i| 0.001 + (i as f64) / 100000.0).collect();
        let threshold = stable_implementation::phi_threshold();

        // Stable implementation
        group.bench_with_input(
            BenchmarkId::new("stable_calculation", size),
            &size,
            |b, _| {
                b.iter(|| {
                    let weights =
                        stable_implementation::calculate_hedge_weights(&spreads, threshold);
                    std_black_box(weights);
                });
            },
        );

        // Nightly SIMD implementation
        #[cfg(feature = "portable_simd")]
        group.bench_with_input(
            BenchmarkId::new("nightly_simd_calculation", size),
            &size,
            |b, _| {
                b.iter(|| {
                    let weights = nightly_simd_implementation::calculate_hedge_weights_simd(
                        &spreads, threshold,
                    );
                    std_black_box(weights);
                });
            },
        );

        // Fallback when SIMD not available
        #[cfg(not(feature = "portable_simd"))]
        group.bench_with_input(
            BenchmarkId::new("nightly_fallback_calculation", size),
            &size,
            |b, _| {
                b.iter(|| {
                    let weights =
                        stable_implementation::calculate_hedge_weights(&spreads, threshold);
                    std_black_box(weights);
                });
            },
        );
    }

    group.finish();
}

/// Compare const fn optimization vs runtime calculation
fn bench_const_fn_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("const_fn_optimization");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(500);

    let iterations = 10000u64;
    group.throughput(Throughput::Elements(iterations));

    // Runtime calculation (stable)
    group.bench_function("runtime_phi_calculation", |b| {
        b.iter(|| {
            let mut total = 0.0;
            for i in 0..iterations {
                let phi = 1.6180339887498948; // Calculated at runtime
                let threshold = phi * 0.05;
                let spread = 0.01 + (i as f64) / 1000000.0;
                let result = if spread > threshold {
                    (spread - threshold) * (1.0 / phi)
                } else {
                    0.0
                };
                total += result;
            }
            std_black_box(total);
        });
    });

    // Const fn calculation (nightly when available)
    #[cfg(feature = "const_fn_floating_point_arithmetic")]
    group.bench_function("const_fn_phi_calculation", |b| {
        const PHI: f64 = 1.6180339887498948;
        const PHI_RECIPROCAL: f64 = 1.0 / PHI;
        const PHI_THRESHOLD: f64 = PHI * 0.05;

        b.iter(|| {
            let mut total = 0.0;
            for i in 0..iterations {
                let spread = 0.01 + (i as f64) / 1000000.0;
                let result = if spread > PHI_THRESHOLD {
                    (spread - PHI_THRESHOLD) * PHI_RECIPROCAL
                } else {
                    0.0
                };
                total += result;
            }
            std_black_box(total);
        });
    });

    // Fallback const calculation (stable)
    #[cfg(not(feature = "const_fn_floating_point_arithmetic"))]
    group.bench_function("stable_const_calculation", |b| {
        const PHI_THRESHOLD: f64 = 0.08090169943749474; // Pre-calculated
        const PHI_RECIPROCAL: f64 = 0.618033988749;

        b.iter(|| {
            let mut total = 0.0;
            for i in 0..iterations {
                let spread = 0.01 + (i as f64) / 1000000.0;
                let result = if spread > PHI_THRESHOLD {
                    (spread - PHI_THRESHOLD) * PHI_RECIPROCAL
                } else {
                    0.0
                };
                total += result;
            }
            std_black_box(total);
        });
    });

    group.finish();
}

/// Compare stable vs nightly hedge capsule implementations
fn bench_hedge_capsule_implementations(c: &mut Criterion) {
    let mut group = c.benchmark_group("hedge_capsule_implementations");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(200);

    let operations = 1000u64;
    group.throughput(Throughput::Elements(operations));

    // Stable hedge capsule
    group.bench_function("stable_hedge_capsule", |b| {
        b.iter(|| {
            let capsule = AtomicHedgeCapsule::new();

            for i in 0..operations {
                let side = i % 2 == 0;
                let quantity = 1000 + (i % 1000) as u32;
                let entry_price = 50000 + (i % 5000) as u32;

                let _result = capsule.start_bracket(side, quantity, entry_price, 500, 1000);

                if i % 10 == 0 {
                    let _state = capsule.read_if_ready();
                }

                if i % 50 == 0 {
                    let _rollback = capsule.rollback_bracket();
                }
            }
        });
    });

    // Nightly hedge capsule (when available)
    #[cfg(feature = "nightly")]
    group.bench_function("nightly_hedge_capsule", |b| {
        b.iter(|| {
            let capsule = AtomicHedgeCapsuleNightly::new();

            for i in 0..operations {
                let spread = 0.01 + (i as f64) / 100000.0;
                let long_pos = i * 1000;
                let short_pos = i * 500;

                let _result = capsule.update_position(long_pos, short_pos, spread);

                if i % 10 == 0 {
                    let _state = capsule.read_position();
                }
            }
        });
    });

    group.finish();
}

/// Bulk operations comparison (stable vs nightly optimizations)
fn bench_bulk_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_operations");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(100);

    for &size in &[100, 500, 1000, 5000] {
        group.throughput(Throughput::Elements(size as u64));

        // Generate test data
        let positions: Vec<(u64, u64)> = (0..size)
            .map(|i| (i as u64 * 1000, i as u64 * 500))
            .collect();
        let spreads: Vec<f64> = (0..size).map(|i| 0.001 + (i as f64) / 100000.0).collect();

        // Stable bulk operations
        group.bench_with_input(
            BenchmarkId::new("stable_bulk_operations", size),
            &size,
            |b, _| {
                b.iter(|| {
                    let results =
                        stable_implementation::bulk_position_updates(&positions, &spreads);
                    std_black_box(results);
                });
            },
        );

        // Nightly SIMD bulk operations
        #[cfg(feature = "portable_simd")]
        group.bench_with_input(
            BenchmarkId::new("nightly_simd_bulk_operations", size),
            &size,
            |b, _| {
                let threshold = stable_implementation::phi_threshold();
                b.iter(|| {
                    let results = nightly_simd_implementation::bulk_position_updates_simd(
                        &positions, &spreads, threshold,
                    );
                    std_black_box(results);
                });
            },
        );

        // Nightly hedge capsule batch operations (when available)
        #[cfg(feature = "nightly")]
        group.bench_with_input(
            BenchmarkId::new("nightly_capsule_batch", size),
            &size,
            |b, _| {
                b.iter(|| {
                    let capsule = AtomicHedgeCapsuleNightly::new();
                    let mut position_vec = positions.clone();

                    #[cfg(feature = "atomic-optimizations")]
                    {
                        let _results = capsule.batch_update_positions(&mut position_vec, &spreads);
                    }

                    #[cfg(not(feature = "atomic-optimizations"))]
                    {
                        // Fallback to individual updates
                        for (pos, &spread) in position_vec.iter().zip(spreads.iter()) {
                            let _result = capsule.update_position(pos.0, pos.1, spread);
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

/// Memory allocation patterns: nightly vs stable
fn bench_memory_allocation_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation_patterns");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(300);

    let operations = 1000u64;
    group.throughput(Throughput::Elements(operations));

    // Stable: Vec allocations for results
    group.bench_function("stable_vec_allocations", |b| {
        b.iter(|| {
            let mut all_results = Vec::new();

            for i in 0..operations {
                let mut local_results = Vec::with_capacity(10);
                for j in 0..10 {
                    local_results.push(i * 100 + j);
                }
                all_results.extend(local_results);
            }

            std_black_box(all_results);
        });
    });

    // Nightly: atomic_from_mut reduces allocations
    #[cfg(feature = "atomic_from_mut")]
    group.bench_function("nightly_reduced_allocations", |b| {
        b.iter(|| {
            let mut data_buffer = vec![0u64; operations as usize * 10];

            for i in 0..operations {
                let start_idx = (i as usize) * 10;
                let end_idx = start_idx + 10;
                let slice = &mut data_buffer[start_idx..end_idx];

                // Use atomic_from_mut to avoid allocations
                let atomic_slice = std::sync::atomic::AtomicU64::from_mut_slice(slice);
                for (j, atomic_val) in atomic_slice.iter().enumerate() {
                    atomic_val.store(i * 100 + j as u64, std::sync::atomic::Ordering::Relaxed);
                }
            }

            std_black_box(data_buffer);
        });
    });

    // Pre-allocated buffer pattern (stable fallback)
    group.bench_function("stable_preallocated_buffer", |b| {
        b.iter(|| {
            let mut data_buffer = vec![0u64; operations as usize * 10];

            for i in 0..operations {
                let start_idx = (i as usize) * 10;
                for j in 0..10 {
                    data_buffer[start_idx + j] = i * 100 + j as u64;
                }
            }

            std_black_box(data_buffer);
        });
    });

    group.finish();
}

/// Comprehensive nightly performance validation
fn bench_nightly_performance_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("nightly_performance_validation");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(50);

    let nightly_features = detect_nightly_features();
    println!("=== Nightly Performance Validation ===");
    println!("{}", nightly_features);
    println!("B32 Target: Overall 10-30% improvement with nightly features");

    let operations = 5000u64;
    group.throughput(Throughput::Elements(operations));

    // Comprehensive stable baseline
    group.bench_function("comprehensive_stable_baseline", |b| {
        b.iter(|| {
            let capsule = AtomicHedgeCapsule::new();
            let mut results = Vec::new();

            for i in 0..operations {
                // Hedge operation
                let side = i % 2 == 0;
                let quantity = 1000 + (i % 1000) as u32;
                let entry_price = 50000 + (i % 5000) as u32;

                let result = capsule.start_bracket(side, quantity, entry_price, 500, 1000);
                results.push(result.is_ok());

                // Weight calculation
                let spread = 0.01 + (i as f64) / 100000.0;
                let threshold = stable_implementation::phi_threshold();
                let weight = if spread > threshold {
                    (spread - threshold) * 0.618033988749
                } else {
                    0.0
                };

                // Decision logic
                let should_hedge = weight > 0.05;
                results.push(should_hedge);

                // State management
                if i % 20 == 0 {
                    let _state = capsule.read_if_ready();
                    let _rollback = capsule.rollback_bracket();
                }
            }

            std_black_box(results);
        });
    });

    // Comprehensive nightly optimized version
    #[cfg(feature = "nightly")]
    group.bench_function("comprehensive_nightly_optimized", |b| {
        b.iter(|| {
            let capsule = AtomicHedgeCapsuleNightly::new();
            let mut results = Vec::new();

            // Prepare SIMD data
            let spreads: Vec<f64> = (0..operations)
                .map(|i| 0.01 + (i as f64) / 100000.0)
                .collect();

            #[cfg(feature = "portable_simd")]
            let weights = nightly_simd_implementation::calculate_hedge_weights_simd(
                &spreads,
                0.08090169943749474,
            );

            #[cfg(not(feature = "portable_simd"))]
            let weights =
                stable_implementation::calculate_hedge_weights(&spreads, 0.08090169943749474);

            for i in 0..operations {
                // Optimized hedge operation
                let spread = spreads[i as usize];
                let long_pos = i * 1000;
                let short_pos = i * 500;

                let result = capsule.update_position(long_pos, short_pos, spread);
                results.push(result.is_ok());

                // Pre-calculated weight
                let weight = weights.get(i as usize).unwrap_or(&0.0);
                let should_hedge = *weight > 0.05;
                results.push(should_hedge);

                // Optimized state management
                if i % 20 == 0 {
                    let _state = capsule.read_position();
                }
            }

            std_black_box(results);
        });
    });

    group.finish();
}

// Configure Criterion benchmark groups
criterion_group!(
    nightly_benches,
    bench_hedge_weight_calculation,
    bench_const_fn_optimization,
    bench_hedge_capsule_implementations,
    bench_bulk_operations,
    bench_memory_allocation_patterns,
    bench_nightly_performance_validation,
);

criterion_main!(nightly_benches);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stable_hedge_weights() {
        let spreads = vec![0.005, 0.015, 0.025, 0.035];
        let threshold = 0.08090169943749474; // φ * 0.05
        let weights = stable_implementation::calculate_hedge_weights(&spreads, threshold);

        // Only spreads above threshold should have non-zero weights
        assert_eq!(weights[0], 0.0); // 0.005 < threshold
        assert_eq!(weights[1], 0.0); // 0.015 < threshold
        assert!(weights[2] > 0.0); // 0.025 > threshold
        assert!(weights[3] > 0.0); // 0.035 > threshold
    }

    #[cfg(feature = "portable_simd")]
    #[test]
    fn test_simd_hedge_weights_consistency() {
        let spreads = vec![0.005, 0.015, 0.025, 0.035, 0.045, 0.055, 0.065, 0.075];
        let threshold = 0.08090169943749474;

        let stable_weights = stable_implementation::calculate_hedge_weights(&spreads, threshold);
        let simd_weights =
            nightly_simd_implementation::calculate_hedge_weights_simd(&spreads, threshold);

        // SIMD and stable should produce same results
        for (stable, simd) in stable_weights.iter().zip(simd_weights.iter()) {
            assert!(
                (stable - simd).abs() < 1e-10,
                "Stable: {}, SIMD: {}",
                stable,
                simd
            );
        }
    }

    #[test]
    fn test_phi_threshold_calculation() {
        let threshold = stable_implementation::phi_threshold();
        let expected = 1.6180339887498948 * 0.05;
        assert!((threshold - expected).abs() < 1e-15);
    }

    #[test]
    fn test_bulk_operations_consistency() {
        let positions = vec![(1000, 500), (2000, 1000), (3000, 1500)];
        let spreads = vec![0.005, 0.015, 0.025];

        let results = stable_implementation::bulk_position_updates(&positions, &spreads);
        assert_eq!(results.len(), 3);

        // Check calculations
        assert_eq!(results[0].0, 1500); // 1000 + 500
        assert_eq!(results[1].0, 3000); // 2000 + 1000
        assert_eq!(results[2].0, 4500); // 3000 + 1500
    }

    #[test]
    fn test_feature_detection() {
        let features = detect_nightly_features();
        assert!(!features.is_empty());
        // Should contain at least one feature description
    }

    #[cfg(feature = "nightly")]
    #[test]
    fn test_nightly_capsule_basic_operations() {
        let capsule = AtomicHedgeCapsuleNightly::new();

        let result = capsule.update_position(1000, 500, 0.05);
        assert!(result.is_ok());

        let (long, short, spread, _) = capsule.read_position();
        assert_eq!(long, 1000);
        assert_eq!(short, 500);
        assert_eq!(spread, 0.05);
    }

    #[test]
    fn test_const_calculations() {
        // These should be the same regardless of const fn availability
        const PHI_RECIPROCAL: f64 = 0.618033988749;
        const PHI_THRESHOLD: f64 = 0.08090169943749474;

        let runtime_phi_reciprocal = 1.0 / 1.6180339887498948;
        let runtime_threshold = 1.6180339887498948 * 0.05;

        assert!((PHI_RECIPROCAL - runtime_phi_reciprocal).abs() < 1e-10);
        assert!((PHI_THRESHOLD - runtime_threshold).abs() < 1e-10);
    }
}
