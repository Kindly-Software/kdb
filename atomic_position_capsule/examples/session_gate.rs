use atomic_position_capsule::{
    AtomicPositionCapsule, EquityWord, GateDecision, PositionHeadWord, SessionWord, TailWord,
};

fn main() {
    let capsule = AtomicPositionCapsule::new();
    capsule.publish(
        PositionHeadWord {
            position_qty: 3,
            avg_px_ticks: 1_250,
            remaining_daily_loss_cents: 45_000,
            flags: 0b0000_0011,
        },
        EquityWord {
            realized_cents: 1_800,
            unrealized_cents: 220,
            peak_equity_cents: 2_200,
            trailing_draw_cents: 180,
        },
        SessionWord {
            now_min_ct: 840,
            forbid_after_min_ct: 905,
            eod_flat_min_ct: 910,
            open_since_ms: 25_000,
            max_open_ms: 180_000,
            max_contracts: 5,
            max_per_trade_cents: 60_000,
            risk_flags: 0,
            reserved_bits: 0,
        },
        TailWord {
            symbol_id: 21,
            account_id: 88,
            last_exec_id: 12_345,
            breaker_level: 0,
            alt_health: 0,
            violation_bits: 0,
        },
    );

    let snapshot = capsule.load().expect("fresh snapshot");
    let desired_delta_qty = 2; // two more contracts in the same direction

    match snapshot.gate_order(desired_delta_qty) {
        GateDecision::Allow => {
            println!(
                "allow add-risk (pos {}, unreal {} cents)",
                snapshot.head().position_qty,
                snapshot.equity().unrealized_cents
            );
        }
        GateDecision::ReduceOnly => {
            println!("reduce-only: flatten or trim only");
        }
        GateDecision::Deny(reason) => {
            println!("deny: {:?}", reason);
        }
    }
}
