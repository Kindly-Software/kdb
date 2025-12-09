#!/bin/bash
# Compilation validation script for atomic_capsule with nightly Rust
# UCE32 Q32: Validate cutting-edge nightly features compile cleanly

set -euo pipefail

echo "=========================================="
echo "  Atomic Capsule Compilation Validation"
echo "  UCE32 Q32 Nightly Features"
echo "=========================================="
echo ""

# Check nightly Rust is available
echo "[1/8] Checking Rust nightly version..."
if ! rustc --version | grep -q nightly; then
    echo "❌ ERROR: Nightly Rust required"
    echo "   Install with: rustup install nightly"
    echo "   Set default: rustup default nightly"
    exit 1
fi
rustc --version
echo "✅ Nightly Rust detected"
echo ""

# Check cargo-clippy is available
echo "[2/8] Checking cargo-clippy availability..."
if ! command -v cargo-clippy &> /dev/null; then
    echo "❌ ERROR: cargo-clippy not found"
    echo "   Install with: rustup component add clippy"
    exit 1
fi
echo "✅ cargo-clippy available"
echo ""

# Test default features (std only)
echo "[3/8] Testing default features (std)..."
cargo +nightly check --color=always
echo "✅ Default features compile"
echo ""

# Test portable_simd feature
echo "[4/8] Testing portable_simd feature..."
cargo +nightly check --features portable_simd --color=always
echo "✅ portable_simd feature compiles"
echo ""

# Test all features
echo "[5/8] Testing all features..."
cargo +nightly check --all-features --color=always
echo "✅ All features compile"
echo ""

# Test no_std compatibility
echo "[6/8] Testing no_std compatibility..."
cargo +nightly check --no-default-features --color=always
echo "✅ no_std compatibility verified"
echo ""

# Run test suite
echo "[7/8] Running test suite..."
cargo +nightly test --all-features --color=always
echo "✅ All tests pass"
echo ""

# Check for warnings with clippy
echo "[8/8] Running clippy (zero warnings enforcement)..."
cargo +nightly clippy --all-features --color=always -- -D warnings
echo "✅ Zero warnings (clippy clean)"
echo ""

echo "=========================================="
echo "  🎉 ALL COMPILATION CHECKS PASSED!"
echo "=========================================="
echo ""
echo "Summary:"
echo "  ✅ Nightly Rust validated"
echo "  ✅ Default features compile"
echo "  ✅ portable_simd feature compiles"
echo "  ✅ All features compile"
echo "  ✅ no_std compatibility verified"
echo "  ✅ Test suite passes"
echo "  ✅ Zero warnings (clippy)"
echo ""
echo "Ready for production deployment."
