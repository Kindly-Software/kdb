# Project Status Overview

## Feature Coverage Snapshot

- **Core layouts**: `standard64` and `compact48` fully covered by unit, property-based, and Tarpaulin runs. Compact layout exercises live in `compact_tests` within `breaker.rs`.
- **Telemetry stack**: Introduced feature-gated modules under `telemetry/`. `TelemetrySample` + `TelemetrySource` power both `apply_sample` APIs and `policy::evaluate_with_telemetry`. `pmu` feature (Linux-only) wraps perf counters via `PmuCollector`; unit-tested with mock counters.
- **Auto-tune plumbing** (`auto_tune` feature): Added `telemetry::history::HistoryBuffer` (ring buffer, CSV export) and `policy::EvaluationObservers` helpers. `MetricsTap` trait plus `MetricsSnapshot` expose before/after ratios without touching the hot path. Example tests exercise transition logging and metrics taps.
- **Auto-calibration (Phase 2)**: `telemetry::calibration::AutoCalibrator` analyses history windows and emits `PolicyDraft`s based on target success/transition rates. `policy::tune` is now the entry point for offline or warm-up tuning loops. `telemetry::scenario` supplies canned histories (overload, under-utilised, flicker, mixed, error-heavy) for deterministic calibration runs.
- **Level feedback (Phase 3)**: `telemetry::feedback::LevelFeedback` inspects annotated histories via `ActionOutcome`s, recommending dwell/backoff deltas applied through `policy::adjust_dwell`.
- **Benchmarks**: `benches/microbench.rs` now measures SWeMR, MPMC, and compact paths across writer counts. Criterion HTML reports emitted to `target/criterion/`.
- **Datasets**: `data/mes_mnq/*.csv` capture disciplined MES/MNQ scalping regimes (normal, vacuum, queue-loss, infra impairments, over-trading) consumable via `examples/mes_mnq_loader.rs`; regenerate with `python3 tools/generate_mes_mnq.py`.
- **Examples**: `stress_sim.rs` drives randomised workloads with `TelemetrySample`s; `pmu_demo.rs` integrates perf events (requires `--features "pmu"` and Linux perf permissions).

## Automation & Coverage

- `ci/coverage_matrix.sh` executes Tarpaulin across `standard64`, `compact48`, `serde`, and `mpmc` permutations, uploading XML artifacts in CI.
- GitHub workflow (`.github/workflows/ci.yml`) runs fmt, clippy, feature test matrix, coverage matrix, and a shortened bench pass.
- Current Tarpaulin runs report ≈85% line coverage aggregated across feature sets.

## Outstanding Notes

- `pmu_demo` cannot run inside the default CI sandbox; test locally on Linux with accessible perf counters.
- `diagnostics` feature enables runtime invariants (state/level sanity, backoff bounds) for deeper debugging.
- Future stretch goals: integrate real perf sampling into CI (requires privileged runners), expand the auto-calibration heuristics with percentile weighting/decay, and evaluate hardware-hint ingestion once PMU coverage solidifies.
