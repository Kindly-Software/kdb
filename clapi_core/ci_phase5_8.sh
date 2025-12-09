#!/usr/bin/env bash
# Phase 5.8 CI Validation Script
# Comprehensive testing for stable and nightly Rust configurations
# Date: 2025-10-21

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "=========================================="
echo "Phase 5.8 CI Validation (Stable + Nightly)"
echo "=========================================="
echo ""

# Configuration 1: Stable Rust (Scalar)
echo -e "${BLUE}Configuration 1: Stable Rust (Scalar Fallback)${NC}"
echo "----------------------------------------"

echo "1.1 Checking compilation..."
if cargo check --lib --features timeline-aggregation; then
    echo -e "${GREEN}✅ Stable compilation: PASS${NC}"
else
    echo -e "${RED}❌ Stable compilation: FAIL${NC}"
    exit 1
fi

echo "1.2 Running clippy..."
if cargo clippy --lib --features timeline-aggregation -- -D warnings; then
    echo -e "${GREEN}✅ Stable clippy: PASS${NC}"
else
    echo -e "${RED}❌ Stable clippy: FAIL${NC}"
    exit 1
fi

echo "1.3 Running tests..."
if cargo test --lib --features timeline-aggregation --quiet; then
    echo -e "${GREEN}✅ Stable tests: PASS${NC}"
else
    echo -e "${RED}❌ Stable tests: FAIL${NC}"
    exit 1
fi

echo ""

# Configuration 2: Nightly Rust (SIMD)
echo -e "${BLUE}Configuration 2: Nightly Rust (SIMD Optimized)${NC}"
echo "----------------------------------------"

echo "2.1 Checking compilation..."
if cargo +nightly check --lib --features timeline-aggregation,portable_simd; then
    echo -e "${GREEN}✅ Nightly compilation: PASS${NC}"
else
    echo -e "${RED}❌ Nightly compilation: FAIL${NC}"
    exit 1
fi

echo "2.2 Running clippy..."
if cargo +nightly clippy --lib --features timeline-aggregation,portable_simd -- -D warnings; then
    echo -e "${GREEN}✅ Nightly clippy: PASS${NC}"
else
    echo -e "${RED}❌ Nightly clippy: FAIL${NC}"
    exit 1
fi

echo "2.3 Running tests..."
if cargo +nightly test --lib --features timeline-aggregation,portable_simd --quiet; then
    echo -e "${GREEN}✅ Nightly tests: PASS${NC}"
else
    echo -e "${RED}❌ Nightly tests: FAIL${NC}"
    exit 1
fi

echo "2.4 Checking benchmark compilation..."
if cargo +nightly bench --no-run --features timeline-aggregation,portable_simd; then
    echo -e "${GREEN}✅ Nightly benchmarks: PASS${NC}"
else
    echo -e "${RED}❌ Nightly benchmarks: FAIL${NC}"
    exit 1
fi

echo ""

# Summary
echo "=========================================="
echo -e "${GREEN}✅ ALL CI CHECKS PASSED${NC}"
echo "=========================================="
echo ""
echo "Validated Configurations:"
echo "  - Stable Rust (scalar): ✅ PASS"
echo "  - Nightly Rust (SIMD): ✅ PASS"
echo ""
echo "Quality Checks:"
echo "  - Compilation: ✅ PASS"
echo "  - Clippy: ✅ PASS"
echo "  - Tests: ✅ PASS"
echo "  - Benchmarks: ✅ PASS"
echo ""
echo "Ready for merge!"
echo "=========================================="
