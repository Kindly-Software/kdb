use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use atomic_portfolio_map::{
    AccountSnapshot, ActEdge, ApcSnapshot, ApmSlot, AvsSnapshot, BreakerLevel, FeedAssembler,
    PortfolioController, PortfolioFlags, PortfolioMapWriter, PortfolioRuntime, SharedActFeed,
    SharedApcFeed, SharedAvsFeed, SymbolGates, SymbolPolicy,
};

fn main() {
    let apc_feed = SharedApcFeed::new();
    let act_feed = SharedActFeed::new();
    let avs_feed = SharedAvsFeed::new();

    let policies = vec![
        SymbolPolicy {
            sym_id: 1,
            max_abs_position: 8,
            forbid_after_min_ct: Some(910),
            eod_flat_min_ct: Some(920),
            priority_offset: 0,
        },
        SymbolPolicy {
            sym_id: 2,
            max_abs_position: 6,
            forbid_after_min_ct: Some(910),
            eod_flat_min_ct: Some(920),
            priority_offset: 0,
        },
    ];

    let assembler = FeedAssembler::new(
        Arc::new(apc_feed.clone()),
        Some(Arc::new(act_feed.clone())),
        Some(Arc::new(avs_feed.clone())),
    );

    let slot = ApmSlot::new();
    let writer = PortfolioMapWriter::new(slot);
    let mut runtime = PortfolioRuntime::new(
        PortfolioController::new(writer, Duration::from_millis(2_000)),
        assembler,
        policies,
    );

    let account = AccountSnapshot {
        account_id: 1_234,
        forbid_after_min_ct: 910,
        eod_flat_min_ct: 920,
        rem_daily_loss_total_cents: 1_000_000,
        trailing_draw_cents: 50_000,
        base_realized_cents: 250_000,
        created_ms_coarse: 41_000,
        portfolio_flags: PortfolioFlags::empty(),
    };

    apc_feed.insert(
        1,
        ApcSnapshot {
            position: 0,
            unreal_cents: 0,
            realized_cents: 10_000,
            rem_daily_loss_cents: 200_000,
            breaker_level: BreakerLevel::L0,
        },
    );
    apc_feed.insert(
        2,
        ApcSnapshot {
            position: 0,
            unreal_cents: 0,
            realized_cents: 8_000,
            rem_daily_loss_cents: 180_000,
            breaker_level: BreakerLevel::L1,
        },
    );
    act_feed.insert(1, ActEdge { edge_surplus_bp: 8 });
    act_feed.insert(2, ActEdge { edge_surplus_bp: 2 });
    avs_feed.insert(
        1,
        AvsSnapshot {
            spread_ticks: 2,
            vol_band: 1,
        },
    );
    avs_feed.insert(
        2,
        AvsSnapshot {
            spread_ticks: 3,
            vol_band: 2,
        },
    );

    let apc_feed_update = apc_feed.clone();
    let act_feed_update = act_feed.clone();
    let avs_feed_update = avs_feed.clone();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag = stop.clone();

    // Background thread simulating external feed updates.
    thread::spawn(move || {
        let mut last_edge = 5;
        let start = Instant::now();
        loop {
            if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            let elapsed_ms = start.elapsed().as_millis() as u64;
            let position = ((elapsed_ms / 500) % 4) as i32;

            apc_feed_update.insert(
                1,
                ApcSnapshot {
                    position,
                    unreal_cents: position * 5_000,
                    realized_cents: 10_000,
                    rem_daily_loss_cents: 200_000,
                    breaker_level: BreakerLevel::L0,
                },
            );
            apc_feed_update.insert(
                2,
                ApcSnapshot {
                    position: -position,
                    unreal_cents: -position * 4_000,
                    realized_cents: 8_000,
                    rem_daily_loss_cents: 180_000,
                    breaker_level: BreakerLevel::L1,
                },
            );

            last_edge = ((last_edge + 1) % 20).max(6);
            act_feed_update.insert(
                1,
                ActEdge {
                    edge_surplus_bp: last_edge,
                },
            );
            act_feed_update.insert(
                2,
                ActEdge {
                    edge_surplus_bp: last_edge - 6,
                },
            );

            avs_feed_update.insert(
                1,
                AvsSnapshot {
                    spread_ticks: 2,
                    vol_band: 1,
                },
            );
            avs_feed_update.insert(
                2,
                AvsSnapshot {
                    spread_ticks: 3,
                    vol_band: 2,
                },
            );

            thread::sleep(Duration::from_millis(200));
        }
    });

    let start = Instant::now();
    for _ in 0..15 {
        let now_ms = start.elapsed().as_millis() as u64;
        if let Some(result) =
            runtime.publish_cycle(&account, 905, now_ms, |_| SymbolGates::default())
        {
            println!(
                "seq={} version={} net_realized={} priority_sym1={}",
                result.snapshot.header.seq,
                result.snapshot.header.version,
                result.snapshot.tail.net_realized_cents,
                result.snapshot.slices[0].priority
            );
        } else {
            println!("publish skipped (missing feed)");
        }

        thread::sleep(Duration::from_millis(250));
        runtime.tick(start.elapsed().as_millis() as u64);
    }

    stop.store(true, std::sync::atomic::Ordering::Relaxed);

    println!(
        "final slot commit={} stale={}",
        atomic_portfolio_map::ApmHeader::decode(
            runtime.controller().writer().slot().raw_words()[0]
        )
        .commit,
        atomic_portfolio_map::ApmHeader::decode(
            runtime.controller().writer().slot().raw_words()[0]
        )
        .stale
    );
}
