//! # B32 Benchmarks - LeaderElectionCapsule
//!
//! **Framework**: B32 (K1-K70) - Honest benchmarking with fair baselines
//! **Comparison**: Mutex-based leader election (fair baseline) vs LeaderElectionCapsule
//!
//! ## Performance Targets (B32)
//! - Vote: <50ns (CAS loop, max 3 retries typical)
//! - Check leader: <10ns (atomic load)
//! - Failover: <100ns (new epoch election)
//!
//! ## Baseline (Fair)
//! - Mutex<LeaderState> with RwLock for reads
//! - Represents typical distributed systems implementation
//! - Not a strawman: Uses standard library primitives

use atomic_capsule::patterns::{ElectionResult, LeaderElectionCapsule};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

// ============================================================================
// Baseline: Mutex-based leader election (fair baseline)
// ============================================================================

#[derive(Debug, Clone, Copy)]
struct BaselineLeaderInfo {
    epoch: u64,
    leader_id: u64,
    state: u8, // 0=NoLeader, 1=LeaderActive, 2=LeaderSuspected
}

struct BaselineLeaderElection {
    state: RwLock<BaselineLeaderInfo>,
}

impl BaselineLeaderElection {
    fn new() -> Self {
        Self {
            state: RwLock::new(BaselineLeaderInfo {
                epoch: 0,
                leader_id: 0,
                state: 0,
            }),
        }
    }

    fn vote(&self, node_id: u64, epoch: u64) -> bool {
        let mut state = self.state.write().unwrap();

        // Check epoch validity
        if epoch < state.epoch {
            return false;
        }

        // Check if leader already elected for this epoch
        if epoch == state.epoch && state.leader_id != 0 {
            return false;
        }

        // Become leader
        state.epoch = epoch;
        state.leader_id = node_id;
        state.state = 1; // LeaderActive
        true
    }

    fn check_leader(&self) -> Option<BaselineLeaderInfo> {
        let state = self.state.read().unwrap();
        if state.leader_id == 0 {
            None
        } else {
            Some(*state)
        }
    }

    fn trigger_failover(&self) -> u64 {
        let mut state = self.state.write().unwrap();
        state.epoch += 1;
        state.leader_id = 0;
        state.state = 0;
        state.epoch
    }

    fn mark_suspected(&self) -> bool {
        let mut state = self.state.write().unwrap();
        if state.state == 1 {
            state.state = 2;
            true
        } else {
            false
        }
    }
}

// ============================================================================
// Benchmark: Vote (Single-threaded)
// ============================================================================

fn bench_vote_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("vote_single_threaded");

    // Baseline: Mutex
    group.bench_function("baseline_mutex", |b| {
        let election = BaselineLeaderElection::new();
        let mut epoch = 0u64;
        b.iter(|| {
            epoch += 1;
            black_box(election.vote(1, epoch))
        });
    });

    // Optimized: LeaderElectionCapsule
    group.bench_function("optimized_capsule", |b| {
        let election = LeaderElectionCapsule::new();
        let mut epoch = 0u64;
        b.iter(|| {
            epoch += 1;
            black_box(election.vote(1, epoch))
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Check Leader (Single-threaded)
// ============================================================================

fn bench_check_leader_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("check_leader_single_threaded");

    // Baseline: Mutex
    group.bench_function("baseline_mutex", |b| {
        let election = BaselineLeaderElection::new();
        election.vote(1, 1);
        b.iter(|| black_box(election.check_leader()));
    });

    // Optimized: LeaderElectionCapsule
    group.bench_function("optimized_capsule", |b| {
        let election = LeaderElectionCapsule::new();
        election.vote(1, 1);
        b.iter(|| black_box(election.check_leader()));
    });

    group.finish();
}

// ============================================================================
// Benchmark: Trigger Failover (Single-threaded)
// ============================================================================

fn bench_trigger_failover_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("trigger_failover_single_threaded");

    // Baseline: Mutex
    group.bench_function("baseline_mutex", |b| {
        let election = BaselineLeaderElection::new();
        election.vote(1, 1);
        b.iter(|| black_box(election.trigger_failover()));
    });

    // Optimized: LeaderElectionCapsule
    group.bench_function("optimized_capsule", |b| {
        let election = LeaderElectionCapsule::new();
        election.vote(1, 1);
        b.iter(|| black_box(election.trigger_failover()));
    });

    group.finish();
}

// ============================================================================
// Benchmark: Mark Suspected (Single-threaded)
// ============================================================================

fn bench_mark_suspected_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("mark_suspected_single_threaded");

    // Baseline: Mutex
    group.bench_function("baseline_mutex", |b| {
        let election = BaselineLeaderElection::new();
        b.iter(|| {
            election.vote(1, 1);
            black_box(election.mark_suspected())
        });
    });

    // Optimized: LeaderElectionCapsule
    group.bench_function("optimized_capsule", |b| {
        let election = LeaderElectionCapsule::new();
        b.iter(|| {
            election.vote(1, 1);
            black_box(election.mark_suspected())
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: Concurrent Votes (Multi-threaded)
// ============================================================================

fn bench_concurrent_votes(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_votes");

    for num_threads in [4, 8, 16].iter() {
        // Baseline: Mutex
        group.bench_with_input(
            BenchmarkId::new("baseline_mutex", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let election = Arc::new(BaselineLeaderElection::new());
                    let handles: Vec<_> = (0..num_threads)
                        .map(|i| {
                            let election = Arc::clone(&election);
                            thread::spawn(move || {
                                election.vote(i + 1, 1);
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );

        // Optimized: LeaderElectionCapsule
        group.bench_with_input(
            BenchmarkId::new("optimized_capsule", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let election = Arc::new(LeaderElectionCapsule::new());
                    let handles: Vec<_> = (0..num_threads)
                        .map(|i| {
                            let election = Arc::clone(&election);
                            thread::spawn(move || {
                                election.vote(i + 1, 1);
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: Concurrent Check Leader (Multi-threaded)
// ============================================================================

fn bench_concurrent_check_leader(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_check_leader");

    for num_threads in [4, 8, 16].iter() {
        // Baseline: Mutex
        group.bench_with_input(
            BenchmarkId::new("baseline_mutex", num_threads),
            num_threads,
            |b, &num_threads| {
                let election = Arc::new(BaselineLeaderElection::new());
                election.vote(1, 1);

                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let election = Arc::clone(&election);
                            thread::spawn(move || {
                                for _ in 0..1000 {
                                    black_box(election.check_leader());
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );

        // Optimized: LeaderElectionCapsule
        group.bench_with_input(
            BenchmarkId::new("optimized_capsule", num_threads),
            num_threads,
            |b, &num_threads| {
                let election = Arc::new(LeaderElectionCapsule::new());
                election.vote(1, 1);

                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let election = Arc::clone(&election);
                            thread::spawn(move || {
                                for _ in 0..1000 {
                                    black_box(election.check_leader());
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark: Mixed Workload (Vote + Check + Failover)
// ============================================================================

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    // Baseline: Mutex
    group.bench_function("baseline_mutex", |b| {
        let election = Arc::new(BaselineLeaderElection::new());

        b.iter(|| {
            let handles: Vec<_> = (0..8)
                .map(|i| {
                    let election = Arc::clone(&election);
                    thread::spawn(move || {
                        for j in 0..100 {
                            match (i + j) % 3 {
                                0 => {
                                    election.vote(i + 1, j + 1);
                                }
                                1 => {
                                    election.check_leader();
                                }
                                _ => {
                                    election.trigger_failover();
                                }
                            }
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    // Optimized: LeaderElectionCapsule
    group.bench_function("optimized_capsule", |b| {
        let election = Arc::new(LeaderElectionCapsule::new());

        b.iter(|| {
            let handles: Vec<_> = (0..8)
                .map(|i| {
                    let election = Arc::clone(&election);
                    thread::spawn(move || {
                        for j in 0..100 {
                            match (i + j) % 3 {
                                0 => {
                                    election.vote(i + 1, j + 1);
                                }
                                1 => {
                                    election.check_leader();
                                }
                                _ => {
                                    election.trigger_failover();
                                }
                            }
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark: High Contention Election
// ============================================================================

fn bench_high_contention_election(c: &mut Criterion) {
    let mut group = c.benchmark_group("high_contention_election");
    group.sample_size(50); // Reduce sample size for expensive benchmark

    for num_threads in [16, 32, 64].iter() {
        // Baseline: Mutex
        group.bench_with_input(
            BenchmarkId::new("baseline_mutex", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let election = Arc::new(BaselineLeaderElection::new());
                    let handles: Vec<_> = (0..num_threads)
                        .map(|i| {
                            let election = Arc::clone(&election);
                            thread::spawn(move || {
                                election.vote(i + 1, 1);
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );

        // Optimized: LeaderElectionCapsule
        group.bench_with_input(
            BenchmarkId::new("optimized_capsule", num_threads),
            num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let election = Arc::new(LeaderElectionCapsule::new());
                    let handles: Vec<_> = (0..num_threads)
                        .map(|i| {
                            let election = Arc::clone(&election);
                            thread::spawn(move || {
                                election.vote(i + 1, 1);
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_vote_single_threaded,
    bench_check_leader_single_threaded,
    bench_trigger_failover_single_threaded,
    bench_mark_suspected_single_threaded,
    bench_concurrent_votes,
    bench_concurrent_check_leader,
    bench_mixed_workload,
    bench_high_contention_election,
);
criterion_main!(benches);
