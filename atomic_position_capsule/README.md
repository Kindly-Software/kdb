# atomic_position_capsule

`atomic_position_capsule` implements the APC-512 (Atomic Position Capsule): a
64-byte snapshot that exposes live position, P&L, and risk headroom in four
128-bit words. Writers stage W1-W3, compute an optional checksum, mirror the
sequence/version into W3, and publish an even version in W0 with a single
release store. Strategies, routers, and dashboards can then gate orders or
updates with one cache-line read.

## Capsule layout

| Word | Contents |
| ---- | -------- |
| **W0** | `pos_qty_s32:32 | avg_px_ticks_s24:24 | rem_daily_loss_cents_u32:32 | flags_u8:8 | ver_even_u8:8 | seq_u16:16 | pad:8` |
| **W1** | `realized_cents_s32:32 | unrealized_cents_s32:32 | peak_equity_cents_s32:32 | trailing_draw_cents_u32:32` |
| **W2** | `now_min_ct_u11:11 | forbid_after_min_ct_u11:11 | eod_flat_min_ct_u11:11 | open_since_ms_u24:24 | max_open_ms_u20:20 | max_contracts_u12:12 | max_per_trade_cents_u20:20 | risk_flags_u8:8 | reserved_u11:11` |
| **W3** | `sym_id_u16:16 | account_id_u16:16 | last_exec_id_u32:32 | breaker_level_u2:2 | alt_health_u6:6 | violation_bits_u16:16 | checksum16_u16:16 | ver_tail_u8:8 | seq_tail_u16:16` |

All setters assert in debug builds that values respect bit width. Signed fields
(`pos`, `avg_px_ticks`, `P&L`) use two's-complement encoding, enabling negative
positions or losses without extra flags.

## Shared flags & levels

- `FLAG_FLAT`, `FLAG_LONG`, `FLAG_SHORT`, `FLAG_LOCKED`, `FLAG_HALT` - live position state published in W0.
- `RISK_FLAG_PAUSE_NEWS`, `RISK_FLAG_NEWS_WINDOW`, `RISK_FLAG_STALL_LAT` - session-local risk gates carried in W2.
- `BREAKER_REDUCE_ONLY_LEVEL` - minimum breaker level that forces reduce-only routing when matched in W3.

## Publish protocol

1. Load current version/sequence from W0.
2. Flip the version to the next odd value (`ver | 1`), write it to W0 (relaxed).
3. Write W1-W3, mirroring the eventual even version/sequence into W3.
4. Optionally compute the checksum over W1-W3 (`checksum` feature, on by default).
5. Store W0 with the even version and sequence using `store(Release)`.

Readers perform relaxed loads of W0-W3 (with an acquire fence around W0) and
accept the snapshot only when:

- `ver` is even and matches `ver_tail`.
- `seq` matches `seq_tail`.
- The checksum matches (when enabled).

## Reader gate (hot-path excerpt)

```rust
use atomic_position_capsule::GateDecision;

if let Some(snapshot) = capsule.load() {
    match snapshot.gate_order(desired_delta_qty) {
        GateDecision::Allow => {
            // proceed with new risk
        }
        GateDecision::ReduceOnly => {
            // only send reduce-only orders
        }
        GateDecision::Deny(reason) => {
            eprintln!("deny: {:?}", reason);
            return;
        }
    }
}
```

## Features

- `checksum` *(default)* - maintain a 16-bit checksum across W1-W3 for torn-load detection.
- `require-cas` *(default)* - forward to `portable-atomic`'s CAS enforcement for platforms without native 128-bit atomics.

Disable the checksum for test/bench variants by exporting `APC_DISABLE_CHECKSUM=1`
and using the helper script (`./scripts/apc-cargo.sh test`).

## Tests & benches

```bash
# default profile
cargo test

# checksum disabled profile
APC_DISABLE_CHECKSUM=1 ./scripts/apc-cargo.sh test

# hot-path benchmark
cargo bench --bench hot_loop

# replay a recorded session
cargo run --example replay_driver -- examples/data/replay_sample.jsonl
```

CI mirrors the above flows and exercises both checksum-enabled and disabled
configurations.

## Integration checklist

- Run `cargo run --example replay_driver -- <log_path>` against recorded sessions to validate gate decisions before live rollout.
- Capture `GateMetrics` snapshots in your strategy/router to feed observability dashboards or alerts.
- Shadow deploy APC gating on target hardware with metrics enabled before switching to hard enforcement.

