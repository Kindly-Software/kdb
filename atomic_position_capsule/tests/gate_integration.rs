use atomic_execution_bundle::{
    AtomicExecutionBundle, BracketsWord, BundleDraft as AebDraft, EntryLegWord, HeaderWord,
    RiskWord,
};
use atomic_position_capsule::{
    AtomicPositionCapsule, CapsuleDraft, EquityWord, GateDecision, GateDeny, PositionHeadWord,
    SessionWord, TailWord,
};

fn sample_header() -> HeaderWord {
    HeaderWord {
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
        created_ms_coarse: 100,
        ttl_ms: 150,
    }
}

fn sample_entry(reduce_only: bool, quantity: u32) -> EntryLegWord {
    EntryLegWord {
        side_is_buy: true,
        anchor: 1,
        order_type: 2,
        tif: 1,
        quantity,
        price_ticks: 500,
        route_id: 17,
        slip_cap_bp: 25,
        post_only: false,
        reduce_only,
        allow_partial: true,
        risk_tag: 0,
        seq_hint: quantity,
    }
}

fn sample_brackets() -> BracketsWord {
    BracketsWord {
        take_profit_ticks: 12,
        stop_loss_ticks: -6,
        trailing_ticks: 0,
        time_stop_ms: 1_000,
        exit_route_id: 21,
        exit_tif: 1,
        take_profit_kind: 0,
        stop_loss_kind: 0,
        rearm_on_reentry: false,
        scale_out_pct: 0,
        slip_cap_exit_bp: 40,
        latency_budget_us: 2_000,
        flags: 0,
        oco_group: 12,
        spare: 0,
    }
}

fn sample_risk() -> RiskWord {
    RiskWord {
        max_open_ms: 15_000,
        max_adverse_cents: 1_200,
        exit_on_breaker_ge_level: 1,
        exit_on_jitter: false,
        exit_on_cost_gt: false,
        forbid_after_min_ct: 905,
        eod_flat_min_ct: 910,
        fallback_route_id: 55,
        on_fail: 1,
        spare: 0,
    }
}

#[test]
fn apc_gate_controls_aeb_publishes() {
    let apc = AtomicPositionCapsule::new();
    let aeb = AtomicExecutionBundle::new();

    let mut apc_draft = CapsuleDraft::new();
    apc.publish_with_reuse(&mut apc_draft, |draft| {
        draft
            .set_head(PositionHeadWord {
                position_qty: 2,
                avg_px_ticks: 320,
                remaining_daily_loss_cents: 20_000,
                flags: 0,
            })
            .set_equity(EquityWord::default())
            .set_session(SessionWord {
                now_min_ct: 840,
                forbid_after_min_ct: 905,
                eod_flat_min_ct: 910,
                open_since_ms: 20_000,
                max_open_ms: 120_000,
                max_contracts: 6,
                max_per_trade_cents: 75_000,
                risk_flags: 0,
                reserved_bits: 0,
            })
            .set_tail(TailWord {
                symbol_id: 77,
                account_id: 99,
                last_exec_id: 0,
                breaker_level: 1,
                alt_health: 0,
                violation_bits: 0,
            });
    });

    let gate = apc.load().expect("initial snapshot");
    let decision = gate.gate_order(1);
    assert_eq!(decision, GateDecision::Allow);
    assert!(decision.permits(gate.head().position_qty, 1));

    let mut aeb_draft = AebDraft::new();
    let live_bundle = aeb.publish_with_reuse(&mut aeb_draft, |draft| {
        draft
            .set_header(sample_header())
            .set_entry(sample_entry(false, 3))
            .set_brackets(sample_brackets())
            .set_risk(sample_risk());
    });
    assert!(live_bundle.commit());
    assert_eq!(live_bundle.header().account_id, 1984);

    // Trip a violation bit to require flattening.
    apc.publish_with_reuse(&mut apc_draft, |draft| {
        draft
            .set_head(PositionHeadWord {
                position_qty: 5,
                avg_px_ticks: 320,
                remaining_daily_loss_cents: 5_000,
                flags: 0,
            })
            .set_equity(EquityWord::default())
            .set_session(SessionWord {
                now_min_ct: 907,
                forbid_after_min_ct: 905,
                eod_flat_min_ct: 910,
                open_since_ms: 130_000,
                max_open_ms: 120_000,
                max_contracts: 6,
                max_per_trade_cents: 75_000,
                risk_flags: 0,
                reserved_bits: 0,
            })
            .set_tail(TailWord {
                symbol_id: 77,
                account_id: 99,
                last_exec_id: live_bundle.sequence() as u32,
                breaker_level: 2,
                alt_health: 0,
                violation_bits: 1,
            });
    });

    let viol = apc.load().expect("violation snapshot");
    assert_eq!(
        viol.gate_order(1),
        GateDecision::Deny(GateDeny::ViolationBits)
    );
    let reduce_decision = viol.gate_order(-5);
    assert_eq!(reduce_decision, GateDecision::ReduceOnly);
    assert!(reduce_decision.permits(viol.head().position_qty, -5));

    // Reduce-only order is published with the bundle flagged accordingly.
    let reduce_bundle = aeb.publish_with_reuse(&mut aeb_draft, |draft| {
        draft
            .set_header(sample_header())
            .set_entry(sample_entry(true, 5))
            .set_brackets(sample_brackets())
            .set_risk(sample_risk());
    });
    assert!(reduce_bundle.entry().reduce_only);
    assert!(!reduce_bundle.header().reduce_only_bundle);
}
