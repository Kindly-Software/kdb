#!/bin/bash
set -euxo pipefail

echo "=== Clapi Core Compilation Validation ==="

echo "Step 1: Check compilation"
cargo +nightly check --all-features

echo "Step 2: Build release"
cargo +nightly build --release

echo "Step 3: Run tests"
cargo +nightly test --all-features

echo "Step 4: Clippy (zero warnings)"
cargo +nightly clippy --all-features -- -D warnings

echo "Step 5: Miri (UB detection)"
cargo +nightly miri test --lib

echo "Step 6: Build documentation"
cargo +nightly doc --all-features --no-deps

echo "✅ All compilation checks passed!"
