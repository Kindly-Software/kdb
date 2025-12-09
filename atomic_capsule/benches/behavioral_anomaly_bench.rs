// BehavioralAnomalyCapsule - B32 Honest Benchmarking
// Framework: UCE34 (T6 Mixed: T3 Fixed-Point + T1 Atomic)
// Validation: Fair baselines, 95% CI, 1000+ iterations

use atomic_capsule::capsules::security::{
    AnomalyDecision as Decision, AnomalyType, BehavioralAnomalyCapsule, ModelId,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;

// ============================================================================
// BASELINE IMPLEMENTATIONS (Fair Comparison)
// ============================================================================

/// Baseline: Mutex-based scoring (traditional approach)
mod baseline {
    use std::sync::Mutex;

    pub struct MutexBasedAnomalyDetector {
        scores: Mutex<[f64; 5]>,
        weights: [f64; 5],
        threshold: Mutex<f64>,
        detections: Mutex<u32>,
        false_positives: Mutex<u32>,
    }

    impl MutexBasedAnomalyDetector {
        pub fn new() -> Self {
            Self {
                scores: Mutex::new([0.0; 5]),
                weights: [0.2, 0.2, 0.2, 0.2, 0.2],
                threshold: Mutex::new(0.85),
                detections: Mutex::new(0),
                false_positives: Mutex::new(0),
            }
        }

        pub fn update_score(&self, model_id: usize, score: f64) {
            let mut scores = self.scores.lock().unwrap();
            scores[model_id] = score.clamp(0.0, 1.0);
        }

        pub fn ensemble_vote(&self) -> bool {
            let scores = self.scores.lock().unwrap();
            let threshold = self.threshold.lock().unwrap();

            let weighted_sum: f64 = scores
                .iter()
                .zip(self.weights.iter())
                .map(|(score, weight)| score * weight)
                .sum();

            weighted_sum >= *threshold
        }

        pub fn record_detection(&self) {
            let mut detections = self.detections.lock().unwrap();
            *detections += 1;
        }

        pub fn record_false_positive(&self) {
            let mut false_positives = self.false_positives.lock().unwrap();
            *false_positives += 1;
        }
    }
}

// ============================================================================
// MICRO-BENCHMARKS (Individual Operations)
// ============================================================================

fn bench_score_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("score_update");

    // Capsule (T3 Fixed-Point)
    group.bench_function("capsule_t3", |b| {
        let capsule = BehavioralAnomalyCapsule::new();
        b.iter(|| {
            capsule.update_score(black_box(ModelId::RandomForest), black_box(0.75));
        });
    });

    // Baseline (Mutex)
    group.bench_function("baseline_mutex", |b| {
        let detector = baseline::MutexBasedAnomalyDetector::new();
        b.iter(|| {
            detector.update_score(black_box(0), black_box(0.75));
        });
    });

    group.finish();
}

fn bench_ensemble_vote(c: &mut Criterion) {
    let mut group = c.benchmark_group("ensemble_vote");

    // Capsule (T3 Fixed-Point + T1 Atomic)
    group.bench_function("capsule_t3_t1", |b| {
        let capsule = BehavioralAnomalyCapsule::new();

        // Pre-populate scores
        for model in ModelId::all() {
            capsule.update_score(model, 0.75);
        }

        b.iter(|| {
            let _ = capsule.ensemble_vote(black_box(AnomalyType::AccessPattern));
        });
    });

    // Baseline (Mutex)
    group.bench_function("baseline_mutex", |b| {
        let detector = baseline::MutexBasedAnomalyDetector::new();

        // Pre-populate scores
        for model_id in 0..5 {
            detector.update_score(model_id, 0.75);
        }

        b.iter(|| {
            let _ = detector.ensemble_vote();
        });
    });

    group.finish();
}

fn bench_detection_counter(c: &mut Criterion) {
    let mut group = c.benchmark_group("detection_counter");

    // Capsule (T1 Atomic)
    group.bench_function("capsule_t1", |b| {
        let capsule = BehavioralAnomalyCapsule::new();
        b.iter(|| {
            capsule.record_detection();
        });
    });

    // Baseline (Mutex)
    group.bench_function("baseline_mutex", |b| {
        let detector = baseline::MutexBasedAnomalyDetector::new();
        b.iter(|| {
            detector.record_detection();
        });
    });

    group.finish();
}

fn bench_false_positive_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("false_positive_rate");

    // Capsule (T1 Atomic)
    group.bench_function("capsule_t1", |b| {
        let capsule = BehavioralAnomalyCapsule::new();

        // Pre-populate counters
        for _ in 0..100 {
            capsule.record_detection();
        }
        for _ in 0..2 {
            capsule.record_false_positive();
        }

        b.iter(|| {
            let _ = capsule.false_positive_rate();
        });
    });

    group.finish();
}

fn bench_adaptive_threshold(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_threshold");

    // Capsule (T3 Fixed-Point)
    group.bench_function("capsule_t3", |b| {
        let capsule = BehavioralAnomalyCapsule::new();

        // Pre-populate counters for meaningful FPR
        for _ in 0..100 {
            capsule.record_detection();
        }
        for _ in 0..5 {
            capsule.record_false_positive();
        }

        b.iter(|| {
            let _ = capsule.adaptive_threshold_adjustment();
        });
    });

    group.finish();
}

// ============================================================================
// INTEGRATION BENCHMARKS (End-to-End Flows)
// ============================================================================

fn bench_full_detection_flow(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_detection_flow");

    // Capsule (T6 Mixed: T3+T1)
    group.bench_function("capsule_t6", |b| {
        let capsule = BehavioralAnomalyCapsule::new();

        b.iter(|| {
            // 1. Update all 5 model scores
            for model in ModelId::all() {
                capsule.update_score(model, black_box(0.85));
            }

            // 2. Ensemble vote
            let decision = capsule.ensemble_vote(black_box(AnomalyType::CommandSequence));

            // 3. Record detection if anomaly
            if let Decision::Anomaly { .. } = decision {
                capsule.record_detection();
            }
        });
    });

    // Baseline (Mutex)
    group.bench_function("baseline_mutex", |b| {
        let detector = baseline::MutexBasedAnomalyDetector::new();

        b.iter(|| {
            // 1. Update all 5 model scores
            for model_id in 0..5 {
                detector.update_score(model_id, black_box(0.85));
            }

            // 2. Ensemble vote
            let is_anomaly = detector.ensemble_vote();

            // 3. Record detection if anomaly
            if is_anomaly {
                detector.record_detection();
            }
        });
    });

    group.finish();
}

// ============================================================================
// CONCURRENT THROUGHPUT BENCHMARKS
// ============================================================================

fn bench_concurrent_score_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_score_updates");

    for thread_count in [1, 2, 4, 8, 16] {
        // Capsule (T3 Fixed-Point)
        group.bench_with_input(
            BenchmarkId::new("capsule_t3", thread_count),
            &thread_count,
            |b, &thread_count| {
                let capsule = Arc::new(BehavioralAnomalyCapsule::new());

                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|thread_id| {
                            let capsule_clone = Arc::clone(&capsule);
                            std::thread::spawn(move || {
                                for i in 0..100 {
                                    let model = ModelId::all()[thread_id % 5];
                                    let score = ((i + thread_id) % 100) as f64 / 100.0;
                                    capsule_clone.update_score(model, score);
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // Baseline (Mutex)
        group.bench_with_input(
            BenchmarkId::new("baseline_mutex", thread_count),
            &thread_count,
            |b, &thread_count| {
                let detector = Arc::new(baseline::MutexBasedAnomalyDetector::new());

                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|thread_id| {
                            let detector_clone = Arc::clone(&detector);
                            std::thread::spawn(move || {
                                for i in 0..100 {
                                    let model_id = thread_id % 5;
                                    let score = ((i + thread_id) % 100) as f64 / 100.0;
                                    detector_clone.update_score(model_id, score);
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_concurrent_ensemble_voting(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_ensemble_voting");

    for thread_count in [1, 2, 4, 8, 16] {
        // Capsule (T6 Mixed)
        group.bench_with_input(
            BenchmarkId::new("capsule_t6", thread_count),
            &thread_count,
            |b, &thread_count| {
                let capsule = Arc::new(BehavioralAnomalyCapsule::new());

                // Pre-populate scores
                for model in ModelId::all() {
                    capsule.update_score(model, 0.75);
                }

                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let capsule_clone = Arc::clone(&capsule);
                            std::thread::spawn(move || {
                                for _ in 0..100 {
                                    let _ = capsule_clone.ensemble_vote(AnomalyType::AccessPattern);
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // Baseline (Mutex)
        group.bench_with_input(
            BenchmarkId::new("baseline_mutex", thread_count),
            &thread_count,
            |b, &thread_count| {
                let detector = Arc::new(baseline::MutexBasedAnomalyDetector::new());

                // Pre-populate scores
                for model_id in 0..5 {
                    detector.update_score(model_id, 0.75);
                }

                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let detector_clone = Arc::clone(&detector);
                            std::thread::spawn(move || {
                                for _ in 0..100 {
                                    let _ = detector_clone.ensemble_vote();
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// THROUGHPUT BENCHMARKS (Events/Second)
// ============================================================================

fn bench_throughput_1m_events(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_1m_events");
    group.sample_size(10); // Reduce iterations for long-running test

    // Capsule (T6 Mixed)
    group.bench_function("capsule_t6", |b| {
        let capsule = BehavioralAnomalyCapsule::new();

        b.iter(|| {
            for i in 0..1_000_000 {
                if i % 100 == 0 {
                    // Update scores every 100 events
                    capsule.update_score(ModelId::RandomForest, 0.75);
                }

                if i % 50 == 0 {
                    // Ensemble vote every 50 events
                    let _ = capsule.ensemble_vote(AnomalyType::AccessPattern);
                }

                if i % 1000 == 0 {
                    // Record detection every 1000 events
                    capsule.record_detection();
                }
            }
        });
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    benches,
    bench_score_update,
    bench_ensemble_vote,
    bench_detection_counter,
    bench_false_positive_rate,
    bench_adaptive_threshold,
    bench_full_detection_flow,
    bench_concurrent_score_updates,
    bench_concurrent_ensemble_voting,
    bench_throughput_1m_events,
);

criterion_main!(benches);
