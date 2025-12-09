//! # TemporalRDOCapsule Comprehensive Tests (T28 Framework)
//!
//! **Test Coverage**: 28 tests across 4 tiers
//! - Tier 1 (Q1-Q7): Unit tests (basic functionality)
//! - Tier 2 (Q8-Q14): Property tests (invariants, edge cases)
//! - Tier 3 (Q15-Q21): Integration tests (multi-threading, composition)
//! - Tier 4 (Q22-Q28): Production tests (performance, stress, realistic workloads)

use atomic_capsule::encoder::{TemporalRDOCapsule, Candidate, MotionVector};
use std::sync::Arc;
use std::thread;

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7)
// ============================================================================

#[test]
fn q1_layout_alignment() {
    assert_eq!(core::mem::size_of::<TemporalRDOCapsule>(), 256);
    assert_eq!(core::mem::align_of::<TemporalRDOCapsule>(), 256);
}

#[test]
fn q2_basic_initialization() {
    let capsule = TemporalRDOCapsule::new(24);
    assert_eq!(capsule.get_qp(), 24);
    assert_eq!(capsule.get_generation(), 1);

    let lambda = capsule.get_lambda();
    assert!(lambda > 0.0);
}

#[test]
fn q3_lambda_formula_correctness() {
    let capsule = TemporalRDOCapsule::new(12);

    // QP=12: λ = 0.85 × 2^0 = 0.85
    let lambda = capsule.compute_lambda(12);
    assert!((lambda - 0.85).abs() < 0.01);

    // QP=24: λ = 0.85 × 2^4 = 13.6
    let lambda = capsule.compute_lambda(24);
    assert!((lambda - 13.6).abs() < 0.1);

    // QP=36: λ = 0.85 × 2^8 = 217.6
    let lambda = capsule.compute_lambda(36);
    assert!((lambda - 217.6).abs() < 1.0);
}

#[test]
fn q4_rd_cost_computation() {
    let capsule = TemporalRDOCapsule::new(24);

    // J = D + λR with λ ≈ 13.6
    let cost = capsule.compute_rd_cost(1000, 100);
    assert!(cost >= 2300 && cost <= 2400); // 1000 + 13.6*100 ≈ 2360
}

#[test]
fn q5_motion_vector_norms() {
    let mv = MotionVector::new(3, 4);
    assert_eq!(mv.l1_norm(), 7);
    assert_eq!(mv.l2_norm_squared(), 25); // 3^2 + 4^2

    let mv_neg = MotionVector::new(-5, 12);
    assert_eq!(mv_neg.l1_norm(), 17);
    assert_eq!(mv_neg.l2_norm_squared(), 169); // 5^2 + 12^2
}

#[test]
fn q6_satd_zero_residual() {
    let capsule = TemporalRDOCapsule::new(24);
    let residual = [0i16; 16];
    let satd = capsule.compute_satd(&residual);
    assert_eq!(satd, 0);
}

#[test]
fn q7_candidate_creation() {
    let c1 = Candidate::new(0, 1000, 100);
    assert_eq!(c1.mode, 0);
    assert_eq!(c1.distortion, 1000);
    assert_eq!(c1.rate, 100);
    assert!(c1.mv.is_none());

    let mv = MotionVector::new(2, -3);
    let c2 = Candidate::with_mv(5, 800, 120, mv);
    assert_eq!(c2.mode, 5);
    assert!(c2.mv.is_some());
    assert_eq!(c2.mv.unwrap(), mv);
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14)
// ============================================================================

#[test]
fn q8_lambda_monotonicity() {
    let capsule = TemporalRDOCapsule::new(12);

    // Lambda should increase monotonically with QP
    let mut prev_lambda = 0.0f32;
    for qp in 12..=51 {
        let lambda = capsule.compute_lambda(qp);
        assert!(lambda > prev_lambda);
        prev_lambda = lambda;
    }
}

#[test]
fn q9_rd_cost_monotonicity() {
    let capsule = TemporalRDOCapsule::new(24);

    // RD cost should increase with distortion
    let base_cost = capsule.compute_rd_cost(1000, 100);
    let higher_cost = capsule.compute_rd_cost(2000, 100);
    assert!(higher_cost > base_cost);

    // RD cost should increase with rate
    let base_cost = capsule.compute_rd_cost(1000, 100);
    let higher_cost = capsule.compute_rd_cost(1000, 200);
    assert!(higher_cost > base_cost);
}

#[test]
fn q10_satd_properties() {
    let capsule = TemporalRDOCapsule::new(24);

    // SATD should be zero for zero residual
    let zero = [0i16; 16];
    assert_eq!(capsule.compute_satd(&zero), 0);

    // SATD should be symmetric (flip sign)
    let residual = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let neg_residual = [-1, -2, -3, -4, -5, -6, -7, -8, -9, -10, -11, -12, -13, -14, -15, -16];
    let satd1 = capsule.compute_satd(&residual);
    let satd2 = capsule.compute_satd(&neg_residual);
    assert_eq!(satd1, satd2);
}

#[test]
fn q11_optimize_block_selects_best() {
    let capsule = TemporalRDOCapsule::new(24);

    let candidates = vec![
        Candidate::new(0, 2000, 100), // RD cost ≈ 2000 + 13.6*100 = 3360
        Candidate::new(1, 1000, 150), // RD cost ≈ 1000 + 13.6*150 = 3040
        Candidate::new(2, 1500, 120), // RD cost ≈ 1500 + 13.6*120 = 3132
    ];

    let best_idx = capsule.optimize_block(&candidates);
    assert_eq!(best_idx, 1); // Mode 1 has lowest RD cost
}

#[test]
fn q12_temporal_cost_increases_with_mv() {
    let capsule = TemporalRDOCapsule::new(24);

    let mv_small = MotionVector::new(1, 1);
    let mv_large = MotionVector::new(10, 10);

    let cost_small = capsule.add_temporal_cost(mv_small, 1000);
    let cost_large = capsule.add_temporal_cost(mv_large, 1000);

    assert!(cost_large > cost_small);
}

#[test]
fn q13_lambda_update_increments_generation() {
    let capsule = TemporalRDOCapsule::new(24);
    assert_eq!(capsule.get_generation(), 1);

    capsule.update_lambda(26);
    assert_eq!(capsule.get_generation(), 2);
    assert_eq!(capsule.get_qp(), 26);

    capsule.update_lambda(28);
    assert_eq!(capsule.get_generation(), 3);
}

#[test]
fn q14_cache_operations() {
    let capsule = TemporalRDOCapsule::new(24);

    // Optimize block caches distortion and rate
    let candidates = vec![Candidate::new(0, 1000, 100)];
    let best_idx = capsule.optimize_block(&candidates);

    let cached_distortion = capsule.get_distortion(best_idx);
    assert_eq!(cached_distortion, Some(1000));

    let cached_rate = capsule.get_rate(best_idx);
    assert_eq!(cached_rate, Some(100));
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21)
// ============================================================================

#[test]
fn q15_concurrent_lambda_updates() {
    let capsule = Arc::new(TemporalRDOCapsule::new(24));
    let mut handles = vec![];

    // 8 threads updating lambda concurrently
    for tid in 0..8 {
        let capsule = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let qp = 12 + ((tid * 100 + i) % 40) as u8;
                capsule.update_lambda(qp);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Generation should be 800 (8 threads × 100 updates)
    let final_gen = capsule.get_generation();
    assert_eq!(final_gen, 801); // 1 initial + 800 updates
}

#[test]
fn q16_concurrent_rd_cost_computations() {
    let capsule = Arc::new(TemporalRDOCapsule::new(24));
    let mut handles = vec![];

    for _ in 0..8 {
        let capsule = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for distortion in (1000..2000).step_by(100) {
                for rate in (50..150).step_by(10) {
                    let cost = capsule.compute_rd_cost(distortion as u32, rate as u32);
                    assert!(cost > 0);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn q17_concurrent_optimize_block() {
    let capsule = Arc::new(TemporalRDOCapsule::new(24));
    let mut handles = vec![];

    for _ in 0..4 {
        let capsule = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            let candidates = vec![
                Candidate::new(0, 2000, 100),
                Candidate::new(1, 1000, 150),
                Candidate::new(2, 1500, 120),
            ];

            for _ in 0..1000 {
                let best = capsule.optimize_block(&candidates);
                assert!(best < candidates.len());
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn q18_concurrent_satd_computation() {
    let capsule = Arc::new(TemporalRDOCapsule::new(24));
    let mut handles = vec![];

    for tid in 0..8 {
        let capsule = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            let residual: Vec<i16> = (0..16).map(|i| ((tid * 16 + i) % 256) as i16).collect();
            let mut residual_arr = [0i16; 16];
            residual_arr.copy_from_slice(&residual);

            for _ in 0..1000 {
                let satd = capsule.compute_satd(&residual_arr);
                assert!(satd < 100000); // Reasonable upper bound
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn q19_temporal_cost_integration() {
    let capsule = TemporalRDOCapsule::new(24);

    // Test with motion vectors
    let mv1 = MotionVector::new(2, 3);
    let mv2 = MotionVector::new(-5, 7);

    let cost1 = capsule.add_temporal_cost(mv1, 1000);
    let cost2 = capsule.add_temporal_cost(mv2, 1000);

    // Larger MV should have higher cost
    assert!(cost2 > cost1);
}

#[test]
fn q20_mixed_workload() {
    let capsule = Arc::new(TemporalRDOCapsule::new(24));
    let mut handles = vec![];

    // Thread 1: Lambda updates
    {
        let capsule = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for qp in 12..40 {
                capsule.update_lambda(qp);
                thread::sleep(std::time::Duration::from_micros(10));
            }
        });
        handles.push(handle);
    }

    // Thread 2-5: RD optimizations
    for _ in 0..4 {
        let capsule = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            let candidates = vec![
                Candidate::new(0, 2000, 100),
                Candidate::new(1, 1000, 150),
            ];

            for _ in 0..500 {
                capsule.optimize_block(&candidates);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn q21_reset_temporal_cost() {
    let capsule = TemporalRDOCapsule::new(24);

    capsule.reset_temporal_cost();
    assert_eq!(capsule.get_temporal_cost(), 0);

    // After some operations, reset should clear
    let _ = capsule.add_temporal_cost(MotionVector::new(5, 5), 1000);
    capsule.reset_temporal_cost();
    assert_eq!(capsule.get_temporal_cost(), 0);
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28)
// ============================================================================

#[test]
fn q22_performance_rd_cost() {
    let capsule = TemporalRDOCapsule::new(24);

    let start = std::time::Instant::now();
    for _ in 0..10000 {
        let _ = capsule.compute_rd_cost(1000, 100);
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / 10000;
    println!("Average RD cost computation: {} ns", avg_ns);
    assert!(avg_ns < 200); // <200ns per call
}

#[test]
fn q23_performance_optimize_block() {
    let capsule = TemporalRDOCapsule::new(24);

    let candidates = vec![
        Candidate::new(0, 2000, 100),
        Candidate::new(1, 1800, 110),
        Candidate::new(2, 1600, 120),
        Candidate::new(3, 1400, 130),
        Candidate::new(4, 1200, 140),
        Candidate::new(5, 1000, 150),
        Candidate::new(6, 1500, 125),
        Candidate::new(7, 1700, 115),
    ];

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = capsule.optimize_block(&candidates);
    }
    let elapsed = start.elapsed();

    let avg_us = elapsed.as_micros() / 1000;
    println!("Average optimize_block (8 candidates): {} μs", avg_us);
    assert!(avg_us < 2); // <2μs per block
}

#[test]
fn q24_performance_satd() {
    let capsule = TemporalRDOCapsule::new(24);
    let residual = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

    let start = std::time::Instant::now();
    for _ in 0..10000 {
        let _ = capsule.compute_satd(&residual);
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / 10000;
    println!("Average SATD computation: {} ns", avg_ns);
    assert!(avg_ns < 500); // <500ns per SATD
}

#[test]
fn q25_stress_concurrent_updates() {
    let capsule = Arc::new(TemporalRDOCapsule::new(24));
    let mut handles = vec![];

    // 16 threads, 10K operations each
    for tid in 0..16 {
        let capsule = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for i in 0..10000 {
                match i % 4 {
                    0 => {
                        capsule.update_lambda(12 + ((tid + i) % 40) as u8);
                    }
                    1 => {
                        let _ = capsule.compute_rd_cost((tid * 1000 + i) as u32, (tid * 100 + i) as u32);
                    }
                    2 => {
                        let residual: [i16; 16] = [tid as i16; 16];
                        let _ = capsule.compute_satd(&residual);
                    }
                    _ => {
                        let mv = MotionVector::new((tid % 10) as i16, (i % 10) as i16);
                        let _ = capsule.add_temporal_cost(mv, 1000);
                    }
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Final generation should be high (many lambda updates)
    let final_gen = capsule.get_generation();
    println!("Final generation after stress test: {}", final_gen);
    assert!(final_gen > 1);
}

#[test]
fn q26_realistic_hevc_workflow() {
    let capsule = TemporalRDOCapsule::new(26);

    // Simulate HEVC CTU (64×64) with 16×16 blocks
    let num_blocks = 16; // 4×4 grid of 16×16 blocks

    for block_idx in 0..num_blocks {
        // Generate 35 HEVC intra modes + 8 inter modes
        let mut candidates = Vec::new();

        // Intra modes (0-34)
        for mode in 0..35 {
            let distortion = 1000 + (mode * 50 % 500);
            let rate = 80 + (mode * 3 % 40);
            candidates.push(Candidate::new(mode as u8, distortion, rate));
        }

        // Inter modes (35-42) with motion vectors
        for mode in 35..43 {
            let mv_x = ((block_idx * 7 + mode) % 16) as i16 - 8;
            let mv_y = ((block_idx * 11 + mode) % 16) as i16 - 8;
            let mv = MotionVector::new(mv_x, mv_y);

            let distortion = 800 + (mode * 30 % 400);
            let rate = 100 + (mode * 5 % 50);
            candidates.push(Candidate::with_mv(mode as u8, distortion, rate, mv));
        }

        // Optimize block
        let best_idx = capsule.optimize_block(&candidates);
        assert!(best_idx < candidates.len());

        // Compute SATD for residual
        let residual: [i16; 16] = [(block_idx * 13 % 256) as i16; 16];
        let satd = capsule.compute_satd(&residual);
        assert!(satd < 100000);
    }
}

#[test]
fn q27_qp_range_validation() {
    // Test full QP range (0-51 for H.264/HEVC)
    for qp in 0..=51 {
        let capsule = TemporalRDOCapsule::new(qp);
        assert_eq!(capsule.get_qp(), qp);

        let lambda = capsule.get_lambda();
        assert!(lambda > 0.0);
        assert!(lambda < 1e6); // Reasonable upper bound
    }
}

#[test]
fn q28_satd_hadamard_properties() {
    let capsule = TemporalRDOCapsule::new(24);

    // Test Hadamard transform properties
    // 1. DC component (all same value) → SATD = 0 (after normalization)
    let dc_block = [100i16; 16];
    let satd_dc = capsule.compute_satd(&dc_block);
    assert!(satd_dc < 100); // Should be very small

    // 2. Checkerboard pattern (high frequency)
    let checkerboard = [
        100, -100, 100, -100,
        -100, 100, -100, 100,
        100, -100, 100, -100,
        -100, 100, -100, 100,
    ];
    let satd_high = capsule.compute_satd(&checkerboard);
    assert!(satd_high > satd_dc); // High frequency → higher SATD

    // 3. Linearity property: SATD(aX) ≈ |a| × SATD(X)
    let base = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let scaled: [i16; 16] = base.iter().map(|&x| x * 2).collect::<Vec<_>>().try_into().unwrap();

    let satd_base = capsule.compute_satd(&base);
    let satd_scaled = capsule.compute_satd(&scaled);

    let ratio = satd_scaled as f32 / satd_base as f32;
    assert!((ratio - 2.0).abs() < 0.5); // Approximately 2× (within tolerance)
}
