use atomic_risk_envelope::{AtomicRiskEnvelope, Fields, OrderCheck, RiskEnvelope};
use std::sync::Arc;
use std::thread;

fn base_envelope() -> RiskEnvelope {
    RiskEnvelope::try_from_fields(Fields {
        rem_daily_loss_cents: 120_000,
        max_per_trade_cents: 60_000,
        max_contracts: 6,
        max_open_ms: 90_000,
        forbid_after_min_ct: 0,
        eod_flat_min_ct: 0,
        flags: atomic_risk_envelope::flag::Flags::EMPTY,
        version: 1,
        sequence: 0,
    })
    .unwrap()
}

#[test]
fn concurrent_debits_are_consistent() {
    use core::sync::atomic::Ordering;

    let envelope = base_envelope();
    let shared = Arc::new(AtomicRiskEnvelope::new(envelope));
    let workers = 8;
    let debits_per_worker = 1_000;
    let debit_size = 10; // cents

    let handles: Vec<_> = (0..workers)
        .map(|_| {
            let shared = Arc::clone(&shared);
            thread::spawn(move || {
                for _ in 0..debits_per_worker {
                    shared
                        .debit_daily_loss(debit_size, Ordering::AcqRel, Ordering::Acquire)
                        .ok();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("worker thread failed");
    }

    let initial = envelope.rem_daily_loss_cents() as u64;
    let expected_spent = (workers * debits_per_worker * debit_size) as u64;
    let final_env = shared.load(Ordering::Acquire);
    let remaining = final_env.rem_daily_loss_cents() as u64;
    assert_eq!(remaining, initial.saturating_sub(expected_spent));
}

#[test]
fn concurrent_orders_respect_updated_envelope() {
    use atomic_risk_envelope::GateOutcome;
    use core::sync::atomic::Ordering;

    let shared = Arc::new(AtomicRiskEnvelope::new(base_envelope()));
    let allow_order = OrderCheck::new(30_000, 2, 500, 20_000);
    assert!(matches!(
        shared.load(Ordering::Acquire).evaluate_order(allow_order),
        GateOutcome::Allow
    ));

    // Drain daily loss concurrently so subsequent orders are denied.
    let drain = Arc::clone(&shared);
    let handle = thread::spawn(move || {
        for _ in 0..5 {
            drain
                .debit_daily_loss(25_000, Ordering::AcqRel, Ordering::Acquire)
                .ok();
        }
    });
    handle.join().unwrap();

    let denied = shared.load(Ordering::Acquire).evaluate_order(allow_order);
    assert!(matches!(
        denied,
        GateOutcome::Deny(atomic_risk_envelope::DenyReason::DailyLossLimit { .. })
    ));
}

#[test]
fn reset_restores_daily_loss() {
    use core::sync::atomic::Ordering;

    let envelope = base_envelope();
    let atomic = AtomicRiskEnvelope::new(envelope);
    atomic
        .debit_daily_loss(60_000, Ordering::AcqRel, Ordering::Acquire)
        .expect("first debit succeeds");
    let baseline = envelope.rem_daily_loss_cents();
    let _ = atomic.fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
        current.with_rem_daily_loss_cents(baseline).ok()
    });
    assert_eq!(
        atomic.load(Ordering::Acquire).rem_daily_loss_cents(),
        baseline
    );
}
