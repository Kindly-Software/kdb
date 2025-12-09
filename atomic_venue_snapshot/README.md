# `atomic_venue_snapshot`

`atomic_venue_snapshot` implements the AVS-128 (Atomic Venue Snapshot) capsule: a single
`AtomicU128` updated by the market-data writer thread and consumed via a single relaxed
load by latency sensitive strategies. The 128-bit word carries spread, imbalance, micro-
price offset, depth summaries, short-horizon volatility, sweep detection, and coarse
sequence metadata in one atomic read.

## Layout summary

```text
+------------------------------- 128 bits total -------------------------------+
| spread_u8 | obi_s12 | micro_off_s12 | sum_bid_u16 | sum_ask_u16 | vol_u16 |
| 120..127  |108..119 |    96..107    |    80..95    |    64..79    | 48..63 |
| sweep_u1 | trend_s11 | ts_ms_q4_u24 | ver_u8 | seq_u4 |
|   47     |   36..46   |    12..35    |  4..11 |  0..3  |
+------------------------------------------------------------------------------+
```

Each field is stored as fixed-width integer, with signed quantities encoded in two's
complement. The imbalance is represented as signed Q1.10 (–1024..+1023), volatility as
unsigned Q8.8 basis points, and the coarse timestamp uses millisecond/4 quanta.

## Contract highlights

- **Writer:** single producer per symbol performs integer computations, packs the word
  and issues a `store(Release)`.
- **Readers:** many parallel consumers perform a single `load(Relaxed)`, branch on the
  extracted fields, and validate staleness.
- **Staleness guard:** discard snapshots older than ≈250 ms by checking the coarse
  timestamp against `now_ms`.

See the crate documentation for bit masks, fixed-point helpers, and packing APIs to wire
AVS-128 into book writers, strategy evaluators, and telemetry.

## Producer helper (requires `std`)

Enable the `std` feature to use [`AvsWriter`], a stateful publisher that consumes
top-of-book updates, computes the derived imbalance/microprice/volatility/sweep fields,
and writes snapshots via an embedded `Avs128`. The helper manages EWMA volatility, the
200 ms trend buffer, sweep detection windows, and the coarse timestamp quantisation so
the L2 thread only needs to feed raw prices, depth, and marketable flow.

- Example bridge from an L2 feed: `cargo run --example l2_stream --features std`
- Historical calibration via CSV replay: `cargo run --example replay_csv --features std -- [--json] [--config cfg.json] [--output stats.json] [--bucket-ms 60000] <file>`
- Per-run overrides: JSON file matching the fields in `WriterConfig` (see `examples/avs_writer_config.sample.json`).
- Bucketed summaries: supply `--bucket-ms` to emit per-window statistics and include them in the JSON payload.
- Hot-path cost sanity check: `cargo bench --bench publish --features std`
- Aggregate replay metrics programmatically via `SnapshotStatsBuilder` in `analysis`.
