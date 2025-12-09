use atomic_risk_envelope::{flag, AtomicRiskEnvelope, Fields, OrderCheck, RiskEnvelope};
use core::sync::atomic::Ordering;

#[derive(Debug, Clone, Copy)]
struct Fill {
    cost_cents: u32,
    contracts: u16,
    open_duration_ms: u32,
}

fn main() {
    // Risk envelope for a prop account against which we will route orders/fills.
    let baseline = RiskEnvelope::try_from_fields(Fields {
        rem_daily_loss_cents: 50_000,
        max_per_trade_cents: 10_000,
        max_contracts: 8,
        max_open_ms: 90_000,
        forbid_after_min_ct: 900,
        eod_flat_min_ct: 915,
        flags: flag::Flags::EMPTY,
        version: 1,
        sequence: 0,
    })
    .expect("baseline envelope");

    let live = AtomicRiskEnvelope::new(baseline);

    // Synthetic order check before routing on the wire.
    let order = OrderCheck::new(8_000, 4, 780, 45_000);
    match live.load(Ordering::Acquire).evaluate_order(order) {
        atomic_risk_envelope::GateOutcome::Allow => {
            println!("order accepted – attempting to route");
        }
        atomic_risk_envelope::GateOutcome::Deny(reason) => {
            eprintln!("order blocked: {reason:?}");
            return;
        }
    }

    // Fills return from the exchange – debit remaining loss atomically.
    let fills = [
        Fill {
            cost_cents: 4_000,
            contracts: 2,
            open_duration_ms: 30_000,
        },
        Fill {
            cost_cents: 6_500,
            contracts: 2,
            open_duration_ms: 60_000,
        },
    ];

    for fill in fills {
        let previous = live
            .debit_daily_loss(fill.cost_cents, Ordering::SeqCst, Ordering::SeqCst)
            .expect("fill does not exceed remaining daily loss");

        let after = live.load(Ordering::SeqCst);
        println!(
            "fill cost={} contracts={} duration={}ms; prev remaining={} new remaining={}",
            fill.cost_cents,
            fill.contracts,
            fill.open_duration_ms,
            previous.rem_daily_loss_cents(),
            after.rem_daily_loss_cents()
        );
    }
}
