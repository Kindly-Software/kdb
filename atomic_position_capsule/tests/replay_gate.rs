use atomic_execution_bundle::{
    AtomicExecutionBundle, BracketsWord, BundleDraft as AebDraft, EntryLegWord, HeaderWord,
    RiskWord,
};
use atomic_position_capsule::{
    AtomicPositionCapsule, CapsuleDraft, EquityWord, GateDecision, GateDeny, PositionHeadWord,
    SessionWord, TailWord, FLAG_HALT, FLAG_LOCKED, RISK_FLAG_NEWS_WINDOW,
};

struct GateCase {
    head: PositionHeadWord,
    equity: EquityWord,
    session: SessionWord,
    tail: TailWord,
    delta_qty: i32,
    expect: GateDecision,
}

fn sample_header() -> HeaderWord {
    HeaderWord {
        stale: false,
        state: 1,
        kind: 0,
        has_bracket: true,
        reduce_only_bundle: false,
        spare_flags: 0,
        symbol_id: 900,
        strategy_id: 12,
        account_id: 42,
        pair_id: 7,
        created_ms_coarse: 1,
        ttl_ms: 250,
    }
}

fn sample_entry(reduce_only: bool, qty: u32) -> EntryLegWord {
    EntryLegWord {
        side_is_buy: true,
        anchor: 1,
        order_type: 2,
        tif: 1,
        quantity: qty,
        price_ticks: 1_250,
        route_id: 15,
        slip_cap_bp: 35,
        post_only: false,
        reduce_only,
        allow_partial: true,
        risk_tag: 0,
        seq_hint: qty,
    }
}

fn sample_brackets() -> BracketsWord {
    BracketsWord {
        take_profit_ticks: 14,
        stop_loss_ticks: -7,
        trailing_ticks: 0,
        time_stop_ms: 1_500,
        exit_route_id: 18,
        exit_tif: 1,
        take_profit_kind: 0,
        stop_loss_kind: 0,
        rearm_on_reentry: false,
        scale_out_pct: 0,
        slip_cap_exit_bp: 45,
        latency_budget_us: 1_500,
        flags: 0,
        oco_group: 20,
        spare: 0,
    }
}

fn sample_risk() -> RiskWord {
    RiskWord {
        max_open_ms: 90_000,
        max_adverse_cents: 1_500,
        exit_on_breaker_ge_level: 1,
        exit_on_jitter: false,
        exit_on_cost_gt: false,
        forbid_after_min_ct: 905,
        eod_flat_min_ct: 910,
        fallback_route_id: 33,
        on_fail: 1,
        spare: 0,
    }
}

fn allow_head(flags: u8, remaining: u32, qty: i32) -> PositionHeadWord {
    PositionHeadWord {
        position_qty: qty,
        avg_px_ticks: 1_200,
        remaining_daily_loss_cents: remaining,
        flags,
    }
}

fn session(now: u16, forbid: u16, eod: u16, open_ms: u32, risk_flags: u8) -> SessionWord {
    SessionWord {
        now_min_ct: now,
        forbid_after_min_ct: forbid,
        eod_flat_min_ct: eod,
        open_since_ms: open_ms,
        max_open_ms: 120_000,
        max_contracts: 6,
        max_per_trade_cents: 75_000,
        risk_flags,
        reserved_bits: 0,
    }
}

fn tail(breaker: u8, viol: u16) -> TailWord {
    TailWord {
        symbol_id: 77,
        account_id: 42,
        last_exec_id: 0,
        breaker_level: breaker,
        alt_health: 0,
        violation_bits: viol,
    }
}

#[test]
fn apc_replay_gate_sequence() {
    let apc = AtomicPositionCapsule::new();
    let aeb = AtomicExecutionBundle::new();

    let cases = [
        GateCase {
            head: allow_head(0, 30_000, 2),
            equity: EquityWord::default(),
            session: session(845, 905, 910, 20_000, 0),
            tail: tail(0, 0),
            delta_qty: 1,
            expect: GateDecision::Allow,
        },
        GateCase {
            head: allow_head(0, 0, 3),
            equity: EquityWord::default(),
            session: session(850, 905, 910, 40_000, 0),
            tail: tail(0, 0),
            delta_qty: 1,
            expect: GateDecision::Deny(GateDeny::DailyLoss),
        },
        GateCase {
            head: allow_head(FLAG_LOCKED, 15_000, 3),
            equity: EquityWord::default(),
            session: session(851, 905, 910, 50_000, RISK_FLAG_NEWS_WINDOW),
            tail: tail(0, 0),
            delta_qty: 1,
            expect: GateDecision::ReduceOnly,
        },
        GateCase {
            head: allow_head(0, 15_000, 3),
            equity: EquityWord::default(),
            session: session(907, 905, 910, 75_000, 0),
            tail: tail(2, 0),
            delta_qty: -2,
            expect: GateDecision::ReduceOnly,
        },
        GateCase {
            head: allow_head(FLAG_HALT, 15_000, 3),
            equity: EquityWord::default(),
            session: session(908, 905, 910, 80_000, 0),
            tail: tail(2, 0),
            delta_qty: 1,
            expect: GateDecision::Deny(GateDeny::Halted),
        },
        GateCase {
            head: allow_head(FLAG_HALT, 15_000, 3),
            equity: EquityWord::default(),
            session: session(908, 905, 910, 80_000, 0),
            tail: tail(2, 1),
            delta_qty: -3,
            expect: GateDecision::ReduceOnly,
        },
    ];

    let mut apc_draft = CapsuleDraft::new();
    let mut aeb_draft = AebDraft::new();

    for (idx, case) in cases.iter().enumerate() {
        apc.publish_with_reuse(&mut apc_draft, |draft| {
            draft
                .set_head(case.head)
                .set_equity(case.equity)
                .set_session(case.session)
                .set_tail(case.tail);
        });

        let snapshot = apc.load().expect("snapshot should load");
        let decision = snapshot.gate_order(case.delta_qty);
        assert_eq!(decision, case.expect, "case {}", idx);

        match decision {
            GateDecision::Allow => {
                let bundle = aeb.publish_with_reuse(&mut aeb_draft, |draft| {
                    draft
                        .set_header(sample_header())
                        .set_entry(sample_entry(false, case.delta_qty.unsigned_abs().max(1)))
                        .set_brackets(sample_brackets())
                        .set_risk(sample_risk());
                });
                assert!(bundle.commit(), "bundle committed on allow");
                assert!(!bundle.entry().reduce_only);
            }
            GateDecision::ReduceOnly => {
                let bundle = aeb.publish_with_reuse(&mut aeb_draft, |draft| {
                    draft
                        .set_header(sample_header())
                        .set_entry(sample_entry(true, case.delta_qty.unsigned_abs().max(1)))
                        .set_brackets(sample_brackets())
                        .set_risk(sample_risk());
                });
                assert!(bundle.entry().reduce_only);
            }
            GateDecision::Deny(_reason) => {
                // no publish occurs
            }
        }
    }
}
