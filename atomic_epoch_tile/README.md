# ET-1kB Epoch Tile

`atomic_epoch_tile` provides the ET-1kB primitive: a fixed-width, cache-aligned
snapshot tile that captures the last known trading session state in 1,024 bytes.
It is designed for one-writer/many-reader workloads where readers need a single
pointer + checksum to obtain a crash-safe snapshot.

## Layout

Each tile is `repr(C, align(64))` and partitioned into four contiguous
256-byte blocks:

- **H0 Header & Integrity** – magic, commit/version markers, epoch metadata,
  capsule digests, hash breadcrumbs, and an XXH32 checksum.
- **C1 Counters & Hists** – order flow counts, P&L aggregates, latency metrics,
  and two compact histograms for latency and slippage.
- **S2 Per-Symbol Slices** – four 64-byte slices mirroring key APC/AVS fields
  (position, P&L, breaker flags, headroom, microstructure hints).
- **L3 Mini-Log & Tail** – eight 24-byte log entries plus tail metadata that
  mirrors the head version/sequence values for reader validation.

The crate uses `static_assertions` to enforce the section sizes and exports the
layout structs so callers can populate fields directly.

## Writer

`TilePublisher` drives the two-phase publish protocol:

1. Populate a `TileShadow` in private memory (commit flag cleared).
2. Compute the XXH32 checksum with the commit flag and checksum word treated as
   zero.
3. Copy the shadow tile into the ring slot, then release-store the checksum,
   version, and commit flag.
4. Optionally chain tiles via keyed BLAKE3 (16-byte truncation) for
   `prev_tile_hash`.

Helpers handle the atomic stores and sequence/version counters so the caller can
focus on wiring live session data into the snapshot.

## Builder & Ring

Use `TileInputs` together with `populate_tile` when you want to assemble a tile
from higher-level feed aggregates (APC/AVS/ALT, capsule digests, etc.). The
builder keeps the translation logic in one place and clamps symbol/log counts to
their fixed capacities.

`TileRing::create` provides an mmap-backed ring (aligned to `EtTile`) that the
publisher can write into directly. `TileRingMapping::open` exposes a read-only
view for consumers, and `FlushStrategy` chooses between async/sync flush
semantics when persisting tiles to disk.

`session::LiveFeeds` wires the live atomic capsules directly into tile
construction. Call `publish_from_feeds` with your APC/AVS/ALT handles plus
session metadata and counters; it stages the shadow tile, publishes via
`TilePublisher`, and optionally flushes the ring.

`LogTail::apm_summary` is a compact 32-bit digest of the latest
APM-1024 header so downstream analytics can triage portfolio posture fast:

- bits 0–1: portfolio breaker level
- bits 2–5: active symbol count (capped at 15)
- bits 6–15: portfolio flags mask
- bits 16–31: remaining daily loss headroom in 10k-cent buckets (saturating)

## Reader

`scan_latest_committed` walks a ring buffer from a WAL hint, validating each
tile via `validate_tile` until a committed tile with a matching checksum and
head/tail markers is found. Readers only need relaxed loads for field access
after validation.

## CLI utilities

Two helper binaries ship with the crate:

- `etdump` – prints the latest (or a specific) tile in a human-friendly or JSON
  format: `cargo run -p atomic_epoch_tile --bin etdump -- /path/to/ring.bin`.
- `etverify` – scans a ring and validates every tile, reporting counts and the
  latest committed slot.

## Testing

Run the unit test suite with:

```bash
cargo test -p atomic_epoch_tile
```

The tests cover structure sizes, checksum invariants, the writer two-phase
commit flow, and reader recovery behaviour across corrupted tiles.
