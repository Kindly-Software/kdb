#![cfg(feature = "std")]

use atomic_venue_snapshot::{
    layout::{
        decode_vol_bp_q8_8, MICRO_OFF_TICKS_MAX, MICRO_OFF_TICKS_MIN, SPREAD_TICKS_MAX,
        TS_COARSE_MS_GRANULARITY,
    },
    AvsWriter, WriterConfig, WriterInput,
};

fn make_input(
    timestamp_ms: u64,
    bid_px: i64,
    ask_px: i64,
    bid_sizes: [u32; 3],
    ask_sizes: [u32; 3],
    marketable_volume: u32,
) -> WriterInput {
    WriterInput {
        timestamp_ms,
        bid_px_ticks: bid_px,
        ask_px_ticks: ask_px,
        bid_sizes,
        ask_sizes,
        marketable_volume,
    }
}

#[test]
fn writer_computes_fields_and_sets_sweep_flag() {
    let mut writer = AvsWriter::new(WriterConfig {
        version: 7,
        bp_per_tick: 1.2,
        ..WriterConfig::default()
    });

    let first = make_input(1_000, 1_000, 1_002, [100, 80, 60], [120, 70, 50], 0);

    let snapshot = writer.publish(first);
    assert_eq!(snapshot.version, 7);
    assert_eq!(snapshot.sequence, 1);
    assert_eq!(snapshot.spread_ticks, 2);
    assert_eq!(snapshot.micro_off_ticks, 0);
    let coarse_ms = u64::from(snapshot.ts_coarse_ms) * u64::from(TS_COARSE_MS_GRANULARITY);
    assert_eq!(coarse_ms, 1_000 & !u64::from(TS_COARSE_MS_GRANULARITY - 1));
    assert!(decode_vol_bp_q8_8(snapshot.vol_bp_q8_8) <= 0.01);
    assert!(!snapshot.sweep_flag);

    let second = make_input(1_100, 998, 1_000, [20, 10, 10], [200, 150, 100], 500);

    let snapshot = writer.publish(second);
    assert_eq!(snapshot.sequence, 2);
    assert_eq!(
        snapshot.spread_ticks.min(SPREAD_TICKS_MAX),
        snapshot.spread_ticks
    );
    assert!(snapshot.micro_off_ticks >= MICRO_OFF_TICKS_MIN);
    assert!(snapshot.micro_off_ticks <= MICRO_OFF_TICKS_MAX);
    assert!(
        snapshot.sweep_flag,
        "sweep flag should latch after aggressive flow"
    );
    assert!(snapshot.trend_200ms_ticks <= 0);
    assert!(decode_vol_bp_q8_8(snapshot.vol_bp_q8_8) > 0.0);
}
