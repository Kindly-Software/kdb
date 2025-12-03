//! Decode a replay log and print StratOps-focused summaries (strategic events, command delays).
//! Usage: `cargo run -p kindly-engine --example stratops_dump -- <replay.bin>`

use kindly_engine::replay::{
    build_stratops_lane, decode_replay_payload, ReplayEvent, StratOpsRecord,
};
use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let path = env::args()
        .nth(1)
        .expect("usage: stratops_dump <replay.bin>");
    let events = load_replay_events(Path::new(&path))?;
    let decoded: Vec<(u64, kindly_engine::replay::ReplayRecord)> = events
        .iter()
        .map(|ev| (ev.tick, decode_replay_payload(ev.payload)))
        .collect();
    let lane = build_stratops_lane(&decoded);
    if lane.is_empty() {
        println!("no stratops records found");
        return Ok(());
    }

    let mut strat_count = 0;
    let mut delay_applied = 0;
    let mut delay_hist = 0;
    let mut eta_hist = 0;

    for rec in &lane {
        match rec {
            StratOpsRecord::Strategic { .. } => strat_count += 1,
            StratOpsRecord::CommandDelayApplied { .. } => delay_applied += 1,
            StratOpsRecord::CommandDelayHist { .. } => delay_hist += 1,
            StratOpsRecord::CourierEtaHist { .. } => eta_hist += 1,
        }
    }

    println!("StratOps summary for {path}:");
    println!("  strategic events        : {}", strat_count);
    println!("  cmd delay applied       : {}", delay_applied);
    println!("  cmd delay hist chunks   : {}", delay_hist);
    println!("  courier ETA hist chunks : {}", eta_hist);
    println!();

    println!("Recent sample (up to 12 entries):");
    for rec in lane.iter().rev().take(12).rev() {
        match rec {
            StratOpsRecord::Strategic {
                tick,
                kind,
                province_id,
                primary,
                secondary,
            } => {
                println!(
                    "  tick {} strategic {:?} province={} primary={} secondary={}",
                    tick, kind, province_id, primary, secondary
                );
            }
            StratOpsRecord::CommandDelayApplied {
                tick,
                count,
                avg_delay_ticks,
            } => println!(
                "  tick {} cmd_delay applied={} avg_delay_ticks={}",
                tick, count, avg_delay_ticks
            ),
            StratOpsRecord::CommandDelayHist {
                tick,
                chunk,
                buckets,
            } => println!(
                "  tick {} cmd_delay_hist chunk={} buckets={:?}",
                tick, chunk, buckets
            ),
            StratOpsRecord::CourierEtaHist {
                tick,
                chunk,
                buckets,
            } => println!(
                "  tick {} courier_eta_hist chunk={} buckets={:?}",
                tick, chunk, buckets
            ),
        }
    }

    Ok(())
}

fn load_replay_events(path: &Path) -> Result<Vec<ReplayEvent>, Box<dyn Error>> {
    let data = fs::read(path)?;
    if data.len() % 16 != 0 {
        eprintln!(
            "warning: replay length {} not divisible by 16 bytes; trailing bytes ignored",
            data.len()
        );
    }
    let mut out = Vec::new();
    for chunk in data.chunks(16) {
        if chunk.len() < 16 {
            break;
        }
        let tick = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
        let payload = u64::from_le_bytes(chunk[8..16].try_into().unwrap());
        out.push(ReplayEvent::new(tick, payload));
    }
    Ok(out)
}
