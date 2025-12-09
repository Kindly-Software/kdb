use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use atomic_cost_tracker::{ActSlot, ActSnapshot, FixedQ8_8};
use atomic_portfolio_map::{
    AccountSnapshot, ApmSlot, FeedAssembler, PortfolioController, PortfolioFlags,
    PortfolioMapWriter, PortfolioRuntime, SymbolGates, SymbolPolicy,
    adapters::{ActSlotFeed, CapsuleApcFeed, CapsuleAvsFeed},
};
use atomic_position_capsule::{
    AtomicPositionCapsule, CapsuleDraft, EquityWord, PositionHeadWord, SessionWord, TailWord,
};
use atomic_venue_snapshot::{Avs128, Avs128Snapshot};

fn main() {
    let apc_feed = CapsuleApcFeed::new();
    let act_feed = ActSlotFeed::new();
    let avs_feed = CapsuleAvsFeed::new();

    let capsule = Arc::new(AtomicPositionCapsule::new());
    let act_slot = Arc::new(ActSlot::default());
    let venue = Arc::new(Avs128::new());
    let mut draft = CapsuleDraft::new();
    draft
        .set_head(PositionHeadWord {
            position_qty: 2,
            avg_px_ticks: 0,
            remaining_daily_loss_cents: 220_000,
            flags: 0,
        })
        .set_equity(EquityWord {
            realized_cents: 90_000,
            unrealized_cents: 15_000,
            peak_equity_cents: 110_000,
            trailing_draw_cents: 5_000,
        })
        .set_session(SessionWord {
            now_min_ct: 905,
            forbid_after_min_ct: 910,
            eod_flat_min_ct: 920,
            open_since_ms: 0,
            max_open_ms: 0,
            max_contracts: 12,
            max_per_trade_cents: 0,
            risk_flags: 0,
            reserved_bits: 0,
        })
        .set_tail(TailWord {
            symbol_id: 1,
            account_id: 777,
            last_exec_id: 0,
            breaker_level: 1,
            alt_health: 0,
            violation_bits: 0,
        });
    capsule.publish_draft(&draft);
    apc_feed.register(1, capsule.clone());

    venue.publish(Avs128Snapshot {
        spread_ticks: 2,
        obi_q1_10: 64,
        micro_off_ticks: 1,
        sum_bid_l1_3: 150,
        sum_ask_l1_3: 135,
        vol_bp_q8_8: 512,
        sweep_flag: false,
        trend_200ms_ticks: 1,
        ts_coarse_ms: 10,
        version: 1,
        sequence: 3,
    });
    avs_feed.register(1, venue.clone());

    let mut act_snapshot = ActSnapshot::empty();
    act_snapshot.net = FixedQ8_8::saturating_from_bp(6.0);
    act_snapshot.min_required = FixedQ8_8::saturating_from_bp(4.0);
    act_slot.publish(&act_snapshot);
    act_feed.register(1, act_slot.clone());

    let assembler = FeedAssembler::new(
        Arc::new(apc_feed.clone()),
        Some(Arc::new(act_feed.clone())),
        Some(Arc::new(avs_feed.clone())),
    );

    let policy = SymbolPolicy {
        sym_id: 1,
        max_abs_position: 8,
        forbid_after_min_ct: Some(910),
        eod_flat_min_ct: Some(920),
        priority_offset: 0,
    };

    let apm_slot = ApmSlot::new();
    let writer = PortfolioMapWriter::new(apm_slot);
    let controller = PortfolioController::new(writer, Duration::from_millis(1_500));
    let mut runtime = PortfolioRuntime::new(controller, assembler, vec![policy]);

    let account = AccountSnapshot {
        account_id: 777,
        forbid_after_min_ct: 910,
        eod_flat_min_ct: 920,
        rem_daily_loss_total_cents: 950_000,
        trailing_draw_cents: 60_000,
        base_realized_cents: 60_000,
        created_ms_coarse: 41_500,
        portfolio_flags: PortfolioFlags::empty(),
    };

    let publish_time = Instant::now();
    let result = runtime
        .publish_cycle(&account, 905, 0, |_| SymbolGates::default())
        .expect("first publish succeeds");

    println!(
        "Published seq={} version={} priority={}",
        result.snapshot.header.seq,
        result.snapshot.header.version,
        result.snapshot.slices[0].priority
    );

    sleep(Duration::from_millis(750));
    runtime.tick(publish_time.elapsed().as_millis() as u64);
    println!(
        "Tick at 750ms => snapshot available? {}",
        runtime
            .controller()
            .writer()
            .slot()
            .load_relaxed()
            .is_some()
    );

    // Update feeds before the next publish.
    let mut refresh_draft = CapsuleDraft::new();
    refresh_draft
        .set_head(PositionHeadWord {
            position_qty: 3,
            avg_px_ticks: 0,
            remaining_daily_loss_cents: 180_000,
            flags: 0,
        })
        .set_equity(EquityWord {
            realized_cents: 90_000,
            unrealized_cents: 18_000,
            peak_equity_cents: 120_000,
            trailing_draw_cents: 6_000,
        })
        .set_session(SessionWord {
            now_min_ct: 906,
            forbid_after_min_ct: 910,
            eod_flat_min_ct: 920,
            open_since_ms: 0,
            max_open_ms: 0,
            max_contracts: 12,
            max_per_trade_cents: 0,
            risk_flags: 0,
            reserved_bits: 0,
        })
        .set_tail(TailWord {
            symbol_id: 1,
            account_id: 777,
            last_exec_id: 0,
            breaker_level: 1,
            alt_health: 0,
            violation_bits: 0,
        });
    capsule.publish_draft(&refresh_draft);
    let mut refresh = ActSnapshot::empty();
    refresh.net = FixedQ8_8::saturating_from_bp(10.0);
    refresh.min_required = FixedQ8_8::saturating_from_bp(4.0);
    act_slot.publish(&refresh);

    runtime
        .publish_cycle(
            &account,
            906,
            publish_time.elapsed().as_millis() as u64,
            |_| SymbolGates::default(),
        )
        .expect("refresh publish succeeds");
    println!(
        "Published refresh seq={}.",
        runtime.controller().writer().current_seq()
    );

    sleep(Duration::from_millis(1_800));
    runtime.tick(publish_time.elapsed().as_millis() as u64);
    println!(
        "Tick at 2.5s => snapshot available? {}",
        runtime
            .controller()
            .writer()
            .slot()
            .load_relaxed()
            .is_some()
    );
}
