# atomic_execution_bundle

`atomic_execution_bundle` implements the AEB-512 (Atomic Execution Bundle): a
64-byte capsule that carries an entry leg, bracket exits, routing preferences,
and risk/time budgets in four 128-bit words. Strategies stage W1–W3, flip the
commit bit in W0 with a release store, and routers obtain a coherent snapshot
with a single cache-line read.

## Capsule layout

| Word | Contents |
| ---- | -------- |
| **W0** | `commit:1 | stale:1 | ver:8 | seq:16 | sym_id:16 | strategy_id:8 | account_id:16 | pair_id:12 | created_ms_coarse:24 | ttl_ms:16 | state:2 | kind:2 | has_bracket:1 | reduce_only_bundle:1 | spare:4` |
| **W1** | `side:1 | anchor:2 | order_type:3 | tif:3 | qty:24 | px_ticks:24 (signed) | route_id:10 | slip_cap_bp:12 | post_only:1 | reduce_only:1 | allow_partial:1 | risk_tag:10 | seq_hint:24 | spare:12` |
| **W2** | `tp_ticks:12 (signed) | sl_ticks:12 (signed) | trail_ticks:12 (signed) | tstop_ms:14 | exit_route_id:10 | exit_tif:3 | tp_kind:2 | sl_kind:2 | rearm:1 | scale_out_pct:8 | slip_cap_exit_bp:12 | lat_budget_us:12 | flags:8 | oco_group:12 | spare:8` |
| **W3** | `max_open_ms:20 | max_adverse_cents:24 | exit_on_breaker_ge_level:2 | exit_on_jitter:1 | exit_on_cost_gt:1 | forbid_after_min_ct:11 | eod_flat_min_ct:11 | routeB_id:10 | on_fail:3 | checksum16:16 | ver_tail:8 | seq_tail:16 | spare:5` |

All setters assert in debug builds that field values remain within their width.
`px_ticks`, `tp/sl/trail_ticks` are stored in two's-complement so negative
offsets are supported.

## Publish protocol

1. Build `BundleDraft` by populating W1–W3 and W0 (without the commit bit).
2. Compute the mirrored even version/sequence, write W1–W3, and finish with a
   release-store of W0 that flips `commit` and publishes the even version.
3. Optional: reuse a preallocated draft via `publish_with_reuse` to avoid zeroing
   on the hot path.

Readers call `load()` which retries the seqlock up to eight times. A snapshot is
accepted only when:

- `commit == 1` and `ver` is even.
- `stale == 0`.
- `ver_tail`/`seq_tail` in W3 match the head.
- The checksum over W1–W3 matches (when the `checksum` feature is enabled).

`sequence_pair()` exposes the head/tail counters so routers can poll for
completion without decoding the capsule.

## Features

- `checksum` *(default)* – maintain the 16-bit checksum for torn-load detection.
- `require-cas` *(default)* – forward to `portable-atomic`'s `require-cas` flag.

## Alignment & helpers

The `AtomicExecutionBundle`, `BundleDraft`, and `Snapshot` types are all tagged
with `#[repr(align(64))]` and statically checked to occupy exactly one 64-byte
cache line. Routers can also use `Snapshot::ttl_deadline_coarse` /
`Snapshot::ttl_expired(now)` to enforce the coarse TTL guard without decoding the
entire header.

Run `cargo run --example router_snapshot` for a minimal reader walkthrough that
loads a capsule, enforces TTL, and inspects the entry/bracket fields.

## Telemetry

Use `AtomicExecutionBundle::load_with_diagnostics` with a shared `DenyCounters` to
record structured gate failures (odd version, seq mismatch, checksum, etc.) and
expose them via `DenySnapshot` for logging or metrics. Each exhausted spin cycle
also increments the `attempts_exhausted` counter so backpressure can be spotted.

## Simulation

Enable the `sim` feature to deserialize JSON fixtures into `sim::Scenario` values,
publish bundles, and assert expected accept/deny paths. Try
`cargo test --features sim --test json_scenario` to replay the provided sample.

## Benchmarks

`cargo bench --bench hot_loop` exercises the writer, reader, and cross-thread
loops. The `scripts/aeb-cargo.sh` helper respects `AEB_DISABLE_CHECKSUM=1` to
benchmark the zero-checksum variant quickly.

See `docs/RELEASE.md` for staging benchmark targets, telemetry checks, and the
operational runbook used for release sign-off.

### Release Notes

The release checklist, benchmark thresholds, and telemetry validation steps live in `docs/RELEASE.md`. Keep these in sync with staging measurements.
