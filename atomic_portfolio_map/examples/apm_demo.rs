use atomic_portfolio_map::{
    ActEdge, ApcSnapshot, ApmSlot, AvsSnapshot, BreakerLevel, FeedSnapshot, PortfolioFlags,
    PortfolioInputs, PortfolioMapWriter, SymbolGates, SymbolInputs, SymbolPolicy,
    build_symbol_inputs,
};

fn main() {
    // Stage 1: account is within limits and MES can scale.
    let stage1_feeds = vec![
        FeedSnapshot {
            policy: SymbolPolicy {
                sym_id: 1,
                max_abs_position: 10,
                forbid_after_min_ct: Some(915),
                eod_flat_min_ct: Some(920),
                priority_offset: 0,
            },
            apc: ApcSnapshot {
                position: 3,
                unreal_cents: 18_500,
                realized_cents: 12_000,
                rem_daily_loss_cents: 300_000,
                breaker_level: BreakerLevel::L0,
            },
            act: Some(ActEdge {
                edge_surplus_bp: 10,
            }),
            avs: Some(AvsSnapshot {
                spread_ticks: 2,
                vol_band: 1,
            }),
            gates: SymbolGates::default(),
        },
        FeedSnapshot {
            policy: SymbolPolicy {
                sym_id: 2,
                max_abs_position: 6,
                forbid_after_min_ct: Some(915),
                eod_flat_min_ct: Some(920),
                priority_offset: 0,
            },
            apc: ApcSnapshot {
                position: -1,
                unreal_cents: -7_500,
                realized_cents: 5_000,
                rem_daily_loss_cents: 50_000,
                breaker_level: BreakerLevel::L1,
            },
            act: Some(ActEdge {
                edge_surplus_bp: -4,
            }),
            avs: Some(AvsSnapshot {
                spread_ticks: 4,
                vol_band: 2,
            }),
            gates: SymbolGates::default(),
        },
    ];

    let stage1_symbols: Vec<SymbolInputs> = stage1_feeds.iter().map(build_symbol_inputs).collect();

    let stage1_inputs = PortfolioInputs {
        account_id: 555,
        forbid_after_min_ct: 915,
        eod_flat_min_ct: 920,
        rem_daily_loss_total_cents: 900_000,
        trailing_draw_cents: 60_000,
        base_realized_cents: 40_000,
        created_ms_coarse: 41_200,
        portfolio_flags: PortfolioFlags::empty(),
        now_minute_count: 910,
        symbols: &stage1_symbols,
    };

    let mut writer = PortfolioMapWriter::new(ApmSlot::new());
    let first_publish = writer.publish(&stage1_inputs);

    println!(
        "[Stage 1] version={} seq={} rem_daily_loss={}",
        first_publish.snapshot.header.version,
        first_publish.snapshot.header.seq,
        first_publish.snapshot.header.rem_daily_loss_total_cents
    );

    for (idx, slice) in first_publish
        .snapshot
        .slices
        .iter()
        .enumerate()
        .take(first_publish.snapshot.header.symbol_count as usize)
    {
        println!(
            "  slice {} -> sym={} qty={} flags={:#04x} priority={}",
            idx,
            slice.sym_id,
            slice.pos_qty,
            slice.flags.bits(),
            slice.priority
        );
    }

    // Demonstrate how a stale marker evicts readers until a new publish arrives.
    writer.mark_stale();
    if writer.slot().load_relaxed().is_none() {
        println!("slot marked stale; readers will wait for a fresh publish");
    }

    // Stage 2: after forbid window hits and rem headroom exhausted for symbol 2.
    let stage2_feeds = vec![
        FeedSnapshot {
            act: stage1_feeds[0].act.clone(),
            avs: stage1_feeds[0].avs.clone(),
            gates: SymbolGates {
                force_reduce_only: false,
                ..stage1_feeds[0].gates.clone()
            },
            apc: ApcSnapshot {
                rem_daily_loss_cents: 180_000,
                ..stage1_feeds[0].apc.clone()
            },
            policy: stage1_feeds[0].policy.clone(),
        },
        FeedSnapshot {
            act: stage1_feeds[1].act.clone(),
            avs: stage1_feeds[1].avs.clone(),
            gates: SymbolGates {
                news_lockout: true,
                force_reduce_only: true,
                ..stage1_feeds[1].gates.clone()
            },
            apc: ApcSnapshot {
                rem_daily_loss_cents: 0,
                ..stage1_feeds[1].apc.clone()
            },
            policy: stage1_feeds[1].policy.clone(),
        },
    ];

    let stage2_symbols: Vec<SymbolInputs> = stage2_feeds.iter().map(build_symbol_inputs).collect();

    let stage2_inputs = PortfolioInputs {
        now_minute_count: 925,
        portfolio_flags: PortfolioFlags::empty(),
        symbols: &stage2_symbols,
        ..stage1_inputs
    };

    let second_publish = writer.publish(&stage2_inputs);

    println!(
        "[Stage 2] version={} seq={} rem_daily_loss={}",
        second_publish.snapshot.header.version,
        second_publish.snapshot.header.seq,
        second_publish.snapshot.header.rem_daily_loss_total_cents
    );

    for (idx, slice) in second_publish
        .snapshot
        .slices
        .iter()
        .enumerate()
        .take(second_publish.snapshot.header.symbol_count as usize)
    {
        println!(
            "  slice {} -> sym={} qty={} flags={:#04x} priority={}",
            idx,
            slice.sym_id,
            slice.pos_qty,
            slice.flags.bits(),
            slice.priority
        );
    }

    if let Some(words) = writer.slot().load_relaxed() {
        for (word_idx, raw) in words.as_words().iter().enumerate() {
            println!("word{}=0x{:032x}", word_idx, raw);
        }
    }
}
