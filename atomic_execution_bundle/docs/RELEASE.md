# Release Checklist (AEB-512)

## Performance thresholds

- **Single-thread publish**: ≤ 30 ns (checksum enabled).
- **Single-thread snapshot load**: ≤ 12 ns.
- **Cross-thread publish→load latency**: ≤ 80 ns (2 cores, checksum enabled).

Run `cargo bench --bench hot_loop` on staging hardware (with full default warm-up
and measurement windows) and record the Criterion summary. Update this document
with exact numbers before tagging a release.

## Telemetry validation

- Exercise `router_snapshot` example and ensure denial counters are emitted.
- Replay JSON fixtures with `cargo test --features sim --test json_scenario` and
  confirm expected accept/deny outcomes.
- Export `DenySnapshot` metrics to the observability pipeline and verify the log
  format matches downstream parsers.

## Operational runbook

1. Build the crate with `--features sim` at least once per release to exercise
   serde paths.
2. Archive the exact JSON scenarios used for sign-off under `tests/fixtures/` and
   record their git hashes here.
3. Ensure topology docs reference `load_with_diagnostics` deny codes for router
   alerting thresholds.
4. Capture the staging benchmark results, telemetry checks, and scenario
   outcomes in the release ticket before promote.

## Known follow-ups

- Benchmark thresholds may need adjustment when checksum is disabled for custom
  environments; document any deviations alongside release notes.
- Add real-market data replays once live fills are available.
