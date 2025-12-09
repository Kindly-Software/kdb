#!/bin/bash
# LockfreeBTree Compilation and Verification Script
# Phase 11.0
# Run after Implementation Experts complete file creation

set -e  # Exit on error

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
PASSED=0
FAILED=0
WARNINGS=0

echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}LockfreeBTree Compilation and Verification${NC}"
echo -e "${BLUE}Phase 11.0 - atomic_capsule${NC}"
echo -e "${BLUE}================================================${NC}"
echo ""

# Function to print test result
print_result() {
    local test_name="$1"
    local status="$2"

    if [ "$status" = "PASS" ]; then
        echo -e "${GREEN}✓${NC} $test_name: ${GREEN}PASS${NC}"
        ((PASSED++))
    elif [ "$status" = "FAIL" ]; then
        echo -e "${RED}✗${NC} $test_name: ${RED}FAIL${NC}"
        ((FAILED++))
    elif [ "$status" = "WARN" ]; then
        echo -e "${YELLOW}⚠${NC} $test_name: ${YELLOW}WARNING${NC}"
        ((WARNINGS++))
    fi
}

# 1. Check implementation files exist
echo -e "${BLUE}[1/10] Checking implementation files...${NC}"
FILES=(
    "src/collections/lockfree_btree/mod.rs"
    "src/collections/lockfree_btree/node.rs"
    "src/collections/lockfree_btree/types.rs"
    "src/collections/lockfree_btree/stats.rs"
)

all_files_exist=true
for file in "${FILES[@]}"; do
    if [ -f "$file" ]; then
        echo -e "  ${GREEN}✓${NC} Found: $file"
    else
        echo -e "  ${RED}✗${NC} Missing: $file"
        all_files_exist=false
    fi
done

if [ "$all_files_exist" = true ]; then
    print_result "Implementation files" "PASS"
else
    print_result "Implementation files" "FAIL"
    echo -e "${RED}ERROR: Implementation files missing. Cannot continue.${NC}"
    exit 1
fi

echo ""

# 2. Check feature flag in Cargo.toml
echo -e "${BLUE}[2/10] Checking feature flags...${NC}"
if grep -q "lockfree-btree" Cargo.toml; then
    echo -e "  ${GREEN}✓${NC} Found: lockfree-btree feature flag"
    print_result "Feature flags" "PASS"
else
    echo -e "  ${RED}✗${NC} Missing: lockfree-btree feature flag"
    print_result "Feature flags" "FAIL"
    echo -e "${YELLOW}  Apply LOCKFREE_BTREE_CARGO_PATCH.md to add feature flags${NC}"
fi

echo ""

# 3. Compilation Matrix
echo -e "${BLUE}[3/10] Testing compilation matrix...${NC}"

# 3.1 No features
echo -e "  Testing: no features..."
if cargo build --lib --no-default-features 2>&1 | grep -q "error"; then
    print_result "Compile: no features" "FAIL"
else
    print_result "Compile: no features" "PASS"
fi

# 3.2 lockfree-btree only
echo -e "  Testing: lockfree-btree..."
if cargo build --lib --features lockfree-btree 2>&1 | grep -q "error"; then
    print_result "Compile: lockfree-btree" "FAIL"
else
    print_result "Compile: lockfree-btree" "PASS"
fi

# 3.3 collections (if defined)
if grep -q "^collections = " Cargo.toml; then
    echo -e "  Testing: collections..."
    if cargo build --lib --features collections 2>&1 | grep -q "error"; then
        print_result "Compile: collections" "FAIL"
    else
        print_result "Compile: collections" "PASS"
    fi
fi

# 3.4 all-features
echo -e "  Testing: all features..."
if cargo build --lib --all-features 2>&1 | grep -q "error"; then
    print_result "Compile: all features" "FAIL"
else
    print_result "Compile: all features" "PASS"
fi

echo ""

# 4. Clippy verification
echo -e "${BLUE}[4/10] Running clippy verification...${NC}"
clippy_output=$(cargo clippy --features lockfree-btree -- \
    -D clippy::missing_capsule_verification \
    -D warnings 2>&1 || true)

warning_count=$(echo "$clippy_output" | grep -c "warning:" || echo "0")
error_count=$(echo "$clippy_output" | grep -c "error:" || echo "0")

echo -e "  Warnings: $warning_count"
echo -e "  Errors: $error_count"

if [ "$error_count" -eq 0 ] && [ "$warning_count" -eq 0 ]; then
    print_result "Clippy verification" "PASS"
else
    print_result "Clippy verification" "FAIL"
    echo -e "${RED}  Clippy output:${NC}"
    echo "$clippy_output"
fi

echo ""

# 5. Capsule verification check
echo -e "${BLUE}[5/10] Checking capsule verification...${NC}"

# Check if BTreeNode uses derive
if grep -r "derive(ComputationalCapsule)" src/collections/lockfree_btree/ | grep -q "BTreeNode"; then
    echo -e "  ${GREEN}✓${NC} BTreeNode: #[derive(ComputationalCapsule)] found"
    print_result "BTreeNode verification" "PASS"
else
    echo -e "  ${RED}✗${NC} BTreeNode: Missing #[derive(ComputationalCapsule)]"
    print_result "BTreeNode verification" "FAIL"
fi

# Check if BTreeStatsCapsule uses derive
if grep -r "derive(ComputationalCapsule)" src/collections/lockfree_btree/ | grep -q "BTreeStatsCapsule"; then
    echo -e "  ${GREEN}✓${NC} BTreeStatsCapsule: #[derive(ComputationalCapsule)] found"
    print_result "BTreeStatsCapsule verification" "PASS"
else
    echo -e "  ${RED}✗${NC} BTreeStatsCapsule: Missing #[derive(ComputationalCapsule)]"
    print_result "BTreeStatsCapsule verification" "FAIL"
fi

echo ""

# 6. Test compilation
echo -e "${BLUE}[6/10] Compiling tests...${NC}"
if cargo test --lib --features lockfree-btree --no-run 2>&1 | grep -q "error"; then
    print_result "Test compilation" "FAIL"
else
    print_result "Test compilation" "PASS"
fi

echo ""

# 7. Run tests
echo -e "${BLUE}[7/10] Running tests...${NC}"
test_output=$(cargo test --lib --features lockfree-btree 2>&1 || true)

if echo "$test_output" | grep -q "test result: ok"; then
    test_count=$(echo "$test_output" | grep -oP '\d+(?= passed)' | head -1)
    echo -e "  Tests passed: ${test_count}"
    print_result "Test execution" "PASS"
else
    echo -e "  ${RED}Some tests failed${NC}"
    print_result "Test execution" "FAIL"
    echo "$test_output" | grep -A 5 "failures:"
fi

echo ""

# 8. Benchmark compilation
echo -e "${BLUE}[8/10] Compiling benchmarks...${NC}"
if [ -f "benches/lockfree_btree_bench.rs" ]; then
    if cargo bench --bench lockfree_btree_bench --no-run 2>&1 | grep -q "error"; then
        print_result "Benchmark compilation" "FAIL"
    else
        print_result "Benchmark compilation" "PASS"
    fi
else
    echo -e "  ${YELLOW}⚠${NC} Benchmark file not found (optional)"
    print_result "Benchmark compilation" "WARN"
fi

echo ""

# 9. Build time measurements
echo -e "${BLUE}[9/10] Measuring build times...${NC}"

# Clean build
echo -e "  Measuring clean build time..."
cargo clean
start_time=$(date +%s)
cargo build --lib --features lockfree-btree >/dev/null 2>&1
end_time=$(date +%s)
clean_time=$((end_time - start_time))
echo -e "  Clean build: ${clean_time}s"

if [ "$clean_time" -lt 120 ]; then
    print_result "Clean build time (<2 min)" "PASS"
else
    print_result "Clean build time (<2 min)" "WARN"
fi

# Incremental build
echo -e "  Measuring incremental build time..."
touch src/collections/lockfree_btree/mod.rs
start_time=$(date +%s)
cargo build --lib --features lockfree-btree >/dev/null 2>&1
end_time=$(date +%s)
incr_time=$((end_time - start_time))
echo -e "  Incremental build: ${incr_time}s"

if [ "$incr_time" -lt 10 ]; then
    print_result "Incremental build time (<10s)" "PASS"
else
    print_result "Incremental build time (<10s)" "WARN"
fi

echo ""

# 10. Module exports check
echo -e "${BLUE}[10/10] Checking module exports...${NC}"

if grep -q "lockfree_btree" src/collections/mod.rs; then
    echo -e "  ${GREEN}✓${NC} Module declared in mod.rs"

    if grep -q "pub use lockfree_btree::" src/collections/mod.rs; then
        echo -e "  ${GREEN}✓${NC} Exports found in mod.rs"
        print_result "Module exports" "PASS"
    else
        echo -e "  ${RED}✗${NC} Exports missing in mod.rs"
        print_result "Module exports" "FAIL"
    fi
else
    echo -e "  ${RED}✗${NC} Module not declared in mod.rs"
    print_result "Module exports" "FAIL"
fi

echo ""

# Summary
echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}Verification Summary${NC}"
echo -e "${BLUE}================================================${NC}"
echo -e "${GREEN}Passed:${NC}   $PASSED"
echo -e "${YELLOW}Warnings:${NC} $WARNINGS"
echo -e "${RED}Failed:${NC}   $FAILED"
echo ""

# Overall result
if [ "$FAILED" -eq 0 ]; then
    if [ "$WARNINGS" -eq 0 ]; then
        echo -e "${GREEN}✓ ALL CHECKS PASSED${NC}"
        echo -e "${GREEN}Phase 11.0 LockfreeBTree is production-ready!${NC}"
        exit 0
    else
        echo -e "${YELLOW}⚠ PASSED WITH WARNINGS${NC}"
        echo -e "${YELLOW}Review warnings before production deployment.${NC}"
        exit 0
    fi
else
    echo -e "${RED}✗ VERIFICATION FAILED${NC}"
    echo -e "${RED}Fix errors before proceeding.${NC}"
    exit 1
fi
