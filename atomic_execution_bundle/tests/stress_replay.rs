use atomic_execution_bundle::{
    AtomicExecutionBundle, BracketsWord, BundleDraft, EntryLegWord, HeaderWord, RiskWord,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

const TOTAL_PUBLISHES: u32 = 10_000;

#[test]
fn high_frequency_replay_consistency() {
    let bundle = Arc::new(AtomicExecutionBundle::new());
    let start = Arc::new(Barrier::new(2));
    let done = Arc::new(AtomicBool::new(false));
    let observed = Arc::new(Mutex::new(Vec::with_capacity(TOTAL_PUBLISHES as usize)));

    let writer_bundle = Arc::clone(&bundle);
    let writer_start = Arc::clone(&start);
    let writer_done = Arc::clone(&done);
    let writer = thread::spawn(move || {
        writer_start.wait();
        let mut draft = BundleDraft::new();
        for tick in 1..=TOTAL_PUBLISHES {
            writer_bundle.publish_with_reuse(&mut draft, |draft| {
                draft
                    .set_header(HeaderWord {
                        stale: false,
                        state: 1,
                        kind: 0,
                        has_bracket: true,
                        reduce_only_bundle: false,
                        spare_flags: 0,
                        symbol_id: 42,
                        strategy_id: 7,
                        account_id: 1984,
                        pair_id: 11,
                        created_ms_coarse: tick,
                        ttl_ms: 125,
                    })
                    .set_entry(EntryLegWord {
                        side_is_buy: tick % 2 == 0,
                        anchor: 1,
                        order_type: 2,
                        tif: 1,
                        quantity: tick,
                        price_ticks: (tick * 4) as i32,
                        route_id: 17,
                        slip_cap_bp: 25,
                        post_only: false,
                        reduce_only: false,
                        allow_partial: true,
                        risk_tag: 0,
                        seq_hint: tick,
                    })
                    .set_brackets(BracketsWord {
                        take_profit_ticks: 10,
                        stop_loss_ticks: -5,
                        trailing_ticks: 0,
                        time_stop_ms: 1_000,
                        exit_route_id: 23,
                        exit_tif: 1,
                        take_profit_kind: 0,
                        stop_loss_kind: 0,
                        rearm_on_reentry: false,
                        scale_out_pct: 0,
                        slip_cap_exit_bp: 50,
                        latency_budget_us: 2_000,
                        flags: 0,
                        oco_group: 12,
                        spare: 0,
                    })
                    .set_risk(RiskWord {
                        max_open_ms: 15_000,
                        max_adverse_cents: 1_000,
                        exit_on_breaker_ge_level: 1,
                        exit_on_jitter: false,
                        exit_on_cost_gt: false,
                        forbid_after_min_ct: 60,
                        eod_flat_min_ct: 120,
                        fallback_route_id: 42,
                        on_fail: 1,
                        spare: 0,
                    });
            });
        }
        writer_done.store(true, Ordering::Release);
    });

    let reader_bundle = Arc::clone(&bundle);
    let reader_start = Arc::clone(&start);
    let reader_done = Arc::clone(&done);
    let reader_observed = Arc::clone(&observed);

    let reader = thread::spawn(move || {
        reader_start.wait();
        let mut last_seen_seq = 0u16;
        loop {
            let (head, tail) = reader_bundle.sequence_pair();
            if head == tail && head != last_seen_seq {
                if let Some(snapshot) = reader_bundle.load() {
                    let entry = snapshot.entry();
                    let qty = entry.quantity;
                    if (qty as u16) == head {
                        last_seen_seq = head;
                        reader_observed.lock().unwrap().push(qty);
                        if qty == TOTAL_PUBLISHES {
                            break;
                        }
                    }
                }
            }
            if reader_done.load(Ordering::Acquire) && (last_seen_seq as u32) >= TOTAL_PUBLISHES {
                break;
            }
            thread::yield_now();
        }
    });

    writer.join().expect("writer thread");
    reader.join().expect("reader thread");

    let observed = observed.lock().unwrap();
    assert!(!observed.is_empty(), "reader captured no snapshots");
    assert_eq!(observed.last().copied().unwrap(), TOTAL_PUBLISHES);
    assert!(
        observed.len() as u32 >= TOTAL_PUBLISHES / 4,
        "captured only {} snapshots",
        observed.len()
    );
    for window in observed.windows(2) {
        assert!(window[0] < window[1], "quantities must increase strictly");
    }
    assert!(observed[0] >= 1);

    bundle.mark_stale();
    thread::sleep(Duration::from_millis(1));
    assert!(bundle.load().is_none());
}
