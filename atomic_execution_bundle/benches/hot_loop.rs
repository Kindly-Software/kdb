use atomic_execution_bundle::{
    AtomicExecutionBundle, BracketsWord, BundleDraft, DenyCounters, EntryLegWord, HeaderWord,
    RiskWord,
};
use core_affinity::CoreId;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::hint::spin_loop;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn sample_payload() -> (HeaderWord, EntryLegWord, BracketsWord, RiskWord) {
    (
        HeaderWord {
            stale: false,
            state: 1,
            kind: 2,
            has_bracket: true,
            reduce_only_bundle: false,
            spare_flags: 0,
            symbol_id: 12,
            strategy_id: 5,
            account_id: 900,
            pair_id: 77,
            created_ms_coarse: 0xDEADB,
            ttl_ms: 2_000,
        },
        EntryLegWord {
            side_is_buy: true,
            anchor: 2,
            order_type: 2,
            tif: 2,
            quantity: 25,
            price_ticks: 980,
            route_id: 310,
            slip_cap_bp: 150,
            post_only: false,
            reduce_only: false,
            allow_partial: true,
            risk_tag: 12,
            seq_hint: 1_024,
        },
        BracketsWord {
            take_profit_ticks: 20,
            stop_loss_ticks: -10,
            trailing_ticks: 5,
            time_stop_ms: 9_000,
            exit_route_id: 204,
            exit_tif: 1,
            take_profit_kind: 0,
            stop_loss_kind: 1,
            rearm_on_reentry: true,
            scale_out_pct: 30,
            slip_cap_exit_bp: 120,
            latency_budget_us: 3_500,
            flags: 0x01,
            oco_group: 55,
            spare: 0,
        },
        RiskWord {
            max_open_ms: 60_000,
            max_adverse_cents: 1_000,
            exit_on_breaker_ge_level: 1,
            exit_on_jitter: true,
            exit_on_cost_gt: true,
            forbid_after_min_ct: 1_250,
            eod_flat_min_ct: 1_300,
            fallback_route_id: 111,
            on_fail: 2,
            spare: 0,
        },
    )
}

fn pick_core_pair() -> Option<(CoreId, CoreId)> {
    let ids = core_affinity::get_core_ids()?;
    let mut iter = ids.into_iter();
    let a = iter.next()?;
    let b = iter.next()?;
    Some((a, b))
}

fn bench_publish(c: &mut Criterion) {
    let mut group = c.benchmark_group("aeb_publish_cycle");
    group.bench_function("publish", |b| {
        let bundle = AtomicExecutionBundle::new();
        let (header, entry, brackets, risk) = sample_payload();
        b.iter(|| {
            let snap = bundle
                .publish(header, entry, brackets, risk)
                .expect("publish succeeds in benchmark");
            black_box(snap.sequence());
        });
    });

    group.bench_function("publish_with", |b| {
        let bundle = AtomicExecutionBundle::new();
        b.iter(|| {
            let snapshot = bundle
                .publish_with(|draft| {
                    let (header, entry, brackets, risk) = sample_payload();
                    draft
                        .set_header(header)
                        .set_entry(entry)
                        .set_brackets(brackets)
                        .set_risk(risk);
                })
                .expect("publish succeeds in benchmark");
            black_box(snapshot.version());
        });
    });

    group.finish();
}

fn bench_load(c: &mut Criterion) {
    c.bench_function("load_snapshot", |b| {
        let bundle = AtomicExecutionBundle::new();
        let payload = sample_payload();
        bundle
            .publish(payload.0, payload.1, payload.2, payload.3)
            .expect("publish succeeds in benchmark");
        b.iter(|| {
            let snapshot = bundle.load().expect("snapshot");
            black_box(snapshot.header().pair_id);
        });
    });
}

fn bench_cross_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("publish_load_cross_thread");

    group.bench_function("fast_path", |b| {
        let bundle = Arc::new(AtomicExecutionBundle::new());
        b.iter_custom(|iters| {
            let stop = Arc::new(AtomicBool::new(false));
            let reader_bundle = Arc::clone(&bundle);
            let reader_stop = Arc::clone(&stop);
            let affinity = pick_core_pair();
            let reader_pin = affinity.map(|(_, reader)| reader);
            if let Some((publish_core, _)) = affinity {
                let _ = core_affinity::set_for_current(publish_core);
            }
            let reader_handle = thread::spawn(move || {
                if let Some(core) = reader_pin {
                    let _ = core_affinity::set_for_current(core);
                }
                let mut last_seq: u16 = 0;
                while !reader_stop.load(AtomicOrdering::Acquire) {
                    if let Some(snapshot) = reader_bundle.load() {
                        let seq = snapshot.sequence();
                        if seq != last_seq {
                            last_seq = seq;
                        }
                    }
                    spin_loop();
                }
            });

            let (header, entry, brackets, risk) = sample_payload();
            let mut draft = BundleDraft::new();
            let start = Instant::now();
            for _ in 0..iters {
                let snapshot = bundle
                    .publish_with_reuse(&mut draft, |draft| {
                        draft
                            .set_header(header)
                            .set_entry(entry)
                            .set_brackets(brackets)
                            .set_risk(risk);
                    })
                    .expect("publish succeeds in benchmark");
                let expected = snapshot.sequence();
                black_box(snapshot.version());
                while bundle.tail_sequence() != expected {
                    spin_loop();
                }
            }
            let elapsed = start.elapsed();
            stop.store(true, AtomicOrdering::Release);
            reader_handle.join().expect("reader join");
            elapsed
        });
    });

    group.bench_function("diagnostics", |b| {
        let bundle = Arc::new(AtomicExecutionBundle::new());
        let counters = Arc::new(DenyCounters::new());
        b.iter_custom(|iters| {
            let stop = Arc::new(AtomicBool::new(false));
            let reader_bundle = Arc::clone(&bundle);
            let reader_stop = Arc::clone(&stop);
            let reader_counters = Arc::clone(&counters);
            let affinity = pick_core_pair();
            let reader_pin = affinity.map(|(_, reader)| reader);
            if let Some((publish_core, _)) = affinity {
                let _ = core_affinity::set_for_current(publish_core);
            }
            let reader_handle = thread::spawn(move || {
                if let Some(core) = reader_pin {
                    let _ = core_affinity::set_for_current(core);
                }
                let mut last_seq: u16 = 0;
                while !reader_stop.load(AtomicOrdering::Acquire) {
                    match reader_bundle.load_with_diagnostics(Some(&reader_counters)) {
                        Ok(snapshot) => {
                            let seq = snapshot.sequence();
                            if seq != last_seq {
                                last_seq = seq;
                            }
                        }
                        Err(_) => {}
                    }
                    spin_loop();
                }
            });

            let (header, entry, brackets, risk) = sample_payload();
            let mut draft = BundleDraft::new();
            let start = Instant::now();
            for _ in 0..iters {
                let snapshot = bundle
                    .publish_with_reuse(&mut draft, |draft| {
                        draft
                            .set_header(header)
                            .set_entry(entry)
                            .set_brackets(brackets)
                            .set_risk(risk);
                    })
                    .expect("publish succeeds in benchmark");
                let expected = snapshot.sequence();
                black_box(snapshot.version());
                while bundle.tail_sequence() != expected {
                    spin_loop();
                }
            }
            let elapsed = start.elapsed();
            stop.store(true, AtomicOrdering::Release);
            reader_handle.join().expect("reader join");
            elapsed
        });
    });

    group.finish();
}

fn register_benches(c: &mut Criterion) {
    bench_publish(c);
    bench_load(c);
    bench_cross_thread(c);
}

criterion_group!(benches, register_benches);
criterion_main!(benches);
