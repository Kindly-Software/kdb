#!/bin/bash
# Phase 2 Compilation Validation Script
# Validates that all Phase 2 modules compile cleanly with zero warnings

set -euxo pipefail

echo "=== Phase 2 Compilation Validation ==="
echo "Working directory: $(pwd)"
echo "Rust version: $(rustc --version)"
echo "Cargo version: $(cargo --version)"
echo ""

cd /home/samuel/Primitives/clapi_core

echo "Step 1: Check library compilation"
cargo +nightly check --lib --all-features

echo ""
echo "Step 2: Check all binaries (if any)"
# Note: No binaries in Phase 1, may be added in Phase 2
if cargo +nightly check --bins --all-features 2>&1 | grep -q "no targets"; then
    echo "No binaries to check (expected for Phase 1)"
else
    cargo +nightly check --bins --all-features
fi

echo ""
echo "Step 3: Run unit tests (lib)"
cargo +nightly test --lib

echo ""
echo "Step 4: Run integration tests"
cargo +nightly test --test integration_tests

echo ""
echo "Step 5: Run Phase 2 proxy tests (if implemented)"
if [ -f tests/proxy_unit_tests.rs ]; then
    cargo +nightly test --test proxy_unit_tests
else
    echo "proxy_unit_tests.rs not yet implemented"
fi

if [ -f tests/proxy_property_tests.rs ]; then
    cargo +nightly test --test proxy_property_tests
else
    echo "proxy_property_tests.rs not yet implemented"
fi

echo ""
echo "Step 6: Check benchmarks compile"
cargo +nightly bench --no-run

echo ""
echo "Step 7: Clippy (zero warnings)"
cargo +nightly clippy --all-features --all-targets -- -D warnings

echo ""
echo "Step 8: Build documentation"
cargo +nightly doc --all-features --no-deps

echo ""
echo "Step 9: Format check"
cargo +nightly fmt --check

echo ""
echo "✅ All Phase 2 compilation checks passed!"
echo ""
echo "Summary:"
echo "  - Library: ✓ Compiled"
echo "  - Tests: ✓ Passed"
echo "  - Benchmarks: ✓ Compiled"
echo "  - Clippy: ✓ Zero warnings"
echo "  - Docs: ✓ Built"
echo "  - Format: ✓ Checked"
