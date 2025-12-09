# Atomic Breaker

Universal, atomic, bit-packed circuit breaker primitive designed for hot-path control
loops where waiting on locks is not an option. The entire breaker state fits inside a
single `AtomicU64`, enabling wait-free reads and single-store updates.

## Bit Layouts

### `standard64` (default)

```
63           58 57        50 49          34 33          18 17        4 3 2 1 0
+--------------+-------------+-------------+-------------+-----------+-+-+-+
| backoff (6)  | cause (8)   | sg_norm Q8.8| mu_norm Q8.8| err (14)  |L|L|S|
+--------------+-------------+-------------+-------------+-----------+-+-+-+
```

### `compact48`

```
47          32 31          16 15         4 3 2 1 0
+-------------+-------------+-----------+-+-+-+
| sg_norm Q6.10| mu_norm Q6.10| err (12)|L|L|S|
+-------------+-------------+-----------+-+-+-+
```

State bits encode `Closed`, `HalfOpen`, `Open`, and `ForcedOpen`. Level bits carry the
fractal degradation tier (L0–L3). Cause flags follow `cause.rs` (`THERM`, `NET`, `IO`, …).

## Quick Start

```rust
use atomic_breaker::breaker::{AtomicBreakerGuard, AtomicBreakerSWeMR, State};
use atomic_breaker::policy::{self, Policy};

let breaker = AtomicBreakerSWeMR::new_standard64(State::Closed);
let mut last_change = 0;
let pol = Policy::ui_holographic();

policy::evaluate(&breaker, 18.5, 1.0, 2, 1_000, &mut last_change, &pol);
let guard = AtomicBreakerGuard::new(breaker.load_acquire());
assert_eq!(guard.state(), State::Open);
```

## Feature Flags

- `standard64` *(default)* – enable the full 64-bit layout with causes/backoff.
- `compact48` – swap to the 48-bit hot-path layout (`standard64` must be disabled).
- `mpmc` – multi-writer breaker that uses a bounded CAS loop.
- `std` – opt back into the standard library for tests, examples, and benches.
- `serde` – serialization for diagnostics and packed words.
- `pmu` – experimental Linux-only telemetry that folds perf counters into breaker metrics.
- `auto_tune` – observability helpers (history ring buffer, metrics taps, CSV export) for
  adaptive policy experiments.

Only one layout feature may be active. The crate enforces this at compile time.

## Memory Ordering Contract

Writers publish dependent configuration (e.g., new processing pipelines) **before**
calling `store(Release)` on the breaker. Readers that might consume that configuration
must `load(Acquire)` before dereferencing it. Metrics-only readers can stay relaxed.

## Hardware Telemetry (`pmu`)

- Enable with `--features "pmu"` (Linux only) to access perf-event backed samples.
- Use `telemetry::PmuCollector` to poll cycles and LLC misses; combine with
  `policy::evaluate_with_telemetry` or `AtomicBreakerSWeMR::apply_sample` to update metrics.
- See `examples/pmu_demo.rs` for a minimal driver loop. Ensure the running user has
  permission to read perf events (e.g., `perf_event_paranoid` settings).

## Adaptive Policy (`auto_tune`)

- Enable `--features "auto_tune"` to capture transition history, register `MetricsTap`
  implementations, and access the `AutoCalibrator`.
- Record breaker episodes into a `HistoryBuffer` (via `policy::EvaluationObservers`) and
  feed them into `policy::tune(&history, &baseline_policy, &targets)` to produce a
  `PolicyDraft` for review.
- Use warm-up mode by invoking `policy::tune_with_mode(..., CalibrationMode::WarmUp { min_observations })`
  when you only want adjustments after an initial settling window.
- Try the synthetic generator with
  `cargo run --example auto_tune_scenarios --features "std auto_tune"` to inspect canned
  overload/under-utilised/flicker datasets and the calibrator’s suggested adjustments.
- Feed annotated histories into `LevelFeedback::analyze` (or `policy::adjust_dwell`) to
  automatically stretch or relax dwell/backoff parameters based on each level’s observed
  recovery latencies.
- `data/mes_mnq/*.csv` provide disciplined MES/MNQ scalping scenarios; load them via
  `cargo run --example mes_mnq_loader --features "std auto_tune" data/mes_mnq/liquidity_vacuum.csv`.

## Testing & Benchmarks

```
cargo test --features std
cargo bench --features std
```

Benchmarks are powered by Criterion and emit HTML reports under
`target/criterion/`. Property-based layout round-trips live alongside the unit
suite.

## MSRV

The minimum supported Rust version is **1.76.0**.

## Examples

Three end-to-end demos live in `examples/` and show how to drive UI, audio, and trading
pipelines using nothing more than breaker loads and simple level/state tables.
