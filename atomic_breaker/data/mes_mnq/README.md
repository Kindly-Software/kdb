# MES/MNQ Scalping Telemetry Dataset

Each CSV captures a 10 Hz stream of normalized metrics for the circuit breaker during a
particular microstructure regime. Columns:

- `timestamp_ms` – monotonic millisecond timeline from the start of the scenario.
- `mid_price` – synthetic mid-price (tick units) centred on the session anchor.
- `spread_ticks` – inside spread in ticks.
- `order_latency_ms` – end-to-end order latency (submission → ACK/fill).
- `fill_ratio` – realised fills / intended quantity.
- `imbalance` – short-horizon order-book imbalance (−1.0..1.0).
- `micro_vol` – realised micro-volatility (ticks per second).
- `pnl_ticks` – per-sample PnL delta in ticks.
- `err_inc` – breaker error increment to apply.
- `cause` – cause bitmask (see `cause.rs`).
- `mu_norm` – normalized mean metric (latency/slippage vs. budget).
- `sg_norm` – normalized jitter metric (volatility vs. budget).
- `success_window_ms` – observed recovery latency after breaker action (0 if unknown).
- `recovered_within_target` – whether recovery occurred within strategy dwell target.
- `gateway_primary_latency_ms`, `gateway_secondary_latency_ms` – per-gateway latencies to
  capture routing jitter across brokers/venues.
- `queue_depth_primary`, `queue_depth_secondary` – estimated resting size in the queue at
  the top of book for each gateway.
- `active_primary` – 1 if the strategy favoured the primary gateway during the sample.

The final columns map directly onto `ActionOutcome` and provide richer signals for
`LevelFeedback` to evaluate dwell/backoff effectiveness.

Regenerate the datasets with:

```
python3 tools/generate_mes_mnq.py
```

To inspect calibration suggestions:

```
cargo run --example mes_mnq_loader --features "std auto_tune" \
    -- dataset=data/mes_mnq/liquidity_vacuum.csv --summary vacuum.json
```

The loader prints auto-tune deltas, dwell feedback, and (optionally) writes a JSON
summary suitable for dashboards.
