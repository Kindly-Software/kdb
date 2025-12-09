#!/bin/bash
# Compilation Verification Commands
# Run these to verify kindly-web compilation status

echo "=== COMPILATION VERIFICATION ==="
echo ""

echo "1. cargo check --lib"
cargo check --lib 2>&1 | grep -E "Finished|error.*could not compile"
echo ""

echo "2. cargo build --lib --release"
cargo build --lib --release 2>&1 | grep -E "Finished|error.*could not compile"
echo ""

echo "3. cargo test --lib --no-run"
cargo test --lib --no-run 2>&1 | grep -E "Finished|error.*could not compile"
echo ""

echo "4. cargo clippy --lib --no-deps -- -D warnings"
cargo clippy --lib --no-deps -- -D warnings 2>&1 | grep -E "Finished|error.*could not compile|warning.*generated"
echo ""

echo "=== SUMMARY ==="
echo "✅ check: PASSED" if cargo check --lib 2>&1 | grep -q "Finished" else echo "❌ check: FAILED"
echo "✅ build: PASSED" if cargo build --lib --release 2>&1 | grep -q "Finished" else echo "❌ build: FAILED"
echo "✅ test compile: PASSED" if cargo test --lib --no-run 2>&1 | grep -q "Finished" else echo "❌ test compile: FAILED"
