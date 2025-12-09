use kindly_engine::replay::{decode_events, doctrine_series, supply_series, ReplayEvent};
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "data/kindly-engine/replay.bin".to_string()),
    );
    let mut buf = Vec::new();
    File::open(&path)?.read_to_end(&mut buf)?;
    if buf.len() % 16 != 0 {
        eprintln!(
            "warning: replay file length {} is not a multiple of 16 bytes; extra bytes ignored",
            buf.len()
        );
    }

    let mut events = Vec::new();
    for chunk in buf.chunks_exact(16) {
        let mut tick_bytes = [0u8; 8];
        tick_bytes.copy_from_slice(&chunk[..8]);
        let mut payload_bytes = [0u8; 8];
        payload_bytes.copy_from_slice(&chunk[8..16]);
        events.push(ReplayEvent::new(
            u64::from_le_bytes(tick_bytes),
            u64::from_le_bytes(payload_bytes),
        ));
    }

    let decoded = decode_events(&events);
    let series = supply_series(&decoded);
    let doctrine = doctrine_series(&decoded);

    if series.is_empty() {
        println!(
            "no supply/fatigue entries found in {:?} (check that supply payload tag 0xC200 is present)",
            path
        );
    } else {
        println!("Supply/fatigue timeline from {:?}:", path);
        for (tick, pressure_q16, fatigue_q16) in series {
            let pressure = pressure_q16 as f32 / 65_536.0;
            let fatigue = fatigue_q16 as f32 / 65_536.0;
            println!(
                "tick {:>8}: pressure_avg {:.3}, fatigue_pen {:.3}",
                tick, pressure, fatigue
            );
        }
    }

    if doctrine.is_empty() {
        println!(
            "no doctrine/rank-fire entries found in {:?} (check that doctrine payload tag 0xC600 is present)",
            path
        );
    } else {
        println!("\nDoctrine/rank-fire timeline from {:?}:", path);
        for (tick, mask, mode, cadence, rank_events, advance_events) in doctrine {
            println!(
                "tick {:>8}: mask 0x{mask:02X}, mode {mode}, cadence {} ticks, rank_fire {}, advance_fire {}",
                tick, cadence, rank_events, advance_events
            );
        }
    }

    Ok(())
}
