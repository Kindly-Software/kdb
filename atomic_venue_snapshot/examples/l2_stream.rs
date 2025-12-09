//! Run with `cargo run --example l2_stream --features std`.

#[cfg(not(feature = "std"))]
fn main() {
    eprintln!("Enable the `std` feature to run this example.");
}

#[cfg(feature = "std")]
fn main() {
    use atomic_venue_snapshot::{
        layout::{decode_vol_bp_q8_8, obi_to_ratio},
        AvsWriter, WriterConfig, WriterInput,
    };

    #[derive(Clone, Debug)]
    struct L2Event {
        timestamp_ms: u64,
        bid_px_ticks: i64,
        ask_px_ticks: i64,
        bid_levels: [u32; 3],
        ask_levels: [u32; 3],
        marketable_volume: u32,
    }

    impl From<L2Event> for WriterInput {
        fn from(event: L2Event) -> Self {
            WriterInput::new(
                event.timestamp_ms,
                event.bid_px_ticks,
                event.ask_px_ticks,
                event.bid_levels,
                event.ask_levels,
                event.marketable_volume,
            )
        }
    }

    let mut writer = AvsWriter::new(WriterConfig {
        version: 1,
        bp_per_tick: 1.25,
        ..WriterConfig::default()
    });

    let events = vec![
        L2Event {
            timestamp_ms: 1_000,
            bid_px_ticks: 100_000,
            ask_px_ticks: 100_100,
            bid_levels: [120, 90, 70],
            ask_levels: [110, 95, 60],
            marketable_volume: 0,
        },
        L2Event {
            timestamp_ms: 1_070,
            bid_px_ticks: 100_000,
            ask_px_ticks: 100_100,
            bid_levels: [95, 80, 65],
            ask_levels: [120, 105, 75],
            marketable_volume: 12,
        },
        L2Event {
            timestamp_ms: 1_110,
            bid_px_ticks: 99_900,
            ask_px_ticks: 100_040,
            bid_levels: [40, 38, 35],
            ask_levels: [150, 130, 110],
            marketable_volume: 90,
        },
    ];

    for event in events {
        let snapshot = writer.publish(event.into());
        println!(
            "t={}ms spread={} ticks obi={:.3} micro={} trend={} vol={:.3}bp sweep={} seq={}",
            snapshot.ts_coarse_ms << 2,
            snapshot.spread_ticks,
            obi_to_ratio(snapshot.obi_q1_10),
            snapshot.micro_off_ticks,
            snapshot.trend_200ms_ticks,
            decode_vol_bp_q8_8(snapshot.vol_bp_q8_8),
            snapshot.sweep_flag,
            snapshot.sequence
        );
    }

    let latest = writer.capsule().load_relaxed().unpack();
    if latest.is_stale(1_400, 250) {
        println!("snapshot stale -> trading gate closed");
    } else {
        println!("latest seq {} is fresh", latest.sequence);
    }
}
