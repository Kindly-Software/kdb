# Internal Release Checklist

This crate is distributed privately. Use the following steps before tagging an internal
release or shipping to downstream systems.

1. Run `tools/release_check.sh` to execute `fmt`, `clippy`, unit tests, and the benchmark smoke.
2. Capture results from `cargo bench --bench multi_account_bench` (target: single-thread 64 envelopes < 40ns, multi-thread 4 threads < 150µs) and stash them in the performance log to monitor regressions.
3. Review `CHANGELOG.md` and ensure the upcoming release entry summarises code and API
   changes.
4. Snapshot the public API using `docs/api_surface.md` and note any additions in the
   changelog.
5. Update `docs/release.md` with any deviations or new validation steps. Run `cargo run --bin offline_gateway -- --threads 4 --cycles 100000 --reset-interval 10000` to sanity-check multi-thread + reset behaviour.
6. Tag the repository (e.g., `git tag v0.2.0-pre1`) and archive the artefact internally.

**Trade Secret Notice**: The crate must not be published to public registries.
