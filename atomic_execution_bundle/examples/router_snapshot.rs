use atomic_execution_bundle::{
    AtomicExecutionBundle, BracketsWord, DenyCounters, EntryLegWord, HeaderWord, RiskWord,
};

fn main() {
    let bundle = AtomicExecutionBundle::new();
    let counters = DenyCounters::new();
    bundle.publish(
        HeaderWord {
            stale: false,
            state: 1,
            kind: 2,
            has_bracket: true,
            reduce_only_bundle: false,
            spare_flags: 0,
            symbol_id: 12,
            strategy_id: 3,
            account_id: 77,
            pair_id: 99,
            created_ms_coarse: 25_000,
            ttl_ms: 1_200,
        },
        EntryLegWord {
            side_is_buy: true,
            anchor: 1,
            order_type: 2,
            tif: 1,
            quantity: 5,
            price_ticks: 12,
            route_id: 410,
            slip_cap_bp: 25,
            post_only: false,
            reduce_only: false,
            allow_partial: true,
            risk_tag: 0x3A,
            seq_hint: 17,
        },
        BracketsWord {
            take_profit_ticks: 8,
            stop_loss_ticks: -6,
            trailing_ticks: 0,
            time_stop_ms: 1_500,
            exit_route_id: 512,
            exit_tif: 1,
            take_profit_kind: 0,
            stop_loss_kind: 0,
            rearm_on_reentry: false,
            scale_out_pct: 0,
            slip_cap_exit_bp: 40,
            latency_budget_us: 2_000,
            flags: 0,
            oco_group: 18,
            spare: 0,
        },
        RiskWord {
            max_open_ms: 60_000,
            max_adverse_cents: 1_500,
            exit_on_breaker_ge_level: 1,
            exit_on_jitter: true,
            exit_on_cost_gt: false,
            forbid_after_min_ct: 930,
            eod_flat_min_ct: 1_050,
            fallback_route_id: 610,
            on_fail: 2,
            spare: 0,
        },
    );

    match bundle.load_with_diagnostics(Some(&counters)) {
        Ok(snapshot) => {
            let now_coarse = 25_500;
            if snapshot.ttl_expired(now_coarse) {
                eprintln!("bundle expired before routing");
                return;
            }

            let entry = snapshot.entry();
            let brackets = snapshot.brackets();
            let deadline = snapshot.ttl_deadline_coarse();

            println!(
                "route={} qty={} tp={} sl={} expires_at={}ms",
                entry.route_id,
                entry.quantity,
                brackets.take_profit_ticks,
                brackets.stop_loss_ticks,
                deadline
            );
        }
        Err(reason) => {
            eprintln!(
                "bundle denied: code={} ({})",
                reason.code(),
                reason.as_str()
            );
            let snap = counters.snapshot();
            eprintln!(
                "denials total accept={} stale={} seq_mismatch={} attempts={}",
                snap.accepts, snap.stale, snap.seq_mismatch, snap.attempts_exhausted
            );
        }
    }
}
