//! Replay a CSV of L2 snapshots into `AvsWriter` for offline calibration.
//!
//! ```text
//! cargo run --example replay_csv --features std -- data/l2_sample.csv 1.15 250
//! ```
//!
//! Columns expected in the CSV (header row is required):
//! `timestamp_ms,bid_px_ticks,ask_px_ticks,bid1,bid2,bid3,ask1,ask2,ask3,marketable_volume`
//!
#[cfg(feature = "std")]
use atomic_venue_snapshot::{
    layout::{decode_vol_bp_q8_8, obi_to_ratio},
    Avs128Snapshot, SnapshotStats, WriterConfig, WriterInput,
};
#[cfg(feature = "std")]
use serde_json::{json, Value};

#[cfg(not(feature = "std"))]
fn main() {
    eprintln!("Enable the `std` feature to run this example.");
}

#[cfg(feature = "std")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, fs};

    use atomic_venue_snapshot::{
        analysis::SnapshotStatsBuilder, AvsWriter, WriterConfig, WriterInput,
    };
    use csv::ReaderBuilder;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Record {
        timestamp_ms: u64,
        bid_px_ticks: i64,
        ask_px_ticks: i64,
        bid1: u32,
        bid2: u32,
        bid3: u32,
        ask1: u32,
        ask2: u32,
        ask3: u32,
        #[serde(default)]
        marketable_volume: u32,
    }

    impl Record {
        fn into_input(self) -> WriterInput {
            let bids = [self.bid1, self.bid2, self.bid3];
            let asks = [self.ask1, self.ask2, self.ask3];
            WriterInput::from_depth_slices(
                self.timestamp_ms,
                self.bid_px_ticks,
                self.ask_px_ticks,
                &bids,
                &asks,
                self.marketable_volume,
            )
        }
    }

    #[derive(Debug, Deserialize)]
    struct ConfigOverrides {
        version: Option<u8>,
        vol_alpha: Option<f64>,
        bp_per_tick: Option<f64>,
        trend_window_ms: Option<u64>,
        sweep_mid_jump_ticks: Option<i64>,
        sweep_window_ms: Option<u64>,
        sweep_hold_ms: Option<u64>,
        sweep_collapse_ratio: Option<f64>,
        sweep_volume_factor: Option<f64>,
    }

    let mut args: Vec<String> = env::args().skip(1).collect();
    let mut json_mode = false;
    let mut config_path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut bucket_ms: Option<u64> = None;

    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--json" => {
                json_mode = true;
                args.remove(idx);
            }
            "--config" => {
                let path = args
                    .get(idx + 1)
                    .expect("`--config` expects a following file path")
                    .clone();
                args.remove(idx + 1);
                args.remove(idx);
                config_path = Some(path);
            }
            "--output" => {
                let path = args
                    .get(idx + 1)
                    .expect("`--output` expects a following file path")
                    .clone();
                args.remove(idx + 1);
                args.remove(idx);
                output_path = Some(path);
            }
            "--bucket-ms" => {
                let value = args
                    .get(idx + 1)
                    .expect("`--bucket-ms` expects a following integer")
                    .parse::<u64>()
                    .expect("bucket size must be an integer");
                args.remove(idx + 1);
                args.remove(idx);
                bucket_ms = Some(value.max(1));
            }
            _ => idx += 1,
        }
    }

    let path = args
        .get(0)
        .map(|s| s.as_str())
        .expect("usage: cargo run --example replay_csv --features std -- [--json] <csv> [bp_per_tick] [stale_budget_ms]");
    let cli_bp = args
        .get(1)
        .map(|s| s.parse::<f64>().expect("bp_per_tick must be a float"));
    let stale_budget = args
        .get(2)
        .map(|s| s.parse::<u64>().expect("stale budget must be ms"))
        .unwrap_or(250);

    let overrides = config_path
        .as_deref()
        .map(
            |path| -> Result<ConfigOverrides, Box<dyn std::error::Error>> {
                let text = fs::read_to_string(path)?;
                Ok(serde_json::from_str(&text)?)
            },
        )
        .transpose()?;

    let mut reader = ReaderBuilder::new().has_headers(true).from_path(path)?;

    let mut config = WriterConfig::default();
    if let Some(over) = &overrides {
        if let Some(v) = over.version {
            config.version = v;
        }
        if let Some(v) = over.vol_alpha {
            config.vol_alpha = v;
        }
        if let Some(v) = over.bp_per_tick {
            config.bp_per_tick = v;
        }
        if let Some(v) = over.trend_window_ms {
            config.trend_window_ms = v;
        }
        if let Some(v) = over.sweep_mid_jump_ticks {
            config.sweep_mid_jump_ticks = v;
        }
        if let Some(v) = over.sweep_window_ms {
            config.sweep_window_ms = v;
        }
        if let Some(v) = over.sweep_hold_ms {
            config.sweep_hold_ms = v;
        }
        if let Some(v) = over.sweep_collapse_ratio {
            config.sweep_collapse_ratio = v;
        }
        if let Some(v) = over.sweep_volume_factor {
            config.sweep_volume_factor = v;
        }
    }
    if let Some(bp) = cli_bp {
        config.bp_per_tick = bp;
    }
    if config.bp_per_tick == 0.0 {
        config.bp_per_tick = 1.0;
    }
    let config = config.normalised();

    let mut writer = AvsWriter::new(config);

    let mut stats_builder = SnapshotStatsBuilder::default();
    let mut bucket_builder = bucket_ms.map(|_| SnapshotStatsBuilder::default());
    let mut current_bucket_start: Option<u64> = None;
    let mut buckets: Vec<BucketSummary> = Vec::new();
    let mut last_entry = None;

    for record in reader.deserialize::<Record>() {
        let input = record?.into_input();
        let snapshot = writer.publish(input);
        stats_builder.observe(&snapshot, input.timestamp_ms, stale_budget);
        if let Some(size) = bucket_ms {
            let bucket_start = (input.timestamp_ms / size) * size;
            if current_bucket_start != Some(bucket_start) {
                if let (Some(start), Some(builder)) = (current_bucket_start, bucket_builder.take())
                {
                    buckets.push(BucketSummary {
                        start_ms: start,
                        end_ms: start + size,
                        stats: builder.finish(),
                    });
                }
                current_bucket_start = Some(bucket_start);
                bucket_builder = Some(SnapshotStatsBuilder::default());
            }
            if let Some(builder) = &mut bucket_builder {
                builder.observe(&snapshot, input.timestamp_ms, stale_budget);
            }
        }
        last_entry = Some((input, snapshot));
    }

    let stats = stats_builder.finish();
    if let (Some(start), Some(builder), Some(size)) =
        (current_bucket_start, bucket_builder.take(), bucket_ms)
    {
        buckets.push(BucketSummary {
            start_ms: start,
            end_ms: start + size,
            stats: builder.finish(),
        });
    }

    if let Some(path) = output_path.as_deref() {
        let payload = build_payload(config, stats, &buckets, last_entry);
        let file = fs::File::create(path)?;
        serde_json::to_writer_pretty(file, &payload)?;
    }

    if json_mode {
        let payload = build_payload(config, stats, &buckets, last_entry);
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "config: ver={} α={} bp_per_tick={:.3} trend_window_ms={} sweep_jump={} sweep_window_ms={} sweep_hold_ms={} collapse_ratio={:.2} volume_factor={:.2}",
            config.version,
            config.vol_alpha,
            config.bp_per_tick,
            config.trend_window_ms,
            config.sweep_mid_jump_ticks,
            config.sweep_window_ms,
            config.sweep_hold_ms,
            config.sweep_collapse_ratio,
            config.sweep_volume_factor,
        );
        println!(
            "events={} sweeps={} ({:.2}%) stale={} ({:.2}%)",
            stats.events,
            stats.sweeps,
            stats.sweep_ratio() * 100.0,
            stats.stale,
            stats.stale_ratio() * 100.0
        );

        if let Some((_, snapshot)) = last_entry {
            println!(
                "last: spread={} obi={:.3} micro={} trend={} vol={:.3}bp sweep={} seq={}",
                snapshot.spread_ticks,
                obi_to_ratio(snapshot.obi_q1_10),
                snapshot.micro_off_ticks,
                snapshot.trend_200ms_ticks,
                decode_vol_bp_q8_8(snapshot.vol_bp_q8_8),
                snapshot.sweep_flag,
                snapshot.sequence
            );
        }

        println!(
            "max spread={} ticks, max vol={:.3} bp, mean vol={:.3} bp, mean |micro|={:.2} ticks, mean obi={:.3}",
            stats.max_spread,
            stats.max_vol_bp,
            stats.mean_vol_bp,
            stats.mean_abs_micro_ticks,
            stats.mean_obi,
        );
        if !buckets.is_empty() {
            println!("bucket summaries ({} ms):", bucket_ms.unwrap());
            for summary in &buckets {
                println!(
                    "  {}-{} ms: events={} sweeps={:.2}% vol_mean={:.3}bp stale={:.2}%",
                    summary.start_ms,
                    summary.end_ms,
                    summary.stats.events,
                    summary.stats.sweep_ratio() * 100.0,
                    summary.stats.mean_vol_bp,
                    summary.stats.stale_ratio() * 100.0,
                );
            }
        }
    }

    Ok(())
}

#[cfg(feature = "std")]
#[derive(Debug)]
struct BucketSummary {
    start_ms: u64,
    end_ms: u64,
    stats: SnapshotStats,
}

#[cfg(feature = "std")]
fn build_payload(
    config: WriterConfig,
    stats: SnapshotStats,
    buckets: &[BucketSummary],
    last_entry: Option<(WriterInput, Avs128Snapshot)>,
) -> Value {
    let last_json = last_entry.map(|(input, snapshot)| {
        json!({
            "timestamp_ms": input.timestamp_ms,
            "bid_px_ticks": input.bid_px_ticks,
            "ask_px_ticks": input.ask_px_ticks,
            "bid_sizes": input.bid_sizes,
            "ask_sizes": input.ask_sizes,
            "marketable_volume": input.marketable_volume,
            "snapshot": {
                "spread_ticks": snapshot.spread_ticks,
                "obi": obi_to_ratio(snapshot.obi_q1_10),
                "micro_off_ticks": snapshot.micro_off_ticks,
                "trend_ticks": snapshot.trend_200ms_ticks,
                "vol_bp": decode_vol_bp_q8_8(snapshot.vol_bp_q8_8),
                "sweep": snapshot.sweep_flag,
                "sequence": snapshot.sequence,
            }
        })
    });

    json!({
        "config": {
            "version": config.version,
            "vol_alpha": config.vol_alpha,
            "bp_per_tick": config.bp_per_tick,
            "trend_window_ms": config.trend_window_ms,
            "sweep_mid_jump_ticks": config.sweep_mid_jump_ticks,
            "sweep_window_ms": config.sweep_window_ms,
            "sweep_hold_ms": config.sweep_hold_ms,
            "sweep_collapse_ratio": config.sweep_collapse_ratio,
            "sweep_volume_factor": config.sweep_volume_factor,
        },
        "statistics": {
            "events": stats.events,
            "sweeps": stats.sweeps,
            "sweep_ratio": stats.sweep_ratio(),
            "stale": stats.stale,
            "stale_ratio": stats.stale_ratio(),
            "max_spread_ticks": stats.max_spread,
            "max_vol_bp": stats.max_vol_bp,
            "mean_vol_bp": stats.mean_vol_bp,
            "mean_abs_micro_ticks": stats.mean_abs_micro_ticks,
            "mean_obi": stats.mean_obi,
        },
        "last": last_json,
        "buckets": buckets
            .iter()
            .map(|bucket| {
                json!({
                    "start_ms": bucket.start_ms,
                    "end_ms": bucket.end_ms,
                    "statistics": {
                        "events": bucket.stats.events,
                        "sweeps": bucket.stats.sweeps,
                        "sweep_ratio": bucket.stats.sweep_ratio(),
                        "stale": bucket.stats.stale,
                        "stale_ratio": bucket.stats.stale_ratio(),
                        "max_spread_ticks": bucket.stats.max_spread,
                        "max_vol_bp": bucket.stats.max_vol_bp,
                        "mean_vol_bp": bucket.stats.mean_vol_bp,
                        "mean_abs_micro_ticks": bucket.stats.mean_abs_micro_ticks,
                        "mean_obi": bucket.stats.mean_obi,
                    }
                })
            })
            .collect::<Vec<_>>(),
    })
}
