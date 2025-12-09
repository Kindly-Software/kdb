#!/usr/bin/env bash
set -euo pipefail

# Local release checklist for atomic_risk_envelope.
# Run from the repository root. CI runs these steps as well, but this script
# provides a single entry point when preparing a whistle-stop internal release.

cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo bench --bench multi_account_bench -- --quick --save-baseline release_check
