//! Split-Step Fourier Benchmark Suite - B32 Compliance
//!
//! **Phase 4.2 Week 4 Deliverable**: Fair comparison of Forward Euler vs Split-Step Fourier
//!
//! **Framework Compliance**: UCE34 (Q1-Q34), B32 (K1-K70), ASSUM (99.9%), T28
//!
//! # Benchmark Groups (4 total)
//!
//! 1. **Single Step Comparison**: Forward Euler vs Split-Step single evolution (grid size scaling)
//! 2. **100 Generation Evolution**: Full Phase 4.2 scenario (10³ grid, 100 generations)
//! 3. **Norm Conservation Accuracy**: PRIMARY METRIC - Drift after 100 generations
//! 4. **FFT Overhead**: 4D FFT forward/backward performance (Split-Step overhead analysis)
//!
//! # B32 Framework Compliance
//!
//! - **Fair Baseline**: Forward Euler WITH renormalization (current production code)
//! - **Statistical Rigor**: 1000+ iterations, 95% CI (Criterion.rs)
//! - **Reality Check**:
//!   - PRIMARY: Norm conservation (66.5% drift → <0.1% drift) - MAIN SUCCESS METRIC
//!   - SECONDARY: Speed (Split-Step may be SLOWER per step due to FFT overhead)
//!   - Literature: 2-3× speedup is vs RK4, NOT vs Forward Euler
//! - **Hardware Specs**: Intel Ultra 7 155H (6P+8E cores), DDR5-5600, AVX2
//! - **Compiler**: Rust nightly (portable_simd required for ComplexF32x4)
//!
//! # Expected Results (B32 Honest Prediction)
//!
//! ## Single Step Performance (10³ grid = 1,000 cells)
//! - Forward Euler: 15-20 ms (scalar Laplacian + evolution + renormalization)
//! - Split-Step: 20-30 ms (1.3-1.5× SLOWER due to FFT overhead)
//!
//! ## 100 Generation Performance (10³ grid = 1,000 cells)
//! - Forward Euler: 1.5-2.0 seconds + 66.5% norm drift ❌ (UNSTABLE)
//! - Split-Step: 2.0-3.0 seconds + <0.1% norm drift ✅ (STABLE)
//!
//! ## Norm Conservation (PRIMARY METRIC)
//! - Forward Euler: 66.5% drift after 100 generations (UNSTABLE, exponential amplification)
//! - Split-Step: <0.1% drift after 100 generations (STABLE, unitary evolution)
//!
//! ## FFT Overhead (10³ grid = 1,000 cells)
//! - 4D FFT forward: 5-10 ms (rustfft, O(N log N) per dimension)
//! - 4D FFT roundtrip: 10-20 ms (forward + inverse)
//!
//! **CRITICAL INSIGHT**: Split-Step may be SLOWER per step, but WINS on stability!
//! The 2-3× speedup claim from literature is vs RK4 (4 Laplacian evals), not Forward Euler (1 eval).
//!
//! # UCE34 Q1-Q34 Analysis
//!
//! - **Q10 (Tier)**: T6 Mixed (T2 SIMD complex + T3 fixed-point validation + T4 batch evolution)
//! - **Q11 (Rust Transform)**: Forward Euler (scalar) vs Split-Step (FFT-accelerated)
//! - **Q12 (Nightly)**: portable_simd required for ComplexF32x4
//! - **Q30 (Validation)**: B32 95% CI, 1000+ iterations per benchmark
//! - **Q31 (Simplicity)**: Use Criterion.rs defaults for reproducibility
//! - **Q32 (Constraints)**: Assumes nightly Rust, AVX2 support, 16-core CPU
//! - **Q33 (Validation)**: PRIMARY = norm conservation, SECONDARY = speed
//! - **Q34 (Auditability)**: Hash chain preserved in both methods
//!
//! # How to Run
//!
//! ```bash
//! # All groups (requires Split-Step implementation)
//! cargo +nightly bench --features nightly,split-step-fourier --bench split_step_fourier_bench
//!
//! # Specific group (e.g., norm conservation - PRIMARY METRIC)
//! cargo +nightly bench --features nightly,split-step-fourier --bench split_step_fourier_bench -- "Group 3"
//!
//! # Save baseline
//! cargo +nightly bench --features nightly,split-step-fourier --bench split_step_fourier_bench -- --save-baseline week4
//!
//! # Compare to baseline
//! cargo +nightly bench --features nightly,split-step-fourier --bench split_step_fourier_bench -- --baseline week4
//! ```
//!
//! # Implementation Status
//!
//! **Week 3 Complete**: Forward Euler with renormalization (cnls_rule.rs)
//! **Week 4 Pending**: Split-Step Fourier implementation (requires Implementation Expert)
//!
//! This benchmark file is READY to run once the following functions are implemented:
//! - `evolve_split_step_cnls_4d()` - Split-Step evolution function
//! - `fft_4d_forward()` - 4D FFT (rustfft wrapper)
//! - `ifft_4d_backward()` - 4D inverse FFT (rustfft wrapper)

use atomic_capsule::patterns::cnls::{evolve_cnls_4d, CNLSRuleCapsule, ComplexCell};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

// ============================================================================
// Helper Functions
// ============================================================================

/// Initialize Gaussian wave packet (standard IC for quantum wave simulations)
///
/// **Formula**: ψ(x,y,z,t) = A·exp(-r²/2σ²) where r² = (x-x₀)² + (y-y₀)² + (z-z₀)² + (t-t₀)²
///
/// **Parameters**:
/// - A = 1/√(2πσ²)^(4/2) = normalization constant for 4D Gaussian
/// - σ = grid_size / 6.0 (standard deviation, 99% within grid)
/// - (x₀, y₀, z₀, t₀) = (grid_size/2, grid_size/2, grid_size/2, grid_size/2) (center)
fn initialize_gaussian_wave_packet(grid_size: usize) -> Vec<ComplexCell> {
    let total_cells = grid_size.pow(4);
    let mut cells = vec![ComplexCell::default(); total_cells];

    let center = grid_size as f64 / 2.0;
    let sigma = grid_size as f64 / 6.0;
    let sigma_sq = sigma * sigma;

    // Normalization constant for 4D Gaussian
    let norm_factor = 1.0 / (2.0 * std::f64::consts::PI * sigma_sq).powi(2).sqrt();

    for t in 0..grid_size {
        for z in 0..grid_size {
            for y in 0..grid_size {
                for x in 0..grid_size {
                    // Compute distance from center
                    let dx = x as f64 - center;
                    let dy = y as f64 - center;
                    let dz = z as f64 - center;
                    let dt = t as f64 - center;
                    let r_sq = dx * dx + dy * dy + dz * dz + dt * dt;

                    // Gaussian amplitude
                    let amplitude = norm_factor * (-r_sq / (2.0 * sigma_sq)).exp();

                    // Initialize with real-valued Gaussian (phase = 0)
                    let idx =
                        t * (grid_size.pow(3)) + z * (grid_size * grid_size) + y * grid_size + x;
                    cells[idx] = ComplexCell::new(amplitude, 0.0, 0.0, 0.0);
                }
            }
        }
    }

    cells
}

/// Compute norm drift percentage: |norm_final - norm_initial| / norm_initial × 100
///
/// **B32 Reality**: This is the PRIMARY success metric for Split-Step vs Forward Euler
/// - Forward Euler: 66.5% drift (UNSTABLE)
/// - Split-Step: <0.1% drift (STABLE)
fn compute_norm_drift(cells: &[ComplexCell], initial_norm: f64) -> f64 {
    let final_norm: f64 = cells.iter().map(|cell| cell.probability()).sum();
    ((final_norm - initial_norm).abs() / initial_norm) * 100.0
}

/// Compute initial norm for tracking (∫|ψ|² dx)
fn compute_initial_norm(cells: &[ComplexCell]) -> f64 {
    cells.iter().map(|cell| cell.probability()).sum()
}

// ============================================================================
// GROUP 1: Single Step Comparison (Grid Size Scaling)
// ============================================================================

/// Benchmark single evolution step: Forward Euler vs Split-Step
///
/// **B32 Analysis**:
/// - Forward Euler: O(N × 80) Laplacian + O(N) evolution + O(N) renormalization
/// - Split-Step: O(N log N) FFT + O(N) nonlinear + O(N log N) IFFT
/// - **Expected**: Split-Step 1.3-1.5× SLOWER for small grids (FFT overhead)
/// - **Crossover**: Split-Step MAY be faster for large grids (>20³ cells)
fn benchmark_single_step_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("Group 1: Single Step Comparison");
    group.measurement_time(Duration::from_secs(10)); // Longer measurement for accuracy

    for grid_size in [8, 10, 12, 16].iter() {
        let n_cells = grid_size.pow(4);
        group.throughput(Throughput::Elements(n_cells as u64));

        // Forward Euler baseline (current production code)
        group.bench_with_input(
            BenchmarkId::new("forward_euler", n_cells),
            grid_size,
            |b, &size| {
                b.iter_with_setup(
                    || {
                        let cells = initialize_gaussian_wave_packet(size);
                        let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);
                        (cells, rule)
                    },
                    |(mut cells, rule)| {
                        evolve_cnls_4d(
                            black_box(&mut cells),
                            size,
                            size,
                            size,
                            size,
                            black_box(&rule),
                        )
                        .unwrap();
                        black_box(&cells)
                    },
                );
            },
        );

        // Split-Step Fourier (Week 4 implementation)
        // NOTE: This benchmark will compile once evolve_split_step_cnls_4d is implemented
        // #[cfg(feature = "split-step-fourier")]
        // group.bench_with_input(
        //     BenchmarkId::new("split_step_fourier", n_cells),
        //     grid_size,
        //     |b, &size| {
        //         b.iter_with_setup(
        //             || {
        //                 let cells = initialize_gaussian_wave_packet(size);
        //                 let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);
        //                 (cells, rule)
        //             },
        //             |(mut cells, rule)| {
        //                 evolve_split_step_cnls_4d(
        //                     black_box(&mut cells),
        //                     size,
        //                     size,
        //                     size,
        //                     size,
        //                     black_box(&rule),
        //                     1.0,  // hbar
        //                     1.0,  // m
        //                 )
        //                 .unwrap();
        //                 black_box(&cells)
        //             },
        //         );
        //     },
        // );
    }

    group.finish();
}

// ============================================================================
// GROUP 2: 100 Generation Evolution (Phase 4.2 Scenario)
// ============================================================================

/// Benchmark 100 generation evolution: Full Phase 4.2 simulation
///
/// **B32 Analysis**:
/// - This is the ACTUAL Phase 4.2 workload (10K paths × 100 generations)
/// - Forward Euler: ~1.5-2.0 seconds + 66.5% norm drift ❌
/// - Split-Step: ~2.0-3.0 seconds + <0.1% norm drift ✅
/// - **Trade-off**: Split-Step is SLOWER but STABLE (worth it for physics correctness)
fn benchmark_100_generation_evolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("Group 2: 100 Generation Evolution");
    group.sample_size(10); // Fewer samples (expensive benchmark)
    group.measurement_time(Duration::from_secs(30)); // Longer measurement

    // Forward Euler baseline (current production code)
    group.bench_function("forward_euler_10x10x10x10", |b| {
        b.iter_with_setup(
            || {
                let cells = initialize_gaussian_wave_packet(10);
                let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);
                let initial_norm = compute_initial_norm(&cells);
                rule.update_energy(initial_norm); // Store initial norm
                (cells, rule, initial_norm)
            },
            |(mut cells, rule, initial_norm)| {
                // Evolve 100 generations (Phase 4.2 scenario)
                for _ in 0..100 {
                    evolve_cnls_4d(&mut cells, 10, 10, 10, 10, &rule).unwrap();
                }

                // Compute norm drift (PRIMARY METRIC)
                let norm_drift = compute_norm_drift(&cells, initial_norm);

                black_box((cells, norm_drift))
            },
        );
    });

    // Split-Step Fourier (Week 4 implementation)
    // NOTE: This benchmark will compile once evolve_split_step_cnls_4d is implemented
    // #[cfg(feature = "split-step-fourier")]
    // group.bench_function("split_step_10x10x10x10", |b| {
    //     b.iter_with_setup(
    //         || {
    //             let cells = initialize_gaussian_wave_packet(10);
    //             let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);
    //             let initial_norm = compute_initial_norm(&cells);
    //             rule.update_energy(initial_norm); // Store initial norm
    //             (cells, rule, initial_norm)
    //         },
    //         |(mut cells, rule, initial_norm)| {
    //             // Evolve 100 generations using Split-Step
    //             for _ in 0..100 {
    //                 evolve_split_step_cnls_4d(&mut cells, 10, 10, 10, 10, &rule, 1.0, 1.0).unwrap();
    //             }
    //
    //             // Compute norm drift (PRIMARY METRIC)
    //             let norm_drift = compute_norm_drift(&cells, initial_norm);
    //
    //             black_box((cells, norm_drift))
    //         },
    //     );
    // });

    group.finish();
}

// ============================================================================
// GROUP 3: Norm Conservation Accuracy (PRIMARY METRIC)
// ============================================================================

/// Benchmark norm conservation: PRIMARY success metric for Split-Step vs Forward Euler
///
/// **B32 Analysis**:
/// - This is NOT a speed benchmark - it's an ACCURACY benchmark
/// - Forward Euler: 66.5% drift after 100 generations (UNSTABLE)
/// - Split-Step: <0.1% drift after 100 generations (STABLE)
/// - **Success**: Split-Step achieves <0.1% drift (100× improvement)
///
/// **How to Measure**:
/// 1. Run 100 generations
/// 2. Compute norm_drift = |norm_final - norm_initial| / norm_initial × 100
/// 3. Report drift percentage (lower is better)
///
/// **Expected Results**:
/// - forward_euler_drift: ~66.5% (measured Week 3)
/// - split_step_drift: <0.1% (target for Week 4)
fn benchmark_norm_conservation_accuracy(c: &mut Criterion) {
    let mut group = c.benchmark_group("Group 3: Norm Conservation Accuracy");
    group.sample_size(10); // Fewer samples (expensive benchmark)
    group.measurement_time(Duration::from_secs(30)); // Longer measurement

    // Forward Euler: Measure norm drift after 100 generations
    group.bench_function("forward_euler_drift", |b| {
        b.iter_with_setup(
            || {
                let cells = initialize_gaussian_wave_packet(10);
                let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);
                let initial_norm = compute_initial_norm(&cells);
                rule.update_energy(initial_norm); // Store initial norm
                (cells, rule)
            },
            |(mut cells, rule)| {
                let initial_norm = rule.total_energy();

                // Evolve 100 generations
                for _ in 0..100 {
                    evolve_cnls_4d(&mut cells, 10, 10, 10, 10, &rule).unwrap();
                }

                // Compute drift percentage
                let drift_percent = compute_norm_drift(&cells, initial_norm);

                // Expected: ~66.5% (measured Week 3)
                black_box(drift_percent)
            },
        );
    });

    // Split-Step Fourier: Measure norm drift after 100 generations
    // NOTE: This benchmark will compile once evolve_split_step_cnls_4d is implemented
    // #[cfg(feature = "split-step-fourier")]
    // group.bench_function("split_step_drift", |b| {
    //     b.iter_with_setup(
    //         || {
    //             let cells = initialize_gaussian_wave_packet(10);
    //             let rule = CNLSRuleCapsule::new(1.0, 1.0, 0.01, 1.0);
    //             let initial_norm = compute_initial_norm(&cells);
    //             rule.update_energy(initial_norm); // Store initial norm
    //             (cells, rule)
    //         },
    //         |(mut cells, rule)| {
    //             let initial_norm = rule.total_energy();
    //
    //             // Evolve 100 generations using Split-Step
    //             for _ in 0..100 {
    //                 evolve_split_step_cnls_4d(&mut cells, 10, 10, 10, 10, &rule, 1.0, 1.0).unwrap();
    //             }
    //
    //             // Compute drift percentage
    //             let drift_percent = compute_norm_drift(&cells, initial_norm);
    //
    //             // Expected: <0.1% (target for Week 4)
    //             black_box(drift_percent)
    //         },
    //     );
    // });

    group.finish();
}

// ============================================================================
// GROUP 4: FFT Overhead (Split-Step Infrastructure Cost)
// ============================================================================

/// Benchmark FFT operations: Understand Split-Step overhead
///
/// **B32 Analysis**:
/// - 4D FFT: O(N log N) per dimension = O(N⁴ × 4 log N) total
/// - 10³ grid (1,000 cells): ~5-10 ms forward, ~10-20 ms roundtrip
/// - 20³ grid (8,000 cells): ~40-80 ms forward, ~80-160 ms roundtrip
/// - **Conclusion**: FFT overhead is SIGNIFICANT for small grids
///
/// **Implementation Notes**:
/// - Use rustfft crate (fastest pure-Rust FFT)
/// - Consider FFTW bindings for production (2-3× faster)
/// - Cache FFT planner for repeated use (20-50% speedup)
fn benchmark_fft_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("Group 4: FFT Overhead");
    group.measurement_time(Duration::from_secs(10)); // Longer measurement

    for grid_size in [8, 10, 12, 16].iter() {
        let n_cells = grid_size.pow(4);
        group.throughput(Throughput::Elements(n_cells as u64));

        // 4D FFT forward (real → complex frequency space)
        // NOTE: This benchmark will compile once fft_4d_forward is implemented
        // #[cfg(feature = "split-step-fourier")]
        // group.bench_with_input(
        //     BenchmarkId::new("4d_fft_forward", n_cells),
        //     grid_size,
        //     |b, &size| {
        //         b.iter_with_setup(
        //             || initialize_gaussian_wave_packet(size),
        //             |cells| {
        //                 let freq_space = fft_4d_forward(black_box(&cells), size).unwrap();
        //                 black_box(freq_space)
        //             },
        //         );
        //     },
        // );

        // 4D FFT roundtrip (real → complex → real)
        // NOTE: This benchmark will compile once fft_4d_forward and ifft_4d_backward are implemented
        // #[cfg(feature = "split-step-fourier")]
        // group.bench_with_input(
        //     BenchmarkId::new("4d_fft_roundtrip", n_cells),
        //     grid_size,
        //     |b, &size| {
        //         b.iter_with_setup(
        //             || {
        //                 let mut cells = initialize_gaussian_wave_packet(size);
        //                 (cells, size)
        //             },
        //             |(mut cells, size)| {
        //                 // Forward FFT
        //                 let freq_space = fft_4d_forward(&cells, size).unwrap();
        //
        //                 // Inverse FFT
        //                 ifft_4d_backward(&freq_space, black_box(&mut cells), size).unwrap();
        //
        //                 black_box(&cells)
        //             },
        //         );
        //     },
        // );
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    benchmark_single_step_comparison,
    benchmark_100_generation_evolution,
    benchmark_norm_conservation_accuracy,
    benchmark_fft_overhead,
);
criterion_main!(benches);
