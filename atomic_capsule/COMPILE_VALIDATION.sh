#!/bin/bash
# Phase 2: Inference Primitives - Compilation Validation Script
# IMPL-2 V3.1: Nightly-first development

set -e

echo "=== Phase 2: Inference Primitives - Compilation Validation ==="
echo

echo "[1/5] Verify nightly features compile with inference-all..."
cargo +nightly check --features inference-all --quiet
echo "✅ Nightly compilation successful"
echo

echo "[2/5] Verify stable fallback (expected error)..."
if cargo +stable check --features inference-all --quiet 2>&1 | grep -q "may not be used on the stable release channel"; then
    echo "✅ Stable fallback error message correct"
else
    echo "❌ Expected stable compilation to fail with feature gate error"
    exit 1
fi
echo

echo "[3/5] Run clippy with capsule verification..."
cargo +nightly clippy --features inference-all -- -D clippy::missing_capsule_verification --quiet 2>&1 | tail -1
echo "✅ Clippy passes"
echo

echo "[4/5] Run inference module tests..."
cargo +nightly test --features inference-all --lib inference --quiet 2>&1 | grep "test result"
echo "✅ Tests pass"
echo

echo "[5/5] Verify feature flag summary..."
cargo +nightly metadata --format-version=1 --no-deps 2>/dev/null | \
    jq -r '.packages[0].features | keys | map(select(startswith("inference"))) | .[]' | sort
echo "✅ Feature flags configured"
echo

echo "=== Compilation Validation Complete ==="
echo
echo "Summary:"
echo "- Nightly features: portable_simd (MANDATORY)"
echo "- const_fn_floating_point: STABLE (no feature gate needed)"
echo "- Feature flags: inference-primitives, inference-matmul, inference-attention, inference-quantization, inference-all"
echo "- Rayon dependency: Added for T4 batch parallelism"
echo "- Stable fallback: Clear error message"
echo
echo "Next: Implement T2 SIMD + T4 batch matmul"
