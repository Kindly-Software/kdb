# Atomic Risk Envelope

Atomic Risk Envelope (ARE) packs per-account guardrails into a single 128-bit word so
nanosecond hot paths can enforce prop-trading limits without branching across multiple
structures. The layout captures daily loss, per-trade limits, contract caps, and session
time guards alongside control flags.

```
[ rem_daily_loss_cents:32 | max_per_trade_cents:24 | max_contracts:12 | max_open_ms:20 |
  forbid_after_min_ct:11 | eod_flat_min_ct:11 | flags:6 | ver:6 | seq:6 ]
```

## Highlights

- One `AtomicU128` publish: readers load once and get every guard.
- Typed `flag::Flags` exposes `PAUSED`, `EMERGENCY_FLAT`, `NEWS_LOCKOUT`, and venue overrides.
- Order gate helper enforces cost, contract, duration, and time limits in constant time.
- `RiskEnvelope` helpers atomically debit daily loss, update flags, and round-trip packed bits.
- Offline gateway and bench harness cover single and multi-thread order flows.
- `no_std` friendly; integrates directly with matching, risk, or gateway loops.

## Feature Flags

- `serde` – enable `Serialize`/`Deserialize` for `Fields`, `RiskEnvelope`, `OrderCheck`, and gate results.

## Stability & Semver

`atomic_risk_envelope` is moving toward a `v1.0.0` API lock. Until then, expect minor
breaking changes on `0.x` releases while we finish integration testing. The current
stability guarantees live in [`docs/api_surface.md`](docs/api_surface.md), and planned
changes are captured in [`CHANGELOG.md`](CHANGELOG.md). Once the foundational types
(`Fields`, `RiskEnvelope`, `AtomicRiskEnvelope`, `OrderCheck`) are frozen we will ship a
`v0.2.0` release candidate ahead of `v1.0.0`.

## Memory Ordering & Safety

- Writers must publish dependent state (e.g., routing tables) before calling
  `AtomicRiskEnvelope::store` or `debit_daily_loss` with `Ordering::Release`/`Ordering::AcqRel`.
- Readers that need the new configuration must load with `Ordering::Acquire` via
  `AtomicRiskEnvelope::load` or `load_validated` before acting on it.
- Metrics-only readers can use relaxed loads, but they must not hand out references to
  the data that was protected by Release/Acquire pairs.
- Prefer the provided `debit_daily_loss` helper for fills; custom mutators should use
  `fetch_update` with Acquire/Release semantics to preserve linearizability.

## Error Handling

## Stability Matrix

| Item | Status | Notes |
| --- | --- | --- |
| `Fields`, `RiskEnvelope`, `AtomicRiskEnvelope`, `OrderCheck` | Stable | Covered by tests and simulator; breaking changes require semver bump. |
| `RiskEnvelope::update_flags`, `fetch_update`, reset helpers | Provisional | Subject to change pending production feedback. |
| CLI (`offline_gateway`), examples, tests | Internal Only | No stability guarantee; may change without notice. |
| `serde` feature (JSON loaders) | Stable | Required for config bootstrap. |
| `bin` feature (clap CLI) | Stable | Enabled by default for internal tooling. |


- `FieldError` exposes `kind()`, `field()`, and `other_field()` so validation failures can
  be surfaced with structured metadata.
- `GateOutcome::Deny` carries a `code()` helper, making it easy to feed reason labels
  into logging or metrics pipelines.

## Simulator CLI

The `offline_gateway` binary exercises ARE enforcement end-to-end. Example usage:

```bash
# Single-thread run with defaults
cargo run --bin offline_gateway -- --cycles 500000

# Multi-thread run with periodic daily-loss resets
cargo run --bin offline_gateway -- --accounts 32 --cycles 500000 --threads 4 --reset-interval 10000

# Bootstrap from JSON config (requires `--features serde`)
cargo run --bin offline_gateway -- --config docs/config.sample.json --threads 2
```

Key flags: `--accounts`, `--cycles`, `--threads`, `--fill-divisor`, `--reset-interval`, and `--config` (JSON array of `Fields`). Denial reasons are aggregated using the `DenyReason::code()` helper.

## Usage

```rust
use atomic_risk_envelope::{flag, Fields, OrderCheck, RiskEnvelope};

let fields = Fields {
    rem_daily_loss_cents: 45_000,
    max_per_trade_cents: 7_500,
    max_contracts: 10,
    max_open_ms: 90_000,
    forbid_after_min_ct: 900,
    eod_flat_min_ct: 915,
    flags: flag::PAUSED,
    version: 1,
    sequence: 0,
};
let env = RiskEnvelope::try_from_fields(fields).unwrap();
let outcome = env.evaluate_order(OrderCheck::new(6_500, 4, 870, 45_000));
assert!(matches!(outcome, atomic_risk_envelope::GateOutcome::Deny(_)));
```

Reload the envelope atomically when routing clears limits or bumps sequences:

```rust
use atomic_risk_envelope::{AtomicRiskEnvelope, Fields, RiskEnvelope};
use core::sync::atomic::Ordering;

let env = RiskEnvelope::try_from_fields(Fields { sequence: 5, ..fields }).unwrap();
let atomic = AtomicRiskEnvelope::new(env);
let live = atomic.load(Ordering::Acquire);
let validated = atomic.load_validated(Ordering::Acquire).unwrap();

// Packed words received from persistence or IPC sources can be verified directly.
let round_trip = RiskEnvelope::try_from_bits(live.bits()).unwrap();
assert_eq!(round_trip.bits(), live.bits());

// Debit remaining daily loss atomically (returns previous envelope on success).
let prev = atomic
    .debit_daily_loss(2_000, Ordering::SeqCst, Ordering::SeqCst)
    .unwrap();
assert_eq!(prev.rem_daily_loss_cents(), live.rem_daily_loss_cents());
let updated = atomic.load(Ordering::SeqCst);
assert!(updated.rem_daily_loss_cents() < live.rem_daily_loss_cents());
```

See `examples/hedge_loop.rs` for a single-account fill loop, and
`examples/multi_account_sim.rs` for a multi-account bootstrap that validates raw words
before routing orders:

```
cargo run --example hedge_loop
cargo run --example multi_account_sim
```

## Testing

```
cargo test
```
