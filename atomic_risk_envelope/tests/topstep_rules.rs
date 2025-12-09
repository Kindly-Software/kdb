use atomic_risk_envelope::{flag, Fields, GateOutcome, OrderCheck, RiskEnvelope};

fn topstep_envelope() -> RiskEnvelope {
    // Example: Topstep 150k Combine daily loss $3,000, per-trade $1,500, max 5 contracts,
    // must be flat by 15:10 (910 minute count), forbid new trades after 15:05 (905).
    let fields = Fields {
        rem_daily_loss_cents: 300_000,
        max_per_trade_cents: 150_000,
        max_contracts: 5,
        max_open_ms: 90_000,
        forbid_after_min_ct: 905,
        eod_flat_min_ct: 910,
        flags: flag::Flags::EMPTY,
        version: 1,
        sequence: 0,
    };
    RiskEnvelope::try_from_fields(fields).expect("valid topstep envelope")
}

#[test]
fn allows_trade_before_guards() {
    let env = topstep_envelope();
    let order = OrderCheck::new(120_000, 3, 845, 40_000);
    assert!(matches!(env.evaluate_order(order), GateOutcome::Allow));
}

#[test]
fn denies_excess_per_trade_risk() {
    let env = topstep_envelope();
    let order = OrderCheck::new(200_000, 2, 840, 30_000);
    assert!(matches!(
        env.evaluate_order(order),
        GateOutcome::Deny(atomic_risk_envelope::DenyReason::PerTradeLimit { .. })
    ));
}

#[test]
fn denies_after_forbid_time() {
    let env = topstep_envelope();
    // Order at or after forbid time should be denied
    let order = OrderCheck::new(100_000, 1, 905, 20_000);
    assert!(matches!(
        env.evaluate_order(order),
        GateOutcome::Deny(atomic_risk_envelope::DenyReason::SessionClosed { .. })
    ));
}

#[test]
fn denies_after_flat_time() {
    let mut fields = topstep_envelope().to_fields();
    fields.forbid_after_min_ct = 0; // disable forbid gate to surface EOD flat guard
    let env = RiskEnvelope::try_from_fields(fields).unwrap();
    let order = OrderCheck::new(80_000, 1, 911, 20_000);
    assert!(matches!(
        env.evaluate_order(order),
        GateOutcome::Deny(atomic_risk_envelope::DenyReason::PastEodFlat { .. })
    ));
}

#[test]
fn denies_when_paused() {
    let mut fields = topstep_envelope().to_fields();
    fields.flags = flag::PAUSED;
    let env = RiskEnvelope::try_from_fields(fields).unwrap();
    let order = OrderCheck::new(40_000, 1, 830, 10_000);
    assert!(matches!(
        env.evaluate_order(order),
        GateOutcome::Deny(atomic_risk_envelope::DenyReason::Paused)
    ));
}

#[test]
fn daily_loss_updates_guard_subsequent_orders() {
    use atomic_risk_envelope::AtomicRiskEnvelope;
    use core::sync::atomic::Ordering;

    let env = topstep_envelope();
    let atomic = AtomicRiskEnvelope::new(env);
    let allowed = OrderCheck::new(120_000, 2, 850, 30_000);
    assert!(matches!(
        atomic.load(Ordering::Acquire).evaluate_order(allowed),
        GateOutcome::Allow
    ));

    // Apply fills to exhaust remaining loss
    let prior = atomic
        .debit_daily_loss(280_000, Ordering::AcqRel, Ordering::Acquire)
        .expect("debit within balance");
    assert_eq!(prior.rem_daily_loss_cents(), 300_000);

    let after = atomic.load(Ordering::Acquire);
    assert_eq!(after.rem_daily_loss_cents(), 20_000);

    let next_order = OrderCheck::new(25_000, 1, 855, 25_000);
    assert!(matches!(
        after.evaluate_order(next_order),
        GateOutcome::Deny(atomic_risk_envelope::DenyReason::DailyLossLimit { .. })
    ));
}
