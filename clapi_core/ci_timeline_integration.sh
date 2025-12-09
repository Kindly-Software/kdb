#!/usr/bin/env bash
set -euo pipefail

# Timeline Integration CI Validation Script
# Fast CI validation for stable and nightly Rust configurations
# Target: <5 minutes total validation time
# Date: 2025-10-21

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

START_TIME=$(date +%s)

echo "=========================================="
echo -e "${MAGENTA}Timeline Integration CI Validation${NC}"
echo "=========================================="
echo -e "${CYAN}Fast validation for CI/CD pipelines${NC}"
echo "Target: <5 minutes total"
echo "=========================================="
echo ""

# ============================================================================
# CONFIGURATION 1: Stable Rust (Scalar Fallback)
# ============================================================================
echo -e "${BLUE}[1/2] Configuration 1: Stable Rust (Scalar Fallback)${NC}"
echo "Testing basic functionality without SIMD optimizations"
echo "=========================================="
echo ""

# Check stable rust availability
if ! command -v rustc &> /dev/null; then
    echo -e "${RED}❌ Rust not found in PATH${NC}"
    exit 1
fi

RUST_VERSION=$(rustc --version)
echo "Rust version: $RUST_VERSION"
echo ""

# Stage 1.1: Compilation check
echo -e "${YELLOW}[1.1] Checking compilation (stable)...${NC}"
if cargo check --lib --features timeline-aggregation 2>&1 | tee ci_stable_check.log | grep -q "Finished"; then
    WARNINGS=$(grep -c "warning:" ci_stable_check.log || true)
    WORKSPACE_WARNINGS=$(grep -c "profiles for the non root package will be ignored" ci_stable_check.log || true)
    CODE_WARNINGS=$((WARNINGS - WORKSPACE_WARNINGS))

    if [ "$CODE_WARNINGS" -eq 0 ]; then
        echo -e "${GREEN}✅ Stable compilation: PASS (0 warnings)${NC}"
    else
        echo -e "${RED}❌ Stable compilation: FAIL ($CODE_WARNINGS warnings)${NC}"
        exit 1
    fi
else
    echo -e "${RED}❌ Stable compilation: FAIL (errors)${NC}"
    exit 1
fi
echo ""

# Stage 1.2: Clippy check
echo -e "${YELLOW}[1.2] Running clippy (stable)...${NC}"
if cargo clippy --lib --features timeline-aggregation -- -D warnings 2>&1 | tee ci_stable_clippy.log | tail -n 5; then
    echo -e "${GREEN}✅ Stable clippy: PASS${NC}"
else
    echo -e "${RED}❌ Stable clippy: FAIL${NC}"
    exit 1
fi
echo ""

# Stage 1.3: Test compilation (no run, faster)
echo -e "${YELLOW}[1.3] Compiling tests (stable)...${NC}"
if cargo test --lib --features timeline-aggregation --no-run 2>&1 | grep -q "Finished"; then
    echo -e "${GREEN}✅ Stable tests: COMPILE PASS${NC}"
else
    echo -e "${RED}❌ Stable tests: COMPILE FAIL${NC}"
    exit 1
fi
echo ""

STABLE_END=$(date +%s)
STABLE_ELAPSED=$((STABLE_END - START_TIME))
echo -e "${CYAN}Stable Rust validation: ${STABLE_ELAPSED}s${NC}"
echo ""

# ============================================================================
# CONFIGURATION 2: Nightly Rust (SIMD Optimized)
# ============================================================================
echo -e "${BLUE}[2/2] Configuration 2: Nightly Rust (SIMD Optimized)${NC}"
echo "Testing SIMD optimizations with portable_simd"
echo "=========================================="
echo ""

# Check nightly rust availability
if ! command -v rustup &> /dev/null; then
    echo -e "${YELLOW}⚠️  rustup not found, skipping nightly validation${NC}"
    NIGHTLY_AVAILABLE=0
else
    if rustup toolchain list | grep -q "nightly"; then
        NIGHTLY_VERSION=$(rustc +nightly --version 2>/dev/null || echo "unknown")
        echo "Nightly version: $NIGHTLY_VERSION"
        NIGHTLY_AVAILABLE=1
    else
        echo -e "${YELLOW}⚠️  Nightly toolchain not installed, skipping${NC}"
        NIGHTLY_AVAILABLE=0
    fi
fi
echo ""

if [ "$NIGHTLY_AVAILABLE" -eq 1 ]; then
    # Stage 2.1: Compilation check with SIMD
    echo -e "${YELLOW}[2.1] Checking compilation (nightly + SIMD)...${NC}"
    if cargo +nightly check --lib --features timeline-aggregation,portable_simd 2>&1 | tee ci_nightly_check.log | grep -q "Finished"; then
        WARNINGS=$(grep -c "warning:" ci_nightly_check.log || true)
        WORKSPACE_WARNINGS=$(grep -c "profiles for the non root package will be ignored" ci_nightly_check.log || true)
        CODE_WARNINGS=$((WARNINGS - WORKSPACE_WARNINGS))

        if [ "$CODE_WARNINGS" -eq 0 ]; then
            echo -e "${GREEN}✅ Nightly compilation: PASS (0 warnings)${NC}"
        else
            echo -e "${RED}❌ Nightly compilation: FAIL ($CODE_WARNINGS warnings)${NC}"
            exit 1
        fi
    else
        echo -e "${RED}❌ Nightly compilation: FAIL (errors)${NC}"
        exit 1
    fi
    echo ""

    # Stage 2.2: Clippy check with SIMD
    echo -e "${YELLOW}[2.2] Running clippy (nightly + SIMD)...${NC}"
    if cargo +nightly clippy --lib --features timeline-aggregation,portable_simd -- -D warnings 2>&1 | tee ci_nightly_clippy.log | tail -n 5; then
        echo -e "${GREEN}✅ Nightly clippy: PASS${NC}"
    else
        echo -e "${RED}❌ Nightly clippy: FAIL${NC}"
        exit 1
    fi
    echo ""

    # Stage 2.3: Benchmark compilation (no run, faster)
    echo -e "${YELLOW}[2.3] Compiling benchmarks (nightly + SIMD)...${NC}"
    if cargo +nightly bench --no-run --features timeline-aggregation,portable_simd 2>&1 | grep -q "Finished"; then
        echo -e "${GREEN}✅ Nightly benchmarks: COMPILE PASS${NC}"
    else
        echo -e "${RED}❌ Nightly benchmarks: COMPILE FAIL${NC}"
        exit 1
    fi
    echo ""

    NIGHTLY_END=$(date +%s)
    NIGHTLY_ELAPSED=$((NIGHTLY_END - STABLE_END))
    echo -e "${CYAN}Nightly Rust validation: ${NIGHTLY_ELAPSED}s${NC}"
else
    echo -e "${YELLOW}⚠️  Nightly validation skipped (toolchain not available)${NC}"
fi
echo ""

# ============================================================================
# Summary
# ============================================================================
END_TIME=$(date +%s)
TOTAL_ELAPSED=$((END_TIME - START_TIME))
TOTAL_MIN=$((TOTAL_ELAPSED / 60))
TOTAL_SEC=$((TOTAL_ELAPSED % 60))

echo "=========================================="
echo -e "${GREEN}✅ ALL CI VALIDATIONS PASSED${NC}"
echo "=========================================="
echo ""
echo -e "${CYAN}Configuration Summary:${NC}"
echo "  - Stable Rust:   ✅ PASS (${STABLE_ELAPSED}s)"
if [ "$NIGHTLY_AVAILABLE" -eq 1 ]; then
    echo "  - Nightly Rust:  ✅ PASS (${NIGHTLY_ELAPSED}s)"
else
    echo "  - Nightly Rust:  ⚠️  SKIP (not available)"
fi
echo ""
echo -e "${CYAN}Total Time: ${TOTAL_MIN}m ${TOTAL_SEC}s${NC}"

if [ "$TOTAL_ELAPSED" -lt 300 ]; then
    echo -e "${GREEN}✅ Under 5 minute target${NC}"
else
    echo -e "${YELLOW}⚠️  Exceeded 5 minute target by $((TOTAL_ELAPSED - 300))s${NC}"
fi
echo ""

echo -e "${CYAN}Logs Generated:${NC}"
echo "  - ci_stable_check.log"
echo "  - ci_stable_clippy.log"
if [ "$NIGHTLY_AVAILABLE" -eq 1 ]; then
    echo "  - ci_nightly_check.log"
    echo "  - ci_nightly_clippy.log"
fi
echo ""

echo -e "${CYAN}Pass/Fail Decision:${NC}"
echo -e "${GREEN}✅ CI VALIDATION PASSED${NC}"
echo "All required checks completed successfully"
echo ""
echo -e "${CYAN}Next Steps:${NC}"
echo "  1. Merge to main branch"
echo "  2. Run full test suite (cargo test --lib)"
echo "  3. Run benchmarks (cargo bench)"
echo "  4. Validate performance targets (B32 framework)"
echo "=========================================="

exit 0
