use super::*;
use crate::integrity::tile_bytes;
use crate::reader::validate_tile;
use crate::session::RouterMetrics;
use crate::session::{
    APM_SUMMARY_BREAKER_MASK, APM_SUMMARY_BREAKER_SHIFT, APM_SUMMARY_FLAGS_MASK,
    APM_SUMMARY_FLAGS_SHIFT, APM_SUMMARY_HEADROOM_SCALE_CENTS, APM_SUMMARY_HEADROOM_SHIFT,
    APM_SUMMARY_SYMBOL_MASK, APM_SUMMARY_SYMBOL_SHIFT,
};
use atomic_event_lockout_map::{
    AccountScope, BuildRequest, EcoWriter, EventAction, EventSeverity, EventWindow, GlobalFlag,
};
use std::sync::atomic::Ordering;

use atomic_breaker::AtomicBreakerSWeMR;
use atomic_cost_tracker::{
    layout::{ActFlags, ActSnapshot, FixedQ8_8},
    writer::ActSlot,
};
use atomic_execution_bundle::{
    AtomicExecutionBundle, BracketsWord, BundleDraft, EntryLegWord, HeaderWord as AebHeaderWord,
    RiskWord,
};
use atomic_portfolio_map::{layout::ApmSnapshot, slot::ApmSlot};
use atomic_pre_execution_capsule::{PexCapsule, PexDraft};
use atomic_risk_envelope::{flag, AtomicRiskEnvelope, Fields as AreFields, RiskEnvelope};
use atomic_risk_ladder_table::layout::actions::ActionWord;
use atomic_risk_ladder_table::{
    layout::{header::HeaderWord as RltHeaderWord, tail::TailWord as RltTailWord},
    Rlt1024,
};

const HASH_KEY: [u8; 32] = [0xAB; 32];

fn hash_bytes(bytes: &[u8]) -> u64 {
    let digest = TileHash::blake3_64(bytes);
    u64::from_le_bytes(digest)
}

fn hash_words(words: &[u128]) -> u64 {
    let mut buffer = [0u8; 16 * 8];
    for (idx, word) in words.iter().enumerate() {
        buffer[idx * 16..(idx + 1) * 16].copy_from_slice(&word.to_le_bytes());
    }
    hash_bytes(&buffer[..words.len() * 16])
}

fn hash_u128(value: u128) -> u64 {
    hash_bytes(&value.to_le_bytes())
}

fn hash_u64(value: u64) -> u64 {
    hash_bytes(&value.to_le_bytes())
}

#[test]
fn layout_sizes_match_contract() {
    assert_eq!(core::mem::size_of::<HeaderSection>(), 256);
    assert_eq!(core::mem::size_of::<CountersSection>(), 256);
    assert_eq!(core::mem::size_of::<SymbolSection>(), 256);
    assert_eq!(core::mem::size_of::<LogSection>(), 256);
    assert_eq!(core::mem::size_of::<EtTile>(), 1024);
}

#[test]
fn checksum_ignores_commit_flag() {
    let mut tile = EtTile::default();
    tile.header.ver_even = 3;
    tile.header.seq_head = 9;
    tile.header.epoch_id = 42;
    let checksum_with_commit_zero = tile_checksum32(&tile);

    tile.header.commit = 1;
    let checksum_with_commit_one = tile_checksum32(&tile);

    assert_eq!(checksum_with_commit_zero, checksum_with_commit_one);
}

#[test]
fn publish_into_commits_tile_and_updates_hash_chain() {
    let mut ring = vec![EtTile::default(); 2];
    let mut publisher = TilePublisher::new(Some(HASH_KEY));
    let mut shadow = TileShadow::new();

    {
        let tile = shadow.tile_mut();
        tile.header.epoch_id = 1001;
        tile.header.created_ms = 1_700_000_000_000;
        tile.header.policy_id = 7;
        tile.header.account_id = 17;
        tile.header.capsule_digests[0] = 0xDEADBEEFDEADBEEF;
    }

    let outcome = publisher.publish_into(TileSlot::new(0, &mut ring[0]), &mut shadow);

    assert_eq!(outcome.tile_index, 0);
    assert_eq!(outcome.seq_head, 0);
    assert_eq!(outcome.ver_even & 1, 0);

    let committed = &ring[0];
    assert_eq!(committed.header.commit, 1);
    assert_eq!(committed.header.seq_head, outcome.seq_head);
    assert_eq!(committed.log.tail.ver_tail, outcome.ver_even);
    if let Err(err) = validate_tile(committed) {
        panic!("validation failed: {err}");
    }

    let expected_hash = TileHash::keyed_128(&HASH_KEY, tile_bytes(committed));
    assert_eq!(publisher.prev_tile_hash(), expected_hash);

    assert_eq!(tile_checksum32(committed), outcome.checksum);
}

#[test]
fn scan_latest_committed_skips_corrupt_tiles() {
    let mut ring = vec![EtTile::default(); 3];
    let mut publisher = TilePublisher::new(Some(HASH_KEY));
    let mut shadow = TileShadow::new();

    for idx in 0..3 {
        {
            let tile = shadow.tile_mut();
            tile.header.epoch_id = idx as u64;
            tile.header.created_ms = 10 + idx as u64;
        }
        publisher.publish_into(TileSlot::new(idx as u16, &mut ring[idx]), &mut shadow);
    }

    ring[2].header.checksum32 ^= 0xFFFF;

    let latest = scan_latest_committed(&ring, 2).expect("expected a valid tile");
    assert_eq!(latest.0, 1);
    assert_eq!(latest.1.header.epoch_id, 1);
}

#[test]
fn builder_populates_tile_sections() {
    let header_inputs = HeaderInputs {
        epoch_id: 55,
        created_ms: 1234,
        run_id: 0xAABBCCDDEEFF00112233445566778899,
        policy_id: 9,
        account_id: 3,
        tz_id: 2,
        symbol_mask: 0b0011,
        forbid_after_min_ct: 15,
        eod_flat_min_ct: 20,
        applied_level: 2,
        global_flags: 1,
        prev_tile_hash: [0x11; 16],
        ale_tail_hash: 0xCAFEBABE,
        capsule_digests: [1, 2, 3, 4, 5, 6, 7, 8],
        tile_index: 0,
        created_seq_head: 7,
    };

    let mut inputs = TileInputs::new(header_inputs);
    inputs.counters = CountersInputs {
        orders_sent: 10,
        acks: 8,
        fills: 5,
        cancels: 2,
        rejects: 1,
        maker_sends: 6,
        taker_sends: 4,
        reduce_only: 1,
        qty_traded: 42,
        trades_won: 3,
        trades_lost: 1,
        realized_cents: 1_000,
        unreal_cents: -500,
        fees_cents: 120,
        slip_mbp_sum: -25,
        slip_mbp_abs_sum: 90,
        peak_equity_cents: 2_000,
        max_draw_cents: -300,
        lat_d2a_quantiles: [10, 20, 30],
        lat_a2f_quantiles: [5, 15, 25],
        rej_rate_bp: 12,
        cxl_rate_bp: 33,
        loss_bp: 8,
        jitter_us: 4,
        lat_hist8: [1, 2, 3, 4, 5, 6, 7, 8],
        slip_hist8: [8, 7, 6, 5, 4, 3, 2, 1],
    };

    inputs.symbols = vec![SymbolInputs {
        sym_id: 101,
        breaker_level: 2,
        flags: 0b101,
        pos_qty: 12,
        avg_px_ticks: 128,
        realized_cents: 500,
        unreal_cents: -250,
        rem_daily_loss_cents: 1_000,
        trailing_draw_cents: 750,
        spread_ticks: 4,
        vol_bp_q8_8: 512,
        obi_q1_10: 30,
        last_exec_id: 0xDEAD,
        sum_bid_l1_3: 111,
        sum_ask_l1_3: 222,
    }];

    inputs.log.entries = vec![LogInputsEntry {
        ts_ms: 99,
        event: 2,
        actor: 3,
        sym_id: 101,
        code: 777,
        aux: -2,
        flags: 0b010,
    }];
    inputs.log.head = 1;
    inputs.log.count = 1;
    inputs.log.now_min_ct = 44;
    inputs.log.next_lockout_min_ct = 45;
    inputs.log.next_resume_min_ct = 46;
    inputs.log.eco_action_now = 2;
    inputs.log.apm_summary = 0xABCD;

    let mut tile = EtTile::default();
    populate_tile(&mut tile, &inputs);

    assert_eq!(tile.header.epoch_id, 55);
    assert_eq!(tile.counters.orders_sent, 10);
    assert_eq!(tile.symbols.slots[0].sym_id, 101);
    assert_eq!(tile.log.entries[0].ts_ms, 99);
    assert_eq!(tile.log.tail.mini_head, 1);
    assert_eq!(tile.log.tail.tile_index, 0);
    assert_eq!(tile.log.tail.apm_summary, 0xABCD);
}

#[test]
fn log_inputs_apply_eco_snapshot_sets_fields() {
    let mut writer = EcoWriter::new(AccountScope::new(9, 0))
        .with_origin_minute(480)
        .with_mask_length(512)
        .with_baseline_window(480, 540);
    let events = [EventWindow::econ(
        500,
        505,
        EventSeverity::High,
        EventAction::ForbidNew,
    )];

    let snapshot = writer.build_and_publish(BuildRequest {
        now_min_ct: 500,
        age_8ms: 8,
        created_ms_coarse: 1_024,
        events: &events,
        global_flags: GlobalFlag::empty(),
        manual_pause: false,
        day_of_week: 1,
        holiday_flag: false,
    });

    let mut log_inputs = LogInputs::default();
    log_inputs.apply_eco_snapshot(&snapshot);

    let tail = snapshot.tail();
    let expected_lockout = snapshot.next_lockout_minute().unwrap_or_default();
    let expected_resume = snapshot.next_resume_minute().unwrap_or_default();

    assert_eq!(log_inputs.now_min_ct, tail.now_min_ct);
    assert_eq!(log_inputs.next_lockout_min_ct, expected_lockout);
    assert_eq!(log_inputs.next_resume_min_ct, expected_resume);
    assert_eq!(log_inputs.eco_action_now, tail.active_action as u8);
}

#[test]
fn ring_supports_publish_and_readback() {
    use std::path::PathBuf;

    let dir = tempfile::tempdir().expect("temp dir");
    let mut path = PathBuf::from(dir.path());
    path.push("et_ring.bin");

    let mut ring = TileRing::create(&path, 4).expect("create ring");
    let mut publisher = TilePublisher::new(Some(HASH_KEY));
    let mut shadow = TileShadow::new();

    {
        let header = HeaderInputs {
            epoch_id: 1,
            created_ms: 42,
            run_id: 0x11,
            policy_id: 2,
            account_id: 3,
            tz_id: 1,
            symbol_mask: 0,
            forbid_after_min_ct: 0,
            eod_flat_min_ct: 0,
            applied_level: 0,
            global_flags: 0,
            prev_tile_hash: [0; 16],
            ale_tail_hash: 0,
            capsule_digests: [0; 8],
            tile_index: 0,
            created_seq_head: 0,
        };
        let inputs = TileInputs::new(header);
        populate_tile(shadow.tile_mut(), &inputs);
    }

    let slot = ring.tile_slot(0);
    let outcome = publisher.publish_into(slot, &mut shadow);
    assert_eq!(outcome.tile_index, 0);
    ring.flush(FlushStrategy::Async).expect("flush");

    drop(ring);

    let mapping = TileRingMapping::open(&path, 4).expect("open mapping");
    let (idx, tile) = scan_latest_committed(mapping.tiles(), 3).expect("tile");
    assert_eq!(idx, 0);
    assert_eq!(tile.header.epoch_id, 1);
    assert_eq!(tile.header.commit, 1);
}

#[test]
fn build_tile_inputs_uses_live_feeds() {
    use atomic_latency_ticket::{AltAtomic, AltSample};
    use atomic_position_capsule::{
        AtomicPositionCapsule, EquityWord, PositionHeadWord, SessionWord, TailWord, FLAG_LONG,
    };
    use atomic_venue_snapshot::{Avs128, Avs128Snapshot};

    let position = AtomicPositionCapsule::new();
    let avs = Avs128::default();
    let alt = AltAtomic::new();
    let breaker = AtomicBreakerSWeMR::from_packed(0x1234_5678);

    let act_slot = ActSlot::default();
    let mut act_snapshot = ActSnapshot::empty();
    act_snapshot.net = FixedQ8_8::saturating_from_bp(320.0);
    act_snapshot.min_required = FixedQ8_8::saturating_from_bp(180.0);
    act_snapshot.flags = ActFlags::OK | ActFlags::MAKER;
    act_snapshot.version = 3;
    act_snapshot.seq = 4;
    act_snapshot.age_ms_bucket = 2;
    act_slot.publish(&act_snapshot);

    let are_fields = AreFields {
        rem_daily_loss_cents: 250_000,
        max_per_trade_cents: 25_000,
        max_contracts: 12,
        max_open_ms: 30_000,
        forbid_after_min_ct: 800,
        eod_flat_min_ct: 900,
        flags: flag::Flags::from_bits_truncate(0b0011),
        version: 2,
        sequence: 9,
    };
    let risk_envelope = AtomicRiskEnvelope::new(RiskEnvelope::try_from_fields(are_fields).unwrap());

    let apm_slot = ApmSlot::new();
    let mut apm_snapshot = ApmSnapshot::empty();
    apm_snapshot.header.commit = true;
    apm_snapshot.header.version = 2;
    apm_snapshot.header.seq = 5;
    apm_snapshot.header.account_id = 333;
    apm_snapshot.header.symbol_count = 1;
    apm_snapshot.slices[0].sym_id = 42;
    apm_snapshot.slices[0].pos_qty = 10;
    apm_snapshot.slices[0].rem_daily_loss_cents = 75_000;
    apm_snapshot.tail.version = 2;
    apm_snapshot.tail.seq = 5;
    apm_snapshot.tail.net_realized_cents = 700;
    apm_snapshot.tail.net_unreal_cents = -400;
    apm_snapshot.tail.sum_pos_abs_contracts = 20;
    let apm_words = apm_snapshot.pack();
    apm_slot.publish(&apm_words);

    let pex_capsule = PexCapsule::new();
    let mut pex_draft = PexDraft::default();
    pex_draft.commit = true;
    pex_draft.stale = false;
    pex_draft.odd_version = 1;
    pex_draft.seq = 2;
    pex_draft.header.account_id = 333;
    pex_capsule.publish(&pex_draft);

    let aeb = AtomicExecutionBundle::new();
    let mut bundle_draft = BundleDraft::new();
    bundle_draft
        .set_header(AebHeaderWord {
            stale: false,
            state: 1,
            kind: 0,
            has_bracket: false,
            reduce_only_bundle: false,
            spare_flags: 0,
            symbol_id: 42,
            strategy_id: 7,
            account_id: 333,
            pair_id: 1,
            created_ms_coarse: 123,
            ttl_ms: 50,
        })
        .set_entry(EntryLegWord {
            side_is_buy: true,
            anchor: 1,
            order_type: 2,
            tif: 3,
            quantity: 5,
            price_ticks: 10,
            route_id: 4,
            slip_cap_bp: 12,
            post_only: false,
            reduce_only: false,
            allow_partial: true,
            risk_tag: 3,
            seq_hint: 9,
        })
        .set_brackets(BracketsWord {
            take_profit_ticks: 6,
            stop_loss_ticks: -4,
            trailing_ticks: 2,
            time_stop_ms: 40,
            exit_route_id: 1,
            exit_tif: 2,
            take_profit_kind: 1,
            stop_loss_kind: 1,
            rearm_on_reentry: false,
            scale_out_pct: 50,
            slip_cap_exit_bp: 20,
            latency_budget_us: 150,
            flags: 0,
            oco_group: 0,
            spare: 0,
        })
        .set_risk(RiskWord {
            max_open_ms: 10_000,
            max_adverse_cents: 2_000,
            exit_on_breaker_ge_level: 1,
            exit_on_jitter: false,
            exit_on_cost_gt: true,
            forbid_after_min_ct: 700,
            eod_flat_min_ct: 800,
            fallback_route_id: 5,
            on_fail: 2,
            spare: 0,
        });
    aeb.publish_with(|draft| *draft = bundle_draft);

    let mut rlt = Rlt1024::new();
    let mut rlt_header = RltHeaderWord::ZERO;
    rlt_header.set_commit(true);
    rlt_header.set_version_even(2);
    rlt_header.set_seq_head(6);
    rlt_header.set_policy_id(99);
    rlt.header = rlt_header;
    let mut rlt_tail = RltTailWord::ZERO;
    rlt_tail.set_version(2);
    rlt_tail.set_seq_tail(6);
    rlt_tail.set_checksum(0xABCD);
    rlt.tail = rlt_tail;
    rlt.strat_a_actions = ActionWord::from_raw(0xDEAD_BEEF);

    let router_metrics = RouterMetrics {
        orders_sent: 20,
        acks: 18,
        fills: 9,
        cancels: 4,
        rejects: 2,
        maker_sends: 7,
        taker_sends: 5,
        reduce_only: 3,
        qty_traded: 12,
        trades_won: 6,
        trades_lost: 1,
        fees_cents: 220,
        slip_mbp_sum: -45,
        slip_mbp_abs_sum: 90,
    };

    let head = PositionHeadWord {
        position_qty: 12,
        avg_px_ticks: 34,
        remaining_daily_loss_cents: 1_500,
        flags: FLAG_LONG,
    };
    let equity = EquityWord {
        realized_cents: 900,
        unrealized_cents: -120,
        peak_equity_cents: 1_200,
        trailing_draw_cents: 250,
    };
    let session_word = SessionWord {
        now_min_ct: 77,
        forbid_after_min_ct: 90,
        eod_flat_min_ct: 120,
        open_since_ms: 1_000,
        max_open_ms: 10_000,
        max_contracts: 15,
        max_per_trade_cents: 25_000,
        risk_flags: 0,
        reserved_bits: 0,
    };
    let tail = TailWord {
        symbol_id: 42,
        account_id: 333,
        last_exec_id: 0xDEAD_BEEF,
        breaker_level: 2,
        alt_health: 1,
        violation_bits: 0,
    };

    position.publish(head, equity, session_word, tail);

    avs.publish(Avs128Snapshot {
        spread_ticks: 5,
        obi_q1_10: -64,
        micro_off_ticks: 0,
        sum_bid_l1_3: 120,
        sum_ask_l1_3: 95,
        vol_bp_q8_8: 512,
        sweep_flag: false,
        trend_200ms_ticks: 0,
        ts_coarse_ms: 0,
        version: 1,
        sequence: 1,
    });

    alt.publish_sample(AltSample {
        feed_to_decision_us: 90,
        decision_to_ack_us: 120,
        ack_to_first_fill_us: 180,
        reject_rate_bps: 11,
        cancel_rate_bps: 31,
        loss_rate_bps: 7,
        jitter_us: 40,
        queue_position: 0.25,
        flags: 0,
        version: 2,
        sequence: 5,
        age_ms: 16,
    });

    let feeds = LiveFeeds {
        position: &position,
        venue: Some(&avs),
        latency: Some(&alt),
        breaker: Some(&breaker),
        cost_tracker: Some(&act_slot),
        risk_envelope: Some(&risk_envelope),
        portfolio_map: Some(&apm_slot),
        pre_execution: Some(&pex_capsule),
        execution_bundle: Some(&aeb),
        risk_ladder: Some(&rlt),
        router_metrics: Some(router_metrics),
    };

    let meta = SessionMetadata {
        epoch_id: 7,
        created_ms: 1_700_000_123,
        run_id: 0xAA55,
        policy_id: 3,
        tz_id: 11,
        tile_index: 0,
        ale_tail_hash: 0x1122_3344_5566_7788,
        prev_tile_hash: [0u8; 16],
        capsule_digests: [0; 8],
    };

    let counters = CountersInputs::default();
    let log = LogInputs::default();

    let inputs = build_tile_inputs(&feeds, meta, counters, log).expect("tile inputs");

    assert_eq!(inputs.header.account_id, 333);
    assert_eq!(inputs.header.symbol_mask, 0b1);
    assert_eq!(inputs.counters.orders_sent, 20);
    assert_eq!(inputs.counters.acks, 18);
    assert_eq!(inputs.counters.fees_cents, 220);
    assert_eq!(inputs.counters.slip_mbp_sum, -45);
    assert_eq!(inputs.counters.realized_cents, 900);
    assert_eq!(inputs.counters.unreal_cents, -120);
    let quantized = alt.load_relaxed().quantized();
    let expected_d2a = (u32::from(quantized.decision_to_ack_us2) * 2) as u16;
    let expected_a2f = (u32::from(quantized.ack_to_fill_us2) * 2) as u16;
    assert_eq!(inputs.counters.lat_d2a_quantiles[0], expected_d2a);
    assert_eq!(inputs.counters.lat_a2f_quantiles[0], expected_a2f);
    assert_eq!(inputs.symbols[0].sym_id, 42);
    assert_eq!(
        inputs.symbols[0].flags & SYMBOL_FLAG_REDUCE_ONLY,
        SYMBOL_FLAG_REDUCE_ONLY
    );
    assert_eq!(inputs.symbols[0].spread_ticks, 5);
    assert_eq!(inputs.symbols[0].vol_bp_q8_8, 512);
    assert_eq!(inputs.symbols[0].sum_bid_l1_3, 120);
    assert_eq!(inputs.log.now_min_ct, 77);
    assert_eq!(inputs.log.next_lockout_min_ct, 90);
    assert_eq!(inputs.log.next_resume_min_ct, 120);
    let expected_summary = {
        let snapshot = ApmSnapshot::unpack(&apm_words);
        let breaker = (snapshot.header.portfolio_breaker.as_u8() as u32) & APM_SUMMARY_BREAKER_MASK;
        let symbol_count = (snapshot.header.symbol_count as u32) & APM_SUMMARY_SYMBOL_MASK;
        let flags = (snapshot.header.portfolio_flags.bits() as u32) & APM_SUMMARY_FLAGS_MASK;
        let headroom = (snapshot.header.rem_daily_loss_total_cents
            / APM_SUMMARY_HEADROOM_SCALE_CENTS)
            .min(u16::MAX as u32);
        (headroom << APM_SUMMARY_HEADROOM_SHIFT)
            | (flags << APM_SUMMARY_FLAGS_SHIFT)
            | (symbol_count << APM_SUMMARY_SYMBOL_SHIFT)
            | (breaker << APM_SUMMARY_BREAKER_SHIFT)
    };
    assert_eq!(inputs.log.apm_summary, expected_summary);

    let apc_snapshot = position.load().expect("position snapshot");
    assert_eq!(
        inputs.header.capsule_digests[0],
        hash_u64(breaker.load_relaxed())
    );
    assert_eq!(
        inputs.header.capsule_digests[1],
        hash_u128(act_slot.load_relaxed().raw())
    );
    assert_eq!(
        inputs.header.capsule_digests[2],
        hash_u128(risk_envelope.load(Ordering::Relaxed).bits())
    );
    assert_eq!(
        inputs.header.capsule_digests[3],
        hash_words(&apc_snapshot.words())
    );
    assert_eq!(
        inputs.header.capsule_digests[4],
        hash_words(apm_slot.load_relaxed().unwrap().as_words())
    );
    assert_eq!(
        inputs.header.capsule_digests[5],
        hash_words(pex_capsule.load_snapshot().unwrap().words())
    );
    assert_eq!(
        inputs.header.capsule_digests[6],
        hash_words(&aeb.load().unwrap().words())
    );
    assert_eq!(
        inputs.header.capsule_digests[7],
        hash_words(&rlt.into_words())
    );
}

#[test]
fn publish_from_feeds_commits_tile() {
    use atomic_latency_ticket::{AltAtomic, AltSample};
    use atomic_position_capsule::{
        AtomicPositionCapsule, EquityWord, PositionHeadWord, SessionWord, TailWord,
    };
    use atomic_venue_snapshot::{Avs128, Avs128Snapshot};
    use std::path::PathBuf;

    let position = AtomicPositionCapsule::new();
    let avs = Avs128::default();
    let alt = AltAtomic::new();

    position.publish(
        PositionHeadWord {
            position_qty: 3,
            avg_px_ticks: 10,
            remaining_daily_loss_cents: 800,
            flags: 0,
        },
        EquityWord {
            realized_cents: 200,
            unrealized_cents: 50,
            peak_equity_cents: 400,
            trailing_draw_cents: 100,
        },
        SessionWord {
            now_min_ct: 10,
            forbid_after_min_ct: 20,
            eod_flat_min_ct: 30,
            open_since_ms: 0,
            max_open_ms: 0,
            max_contracts: 0,
            max_per_trade_cents: 0,
            risk_flags: 0,
            reserved_bits: 0,
        },
        TailWord {
            symbol_id: 99,
            account_id: 7,
            last_exec_id: 1,
            breaker_level: 1,
            alt_health: 0,
            violation_bits: 0,
        },
    );

    avs.publish(Avs128Snapshot {
        spread_ticks: 1,
        obi_q1_10: 10,
        micro_off_ticks: 0,
        sum_bid_l1_3: 50,
        sum_ask_l1_3: 40,
        vol_bp_q8_8: 256,
        sweep_flag: false,
        trend_200ms_ticks: 0,
        ts_coarse_ms: 0,
        version: 1,
        sequence: 1,
    });

    alt.publish_sample(AltSample {
        feed_to_decision_us: 50,
        decision_to_ack_us: 60,
        ack_to_first_fill_us: 70,
        reject_rate_bps: 5,
        cancel_rate_bps: 8,
        loss_rate_bps: 2,
        jitter_us: 20,
        queue_position: 0.5,
        flags: 0,
        version: 1,
        sequence: 1,
        age_ms: 4,
    });

    let feeds = LiveFeeds {
        position: &position,
        venue: Some(&avs),
        latency: Some(&alt),
        breaker: None,
        cost_tracker: None,
        risk_envelope: None,
        portfolio_map: None,
        pre_execution: None,
        execution_bundle: None,
        risk_ladder: None,
        router_metrics: None,
    };

    let meta = SessionMetadata {
        epoch_id: 1,
        created_ms: 1_700_123_456,
        run_id: 0x55AA,
        policy_id: 4,
        tz_id: 5,
        tile_index: 0,
        ale_tail_hash: 0xAABBCCDD,
        prev_tile_hash: [0; 16],
        capsule_digests: [0; 8],
    };

    let counters = CountersInputs::default();
    let log = LogInputs::default();

    let dir = tempfile::tempdir().expect("tmp");
    let mut path = PathBuf::from(dir.path());
    path.push("ring.bin");

    let mut ring = TileRing::create(&path, 2).expect("ring");
    let mut publisher = TilePublisher::new(None);
    let mut shadow = TileShadow::new();

    let outcome = publish_from_feeds(
        &mut ring,
        &mut publisher,
        &mut shadow,
        &feeds,
        meta,
        counters,
        log,
        Some(FlushStrategy::Async),
    )
    .expect("publish")
    .expect("outcome");

    let mapping = TileRingMapping::open(&path, 2).expect("map");
    let tile = &mapping.tiles()[outcome.tile_index as usize];
    validate_tile(tile).expect("valid tile");
    assert_eq!(tile.header.account_id, 7);
    assert_eq!(tile.symbols.slots[0].sym_id, 99);
    assert_eq!(tile.log.tail.apm_summary, 0);
}
