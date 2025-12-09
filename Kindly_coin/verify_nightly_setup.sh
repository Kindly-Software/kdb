#!/bin/bash
# Verify Nightly Rust Setup for Kindly_coin
# Q32 Nightly Enhancement Validation

set -e

echo "================================================================================"
echo "  Nightly Rust Configuration Verification"
echo "================================================================================"
echo ""

# Check rust version
echo "1. Checking Rust toolchain..."
rustc --version | grep -q "nightly" && echo "  ✅ Nightly toolchain active" || echo "  ❌ Not using nightly"
echo ""

# Check configuration files
echo "2. Checking configuration files..."
test -f rust-toolchain.toml && echo "  ✅ rust-toolchain.toml present" || echo "  ❌ Missing rust-toolchain.toml"
test -f .cargo/config.toml && echo "  ✅ .cargo/config.toml present" || echo "  ❌ Missing .cargo/config.toml"
echo ""

# Check LLD linker
echo "3. Checking LLD linker..."
which lld >/dev/null 2>&1 && echo "  ✅ LLD linker available" || echo "  ⚠️  LLD not found (install: sudo apt install lld)"
echo ""

# Check nightly features
echo "4. Checking nightly features..."
grep -q "portable_simd" kindly_core/Cargo.toml && echo "  ✅ portable_simd enabled in kindly_core" || echo "  ❌ Missing portable_simd"
grep -q "const_float" kindly_core/Cargo.toml && echo "  ✅ const_float enabled in kindly_core" || echo "  ❌ Missing const_float"
grep -q "atomic_enhanced" kindly_ubi/Cargo.toml && echo "  ✅ atomic_enhanced enabled in kindly_ubi" || echo "  ❌ Missing atomic_enhanced"
echo ""

# Check compilation
echo "5. Checking compilation status..."
echo "  Running: cargo check --workspace --lib..."
if cargo check --workspace --lib 2>&1 | grep -q "error: could not compile"; then
    FAILED=$(cargo check --workspace --lib 2>&1 | grep -c "error: could not compile" || echo "0")
    echo "  ⚠️  $FAILED crate(s) failed to compile"
    echo "  Note: kindly_network requires BitwiseSerializable API migration"
else
    echo "  ✅ All crates compile successfully"
fi
echo ""

# Check release profile
echo "6. Checking release profile optimizations..."
grep -q 'lto = "fat"' Cargo.toml && echo "  ✅ Fat LTO enabled" || echo "  ❌ Missing fat LTO"
grep -q "strip = true" Cargo.toml && echo "  ✅ Strip symbols enabled" || echo "  ❌ Missing strip"
grep -q "codegen-units = 1" Cargo.toml && echo "  ✅ Single codegen unit" || echo "  ❌ Missing codegen-units=1"
echo ""

# Documentation
echo "7. Checking documentation..."
test -f COMPILATION_EXPERT_NIGHTLY_REPORT.md && echo "  ✅ Full technical report present" || echo "  ❌ Missing report"
test -f NIGHTLY_QUICK_START.md && echo "  ✅ Quick start guide present" || echo "  ❌ Missing quick start"
echo ""

# Summary
echo "================================================================================"
echo "  Verification Complete"
echo "================================================================================"
echo ""
echo "Quick Commands:"
echo "  cargo build --release               # Optimized build (30% faster with LLD)"
echo "  cargo bench --features portable_simd # SIMD benchmarks"
echo "  cargo fix --workspace --allow-dirty # Fix warnings"
echo ""
echo "Documentation:"
echo "  📄 COMPILATION_EXPERT_NIGHTLY_REPORT.md"
echo "  📄 NIGHTLY_QUICK_START.md"
echo "  📄 BUILD_SUCCESS_SUMMARY.txt"
echo ""
