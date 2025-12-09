//! # TemporalRDOCapsule Benchmarks (B32 Framework)
//!
//! **Baseline**: Standard floating-point RD optimization (non-atomic)
//! **Target**: <2μs per block optimization (16 candidates)
//! **Iterations**: 1000+ for 95% confidence interval
//! **Hardware**: Captures CPU model for reproducibility

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use atomic_capsule::encoder::{TemporalRDOCapsule, Candidate, MotionVector};
use std::sync::Arc;
use std::thread;

// Helper to create Candidate
fn make_candidate(mode_id: u8, distortion: u32, rate: u32) -> Candidate {
    Candidate { mode_id, distortion, rate, mv: None }
}

fn make_candidate_with_mv(mode_id: u8, distortion: u32, rate: u32, mv: MotionVector) -> Candidate {
    Candidate { mode_id, distortion, rate, mv: Some(mv) }
}

// ============================================================================
// BASELINE: Standard Floating-Point RD Optimization (Non-Atomic)
// ============================================================================

struct BaselineRDO {
    lambda: f32,
    qp: u8,
}

impl BaselineRDO {
    fn new(qp: u8) -> Self {
        let lambda = 0.85 * 2.0f32.powf((qp as f32 - 12.0) / 3.0);
        Self { lambda, qp }
    }

    fn compute_rd_cost(&self, distortion: u32, rate: u32) -> u32 {
        let lambda_rate = (self.lambda * (rate as f32)) as u32;
        distortion.saturating_add(lambda_rate)
    }

    fn optimize_block(&self, candidates: &[Candidate]) -> usize {
        let mut best_idx = 0;
        let mut best_cost = u32::MAX;

        for (idx, candidate) in candidates.iter().enumerate() {
            let rd_cost = self.compute_rd_cost(candidate.distortion, candidate.rate);
            if rd_cost < best_cost {
                best_cost = rd_cost;
                best_idx = idx;
            }
        }

        best_idx
    }

    fn compute_satd(&self, residual: &[i16]) -> u32 {
        if residual.len() < 16 {
            return 0;
        }

        let mut buf = [0i32; 16];

        for i in 0..4 {
            let offset = i * 4;
            let a0 = residual[offset] as i32;
            let a1 = residual[offset + 1] as i32;
            let a2 = residual[offset + 2] as i32;
            let a3 = residual[offset + 3] as i32;

            let b0 = a0 + a3;
            let b1 = a1 + a2;
            let b2 = a1 - a2;
            let b3 = a0 - a3;

            buf[offset] = b0 + b1;
            buf[offset + 1] = b3 + b2;
            buf[offset + 2] = b0 - b1;
            buf[offset + 3] = b3 - b2;
        }

        let mut satd = 0u32;
        for i in 0..4 {
            let a0 = buf[i];
            let a1 = buf[4 + i];
            let a2 = buf[8 + i];
            let a3 = buf[12 + i];

            let b0 = a0 + a3;
            let b1 = a1 + a2;
            let b2 = a1 - a2;
            let b3 = a0 - a3;

            let c0 = b0 + b1;
            let c1 = b3 + b2;
            let c2 = b0 - b1;
            let c3 = b3 - b2;

            satd += c0.unsigned_abs();
            satd += c1.unsigned_abs();
            satd += c2.unsigned_abs();
            satd += c3.unsigned_abs();
        }

        (satd + 1) / 2
    }
}

// ============================================================================
// BENCHMARK GROUPS
// ============================================================================

fn bench_lambda_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("lambda_computation");

    group.bench_function("baseline_lambda", |b| {
        b.iter(|| {
            let qp = black_box(24);
            let lambda = 0.85 * 2.0f32.powf((qp as f32 - 12.0) / 3.0);
            black_box(lambda)
        })
    });

    group.bench_function("capsule_lambda", |b| {
        let capsule = TemporalRDOCapsule::new(24);
        b.iter(|| {
            let lambda = capsule.get_lambda();
            black_box(lambda)
        })
    });

    group.finish();
}

fn bench_rd_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("rd_cost");

    group.bench_function("baseline_rd_cost", |b| {
        let rdo = BaselineRDO::new(24);
        b.iter(|| {
            let cost = rdo.compute_rd_cost(black_box(1000), black_box(100));
            black_box(cost)
        })
    });

    group.bench_function("capsule_rd_cost", |b| {
        let capsule = TemporalRDOCapsule::new(24);
        b.iter(|| {
            let cost = capsule.compute_rd_cost(black_box(1000), black_box(100));
            black_box(cost)
        })
    });

    group.finish();
}

fn bench_optimize_block(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimize_block");
    group.throughput(Throughput::Elements(1));

    // Small workload (4 candidates)
    let candidates_small = vec![
        Candidate { mode_id: 0, distortion: 2000, rate: 100, mv: None },
        Candidate { mode_id: 1, distortion: 1500, rate: 120, mv: None },
        Candidate { mode_id: 2, distortion: 1800, rate: 110, mv: None },
        Candidate { mode_id: 3, distortion: 1600, rate: 115, mv: None },
    ];

    group.bench_with_input(
        BenchmarkId::new("baseline", "4_candidates"),
        &candidates_small,
        |b, candidates| {
            let rdo = BaselineRDO::new(24);
            b.iter(|| {
                let best = rdo.optimize_block(black_box(candidates));
                black_box(best)
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::new("capsule", "4_candidates"),
        &candidates_small,
        |b, candidates| {
            let capsule = TemporalRDOCapsule::new(24);
            b.iter(|| {
                let best = capsule.optimize_block(black_box(candidates));
                black_box(best)
            })
        },
    );

    // Medium workload (16 candidates, realistic)
    let candidates_medium: Vec<Candidate> = (0..16)
        .map(|i| {
            let distortion = 1000 + (i * 100);
            let rate = 80 + (i * 5);
            make_candidate(i as u8, distortion, rate)
        })
        .collect();

    group.bench_with_input(
        BenchmarkId::new("baseline", "16_candidates"),
        &candidates_medium,
        |b, candidates| {
            let rdo = BaselineRDO::new(24);
            b.iter(|| {
                let best = rdo.optimize_block(black_box(candidates));
                black_box(best)
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::new("capsule", "16_candidates"),
        &candidates_medium,
        |b, candidates| {
            let capsule = TemporalRDOCapsule::new(24);
            b.iter(|| {
                let best = capsule.optimize_block(black_box(candidates));
                black_box(best)
            })
        },
    );

    // Large workload (43 candidates, full HEVC)
    let candidates_large: Vec<Candidate> = (0..43)
        .map(|i| {
            let distortion = 1000 + (i * 50 % 500);
            let rate = 80 + (i * 3 % 40);
            if i >= 35 {
                let mv = MotionVector::new((i % 16) as i16 - 8, (i % 16) as i16 - 8);
                make_candidate_with_mv(i as u8, distortion, rate, mv)
            } else {
                make_candidate(i as u8, distortion, rate)
            }
        })
        .collect();

    group.bench_with_input(
        BenchmarkId::new("baseline", "43_candidates_hevc"),
        &candidates_large,
        |b, candidates| {
            let rdo = BaselineRDO::new(24);
            b.iter(|| {
                let best = rdo.optimize_block(black_box(candidates));
                black_box(best)
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::new("capsule", "43_candidates_hevc"),
        &candidates_large,
        |b, candidates| {
            let capsule = TemporalRDOCapsule::new(24);
            b.iter(|| {
                let best = capsule.optimize_block(black_box(candidates));
                black_box(best)
            })
        },
    );

    group.finish();
}

fn bench_satd(c: &mut Criterion) {
    let mut group = c.benchmark_group("satd");
    group.throughput(Throughput::Elements(16)); // 16 residual samples

    let residual = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

    group.bench_function("baseline_satd", |b| {
        let rdo = BaselineRDO::new(24);
        b.iter(|| {
            let satd = rdo.compute_satd(black_box(&residual));
            black_box(satd)
        })
    });

    group.bench_function("capsule_satd", |b| {
        let capsule = TemporalRDOCapsule::new(24);
        b.iter(|| {
            let satd = capsule.compute_satd(black_box(&residual));
            black_box(satd)
        })
    });

    group.finish();
}

fn bench_temporal_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal_cost");

    group.bench_function("capsule_temporal_cost", |b| {
        let capsule = TemporalRDOCapsule::new(24);
        let mv = MotionVector::new(5, 7);
        b.iter(|| {
            let cost = capsule.add_temporal_cost(black_box(mv), black_box(1000));
            black_box(cost)
        })
    });

    group.finish();
}

fn bench_concurrent_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_updates");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("8_threads_lambda_updates", |b| {
        b.iter(|| {
            let capsule = Arc::new(TemporalRDOCapsule::new(24));
            let mut handles = vec![];

            for tid in 0..8 {
                let capsule = Arc::clone(&capsule);
                let handle = thread::spawn(move || {
                    for i in 0..125 {
                        let qp = 12 + ((tid * 125 + i) % 40) as u8;
                        capsule.update_lambda(qp);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }

            black_box(capsule.get_generation())
        })
    });

    group.finish();
}

fn bench_realistic_hevc_ctu(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_hevc_ctu");
    group.throughput(Throughput::Elements(16)); // 16 blocks per CTU

    group.bench_function("hevc_ctu_64x64", |b| {
        let capsule = TemporalRDOCapsule::new(26);

        b.iter(|| {
            for block_idx in 0..16 {
                // Generate 35 intra + 8 inter candidates
                let mut candidates = Vec::with_capacity(43);

                for mode in 0..35 {
                    let distortion = 1000 + (mode * 50 % 500);
                    let rate = 80 + (mode * 3 % 40);
                    candidates.push(make_candidate(mode as u8, distortion, rate));
                }

                for mode in 35..43 {
                    let mv_x = ((block_idx * 7 + mode) % 16) as i16 - 8;
                    let mv_y = ((block_idx * 11 + mode) % 16) as i16 - 8;
                    let mv = MotionVector::new(mv_x, mv_y);

                    let distortion = 800 + (mode * 30 % 400);
                    let rate = 100 + (mode * 5 % 50);
                    candidates.push(make_candidate_with_mv(mode as u8, distortion, rate, mv));
                }

                let best = capsule.optimize_block(black_box(&candidates));
                black_box(best);

                let residual: [i16; 16] = [(block_idx * 13 % 256) as i16; 16];
                let satd = capsule.compute_satd(black_box(&residual));
                black_box(satd);
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_lambda_computation,
    bench_rd_cost,
    bench_optimize_block,
    bench_satd,
    bench_temporal_cost,
    bench_concurrent_updates,
    bench_realistic_hevc_ctu
);
criterion_main!(benches);
