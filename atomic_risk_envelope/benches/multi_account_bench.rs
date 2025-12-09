use atomic_risk_envelope::{flag, AtomicRiskEnvelope, Fields, OrderCheck, RiskEnvelope};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::sync::Arc;
use std::thread;

fn random_fields(rng: &mut StdRng) -> Fields {
    let rem = rng.random_range(30_000..120_000);
    let max_trade = rng.random_range(5_000..=rem);
    let eod = rng.random_range(850..930);
    let forbid = rng.random_range(0..=eod);
    Fields {
        rem_daily_loss_cents: rem,
        max_per_trade_cents: max_trade,
        max_contracts: rng.random_range(1..24),
        max_open_ms: rng.random_range(10_000..120_000),
        forbid_after_min_ct: forbid,
        eod_flat_min_ct: eod,
        flags: if rng.random_bool(0.05) {
            flag::NEWS_LOCK
        } else {
            flag::Flags::EMPTY
        },
        version: 1,
        sequence: rng.random_range(0..64),
    }
}

fn bench_gate(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_thread_gate");
    let mut rng = StdRng::seed_from_u64(0xabba_feed);
    let configs = [32usize, 64, 128];

    for accounts in configs {
        group.bench_function(BenchmarkId::from_parameter(accounts), |b| {
            let envelopes: Vec<AtomicRiskEnvelope> = (0..accounts)
                .map(|_| {
                    AtomicRiskEnvelope::new(
                        RiskEnvelope::try_from_fields(random_fields(&mut rng)).unwrap(),
                    )
                })
                .collect();
            let mut order_rng = StdRng::seed_from_u64(0xfeed_dead);

            b.iter(|| {
                let idx = order_rng.random_range(0..accounts);
                let snapshot = envelopes[idx].load(std::sync::atomic::Ordering::Acquire);
                let order = OrderCheck::new(
                    order_rng.random_range(1_000..20_000),
                    order_rng.random_range(1..12),
                    order_rng.random_range(600..950),
                    order_rng.random_range(5_000..80_000),
                );
                black_box(snapshot.evaluate_order(order));
            });
        });
    }

    group.finish();
}

fn bench_multi_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_thread_gate");
    let mut rng = StdRng::seed_from_u64(0x1234_5678);
    let setups = [2usize, 4, 8];

    for threads in setups {
        group.bench_function(BenchmarkId::from_parameter(threads), |b| {
            let accounts = 128;
            let envelopes: Vec<AtomicRiskEnvelope> = (0..accounts)
                .map(|_| {
                    AtomicRiskEnvelope::new(
                        RiskEnvelope::try_from_fields(random_fields(&mut rng)).unwrap(),
                    )
                })
                .collect();
            let arc = Arc::new(envelopes);

            b.iter(|| {
                thread::scope(|scope| {
                    for tid in 0..threads {
                        let envs = Arc::clone(&arc);
                        scope.spawn(move || {
                            let mut rng = StdRng::seed_from_u64(0x7777_0000 ^ tid as u64);
                            for _ in 0..64 {
                                let idx = rng.random_range(0..envs.len());
                                let snapshot = envs[idx].load(std::sync::atomic::Ordering::Acquire);
                                let order = OrderCheck::new(
                                    rng.random_range(1_000..20_000),
                                    rng.random_range(1..12),
                                    rng.random_range(600..950),
                                    rng.random_range(5_000..80_000),
                                );
                                black_box(snapshot.evaluate_order(order));
                            }
                        });
                    }
                });
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_gate, bench_multi_thread);
criterion_main!(benches);
