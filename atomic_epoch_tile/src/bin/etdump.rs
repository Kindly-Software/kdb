use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

use atomic_epoch_tile::{
    scan_latest_committed, validate_tile, CountersSection, EtTile, HeaderSection, SymbolSection,
    TileRingMapping,
};
use serde_json::json;

#[derive(Parser, Debug)]
#[command(about = "Inspect a single ET tile from a ring file", version)]
struct Args {
    /// Path to the memory-mapped tile ring.
    path: PathBuf,

    /// Total number of tiles in the ring.
    #[arg(short = 'n', long, default_value_t = 64)]
    tiles: usize,

    /// Tile index to dump. Defaults to the latest committed tile.
    #[arg(short, long)]
    index: Option<usize>,

    /// Output format.
    #[arg(value_enum, short, long, default_value = "pretty")]
    format: OutputFormat,
}

#[derive(Clone, Debug, ValueEnum)]
enum OutputFormat {
    Pretty,
    Json,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mapping = TileRingMapping::open(&args.path, args.tiles)
        .with_context(|| format!("failed to open tile ring at {}", args.path.display()))?;

    let tiles = mapping.tiles();
    if tiles.is_empty() {
        anyhow::bail!("ring contains zero tiles");
    }

    let (index, tile) = if let Some(index) = args.index {
        let idx = index % tiles.len();
        (idx, &tiles[idx])
    } else {
        scan_latest_committed(tiles, tiles.len() - 1).context("no committed tiles found in ring")?
    };

    validate_tile(tile).context("selected tile is not committed")?;

    match args.format {
        OutputFormat::Pretty => print_tile_pretty(index, tile),
        OutputFormat::Json => print_tile_json(index, tile)?,
    }

    Ok(())
}

fn print_tile_pretty(index: usize, tile: &EtTile) {
    println!("tile_index: {}", index);
    print_header(&tile.header);
    print_counters(&tile.counters);
    print_symbols(&tile.symbols);
    print_log(tile);
}

fn print_tile_json(index: usize, tile: &EtTile) -> Result<()> {
    let symbols: Vec<_> = tile
        .symbols
        .slots
        .iter()
        .filter(|slot| slot.sym_id != 0)
        .map(|slot| {
            json!({
                "sym_id": slot.sym_id,
                "breaker_level": slot.breaker_level,
                "flags": slot.flags,
                "pos_qty": slot.pos_qty,
                "avg_px_ticks": slot.avg_px_ticks,
                "realized_cents": slot.realized_cents,
                "unreal_cents": slot.unreal_cents,
            })
        })
        .collect();

    let entries: Vec<_> = tile
        .log
        .entries
        .iter()
        .filter(|entry| {
            !(entry.ts_ms == 0 && entry.event == 0 && entry.actor == 0 && entry.sym_id == 0)
        })
        .map(|entry| {
            json!({
                "ts_ms": entry.ts_ms,
                "event": entry.event,
                "actor": entry.actor,
                "sym_id": entry.sym_id,
                "code": entry.code,
                "aux": entry.aux,
                "flags": entry.flags,
            })
        })
        .collect();

    let payload = json!({
        "index": index,
        "header": {
            "epoch_id": tile.header.epoch_id,
            "created_ms": tile.header.created_ms,
            "run_id": format!("{:032x}", tile.header.run_id),
            "policy_id": tile.header.policy_id,
            "account_id": tile.header.account_id,
            "prev_tile_hash": tile.header.prev_tile_hash,
            "ale_tail_hash": tile.header.ale_tail_hash,
            "commit": tile.header.commit,
            "ver_even": tile.header.ver_even,
            "seq_head": tile.header.seq_head,
            "capsule_digests": tile.header.capsule_digests,
        },
        "counters": {
            "orders_sent": tile.counters.orders_sent,
            "acks": tile.counters.acks,
            "fills": tile.counters.fills,
            "cancels": tile.counters.cancels,
            "rejects": tile.counters.rejects,
            "realized_cents": tile.counters.realized_cents,
            "unreal_cents": tile.counters.unreal_cents,
            "fees_cents": tile.counters.fees_cents,
            "peak_equity_cents": tile.counters.peak_equity_cents,
            "max_draw_cents": tile.counters.max_draw_cents,
            "lat_hist8": tile.counters.lat_hist8,
            "slip_hist8": tile.counters.slip_hist8,
        },
        "symbols": symbols,
        "mini_log": {
            "head": tile.log.tail.mini_head,
            "count": tile.log.tail.mini_count,
            "entries": entries,
        }
    });

    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn print_header(header: &HeaderSection) {
    println!("[header]");
    println!("  created_ms   : {}", header.created_ms);
    println!("  epoch_id     : {}", header.epoch_id);
    println!("  run_id       : {:032x}", header.run_id);
    println!("  policy_id    : {}", header.policy_id);
    println!("  account_id   : {}", header.account_id);
    println!("  prev_hash    : {:02x?}", header.prev_tile_hash);
    println!("  ale_tail_hash: {:016x}", header.ale_tail_hash);
    println!("  commit       : {}", header.commit);
    println!("  ver_even     : {}", header.ver_even);
    println!("  seq_head     : {}", header.seq_head);
}

fn print_counters(counters: &CountersSection) {
    println!("[counters]");
    println!("  orders_sent  : {}", counters.orders_sent);
    println!("  fills        : {}", counters.fills);
    println!("  cancels      : {}", counters.cancels);
    println!("  rejects      : {}", counters.rejects);
    println!("  realized_cents: {}", counters.realized_cents);
    println!("  unreal_cents : {}", counters.unreal_cents);
    println!("  peak_equity  : {}", counters.peak_equity_cents);
    println!("  max_drawdown : {}", counters.max_draw_cents);
}

fn print_symbols(symbols: &SymbolSection) {
    println!("[symbols]");
    for (idx, slice) in symbols.slots.iter().enumerate() {
        if slice.sym_id == 0 {
            continue;
        }
        println!("  slot {}:", idx);
        println!("    sym_id       : {}", slice.sym_id);
        println!("    breaker_lvl  : {}", slice.breaker_level);
        println!("    flags        : {:#04x}", slice.flags);
        println!("    pos_qty      : {}", slice.pos_qty);
        println!("    avg_px_ticks : {}", slice.avg_px_ticks);
        println!("    realized_cents: {}", slice.realized_cents);
        println!("    unreal_cents : {}", slice.unreal_cents);
    }
}

fn print_log(tile: &EtTile) {
    println!("[mini_log]");
    println!(
        "  head={} count={}",
        tile.log.tail.mini_head, tile.log.tail.mini_count
    );
    for (idx, entry) in tile.log.entries.iter().enumerate() {
        if entry.ts_ms == 0 && entry.event == 0 && entry.actor == 0 && entry.sym_id == 0 {
            continue;
        }
        println!(
            "  {}: ts={} event={} actor={} sym={} code={} aux={} flags={:#04x}",
            idx,
            entry.ts_ms,
            entry.event,
            entry.actor,
            entry.sym_id,
            entry.code,
            entry.aux,
            entry.flags
        );
    }
}
