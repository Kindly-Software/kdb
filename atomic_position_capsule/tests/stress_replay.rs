use atomic_position_capsule::{
    AtomicPositionCapsule, CapsuleDraft, EquityWord, PositionHeadWord, SessionWord, TailWord,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

const TOTAL_PUBLISHES: u32 = 10_000;

#[test]
fn high_frequency_replay_consistency() {
    let capsule = Arc::new(AtomicPositionCapsule::new());
    let start = Arc::new(Barrier::new(2));
    let done = Arc::new(AtomicBool::new(false));
    let observed = Arc::new(Mutex::new(Vec::with_capacity(TOTAL_PUBLISHES as usize)));

    let writer_capsule = Arc::clone(&capsule);
    let writer_start = Arc::clone(&start);
    let writer_done = Arc::clone(&done);

    let writer = thread::spawn(move || {
        writer_start.wait();
        let mut draft = CapsuleDraft::new();
        for tick in 1..=TOTAL_PUBLISHES {
            writer_capsule.publish_with_reuse(&mut draft, |draft| {
                draft
                    .set_head(PositionHeadWord {
                        position_qty: tick as i32,
                        avg_px_ticks: (tick as i32) * 2,
                        remaining_daily_loss_cents: 100_000u32.saturating_sub(tick * 3),
                        flags: if tick & 1 == 0 { 0b10 } else { 0b01 },
                    })
                    .set_equity(EquityWord {
                        realized_cents: tick as i32,
                        unrealized_cents: -(tick as i32),
                        peak_equity_cents: tick as i32 * 2,
                        trailing_draw_cents: tick,
                    })
                    .set_session(SessionWord {
                        now_min_ct: 840,
                        forbid_after_min_ct: 905,
                        eod_flat_min_ct: 910,
                        open_since_ms: tick * 10,
                        max_open_ms: 120_000,
                        max_contracts: 10,
                        max_per_trade_cents: 80_000,
                        risk_flags: 0,
                        reserved_bits: 0,
                    })
                    .set_tail(TailWord {
                        symbol_id: 77,
                        account_id: 900 + (tick as u16 % 5),
                        last_exec_id: tick,
                        breaker_level: (tick % 3) as u8,
                        alt_health: (tick % 8) as u8,
                        violation_bits: if tick % 250 == 0 { 0xFFFF } else { 0 },
                    });
            });
        }
        writer_done.store(true, Ordering::Release);
    });

    let reader_capsule = Arc::clone(&capsule);
    let reader_start = Arc::clone(&start);
    let reader_done = Arc::clone(&done);
    let reader_observed = Arc::clone(&observed);

    let reader = thread::spawn(move || {
        reader_start.wait();
        let mut last_seen_seq = 0u16;
        loop {
            let (head, tail) = reader_capsule.sequence_pair();
            if head == tail && head != last_seen_seq {
                if let Some(snapshot) = reader_capsule.load() {
                    let seq = snapshot.sequence();
                    if seq == head {
                        last_seen_seq = seq;
                        let pos = snapshot.head().position_qty;
                        reader_observed.lock().unwrap().push(pos);
                        if seq as u32 == TOTAL_PUBLISHES {
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
    assert_eq!(observed.last().copied().unwrap(), TOTAL_PUBLISHES as i32);
    for window in observed.windows(2) {
        assert!(window[0] < window[1], "positions must increase strictly");
    }
    assert!(observed[0] >= 1);

    thread::sleep(Duration::from_millis(2));
    assert!(capsule.load().is_some());
}
