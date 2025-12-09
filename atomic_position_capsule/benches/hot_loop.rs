use atomic_position_capsule::{
    AtomicPositionCapsule, CapsuleDraft, EquityWord, PositionHeadWord, SessionWord, TailWord,
};
use core_affinity::CoreId;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::hint::spin_loop;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn sample_payload() -> (PositionHeadWord, EquityWord, SessionWord, TailWord) {
    (
        PositionHeadWord {
            position_qty: 4,
            avg_px_ticks: 320,
            remaining_daily_loss_cents: 75_000,
            flags: 0b0001_0011,
        },
        EquityWord {
            realized_cents: 15_000,
            unrealized_cents: -1_200,
            peak_equity_cents: 17_500,
            trailing_draw_cents: 2_250,
        },
        SessionWord {
            now_min_ct: 845,
            forbid_after_min_ct: 905,
            eod_flat_min_ct: 910,
            open_since_ms: 60_000,
            max_open_ms: 120_000,
            max_contracts: 8,
            max_per_trade_cents: 80_000,
            risk_flags: 0b0000_0101,
            reserved_bits: 0,
        },
        TailWord {
            symbol_id: 101,
            account_id: 58,
            last_exec_id: 1_048_576,
            breaker_level: 1,
            alt_health: 3,
            violation_bits: 0,
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
    let mut group = c.benchmark_group("apc_publish");
    group.bench_function("publish", |b| {
        let capsule = AtomicPositionCapsule::new();
        let (head, equity, session, tail) = sample_payload();
        b.iter(|| {
            let snap = capsule.publish(head, equity, session, tail);
            black_box(snap.sequence());
        });
    });

    group.bench_function("publish_with", |b| {
        let capsule = AtomicPositionCapsule::new();
        b.iter(|| {
            let snapshot = capsule.publish_with(|draft| {
                let (head, equity, session, tail) = sample_payload();
                draft
                    .set_head(head)
                    .set_equity(equity)
                    .set_session(session)
                    .set_tail(tail);
            });
            black_box(snapshot.version());
        });
    });

    group.finish();
}

fn bench_load(c: &mut Criterion) {
    c.bench_function("load_snapshot", |b| {
        let capsule = AtomicPositionCapsule::new();
        let payload = sample_payload();
        capsule.publish(payload.0, payload.1, payload.2, payload.3);
        b.iter(|| {
            let snapshot = capsule.load().expect("snapshot");
            black_box(snapshot.head().remaining_daily_loss_cents);
        });
    });
}

fn bench_gate(c: &mut Criterion) {
    c.bench_function("gate_order", |b| {
        let capsule = AtomicPositionCapsule::new();
        let payload = sample_payload();
        capsule.publish(payload.0, payload.1, payload.2, payload.3);
        let snapshot = capsule.load().expect("snapshot");
        b.iter(|| {
            black_box(snapshot.gate_order(black_box(1)));
        });
    });
}

fn bench_cross_thread(c: &mut Criterion) {
    c.bench_function("publish_load_cross_thread", |b| {
        let capsule = Arc::new(AtomicPositionCapsule::new());
        let stop = Arc::new(AtomicBool::new(false));

        let affinity = pick_core_pair();
        if let Some((publish_core, _)) = affinity {
            let _ = core_affinity::set_for_current(publish_core);
        }
        let reader_pin = affinity.map(|(_, reader)| reader);

        let reader_capsule = Arc::clone(&capsule);
        let reader_stop = Arc::clone(&stop);

        let handle = thread::spawn(move || {
            if let Some(core) = reader_pin {
                let _ = core_affinity::set_for_current(core);
            }
            let mut last_seq: u16 = 0;
            while !reader_stop.load(AtomicOrdering::Acquire) {
                if let Some(snapshot) = reader_capsule.load() {
                    let seq = snapshot.sequence();
                    if seq != last_seq {
                        last_seq = seq;
                    }
                }
                spin_loop();
            }
        });

        let payload = sample_payload();
        let mut draft = CapsuleDraft::new();

        b.iter_custom(|iters| {
            let (mut last_seen, _) = capsule.sequence_pair();
            let start = Instant::now();
            for _ in 0..iters {
                let snapshot = capsule.publish_with_reuse(&mut draft, |draft| {
                    draft
                        .set_head(payload.0)
                        .set_equity(payload.1)
                        .set_session(payload.2)
                        .set_tail(payload.3);
                });
                black_box(snapshot.version());
                loop {
                    let (head, tail) = capsule.sequence_pair();
                    if head == tail && head != last_seen {
                        last_seen = head;
                        break;
                    }
                    spin_loop();
                }
            }
            start.elapsed()
        });

        stop.store(true, AtomicOrdering::Release);
        handle.join().expect("reader join");
    });
}

fn register_benches(c: &mut Criterion) {
    bench_publish(c);
    bench_load(c);
    bench_gate(c);
    bench_cross_thread(c);
}

criterion_group!(benches, register_benches);
criterion_main!(benches);
