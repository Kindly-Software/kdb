use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use atomic_epoch_tile::{scan_latest_committed, validate_tile, TileRingMapping};

#[derive(Parser, Debug)]
#[command(about = "Verify integrity of all ET tiles in a ring", version)]
struct Args {
    /// Path to the memory-mapped tile ring.
    path: PathBuf,

    /// Total number of tiles in the ring.
    #[arg(short = 'n', long, default_value_t = 64)]
    tiles: usize,

    /// Emit details about every tile.
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mapping = TileRingMapping::open(&args.path, args.tiles)
        .with_context(|| format!("failed to open tile ring at {}", args.path.display()))?;
    let tiles = mapping.tiles();
    if tiles.is_empty() {
        anyhow::bail!("ring contains zero tiles");
    }

    let mut valid = 0usize;
    let mut invalid = 0usize;

    for (idx, tile) in tiles.iter().enumerate() {
        match validate_tile(tile) {
            Ok(()) => {
                valid += 1;
                if args.verbose {
                    println!(
                        "ok   tile={} epoch={} seq={} created_ms={}",
                        idx, tile.header.epoch_id, tile.header.seq_head, tile.header.created_ms
                    );
                }
            }
            Err(err) => {
                invalid += 1;
                println!("fail tile={} err={}", idx, err);
            }
        }
    }

    println!(
        "summary: valid={} invalid={} total={}",
        valid,
        invalid,
        tiles.len()
    );

    if invalid > 0 {
        anyhow::bail!("ring contains invalid tiles");
    }

    if let Some((idx, tile)) = scan_latest_committed(tiles, tiles.len() - 1) {
        println!(
            "latest: index={} epoch={} created_ms={} seq={}",
            idx, tile.header.epoch_id, tile.header.created_ms, tile.header.seq_head
        );
    } else {
        println!("latest: none committed");
    }

    Ok(())
}
