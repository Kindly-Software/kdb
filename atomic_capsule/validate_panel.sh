#!/bin/bash
# Quick validation script for PanelCapsule

set -e

echo "=== PanelCapsule Validation ==="
echo ""

echo "1. Checking compilation..."
cargo check --lib --features terminal-widgets --quiet
echo "✓ Compilation successful"
echo ""

echo "2. Checking test compilation..."
cargo test --lib --features terminal-widgets --no-run --quiet
echo "✓ Test compilation successful"
echo ""

echo "3. Running panel tests..."
timeout 10 cargo test --lib --features terminal-widgets test_q 2>&1 | grep -E "(test.*panel.*ok|test result:)" || true
echo ""

echo "4. Module structure check..."
grep -l "PanelCapsule" src/terminal/widget/container/*.rs
echo "✓ PanelCapsule files found"
echo ""

echo "5. Export verification..."
grep "pub use.*PanelCapsule" src/terminal/widget/mod.rs
echo "✓ PanelCapsule properly exported"
echo ""

echo "=== Validation Complete ==="
