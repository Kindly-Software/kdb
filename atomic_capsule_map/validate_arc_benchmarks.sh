#!/bin/bash
# Arc<T> Benchmark Validation Script
# Checks that all benchmarks compile and provides quick smoke test

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "========================================="
echo "Arc<T> Benchmark Validation"
echo "========================================="
echo

# 1. Check compilation
echo -e "${YELLOW}[1/5]${NC} Checking benchmark compilation..."
if cargo check --benches --quiet 2>&1 | grep -q "error"; then
    echo -e "${RED}✗ Compilation errors found${NC}"
    cargo check --benches
    exit 1
else
    echo -e "${GREEN}✓ All benchmarks compile${NC}"
fi
echo

# 2. Verify benchmark registration
echo -e "${YELLOW}[2/5]${NC} Verifying benchmark registration in Cargo.toml..."
if grep -q "name = \"arc_ops\"" Cargo.toml; then
    echo -e "${GREEN}✓ arc_ops registered${NC}"
else
    echo -e "${RED}✗ arc_ops not found in Cargo.toml${NC}"
    exit 1
fi

if grep -q "name = \"arc_performance\"" Cargo.toml; then
    echo -e "${GREEN}✓ arc_performance registered${NC}"
else
    echo -e "${RED}✗ arc_performance not found in Cargo.toml${NC}"
    exit 1
fi
echo

# 3. Check benchmark file exists
echo -e "${YELLOW}[3/5]${NC} Checking benchmark files exist..."
if [ -f "benches/arc_ops.rs" ]; then
    LINES=$(wc -l < benches/arc_ops.rs)
    echo -e "${GREEN}✓ arc_ops.rs exists ($LINES lines)${NC}"
else
    echo -e "${RED}✗ benches/arc_ops.rs not found${NC}"
    exit 1
fi

if [ -f "benches/arc_performance.rs" ]; then
    LINES=$(wc -l < benches/arc_performance.rs)
    echo -e "${GREEN}✓ arc_performance.rs exists ($LINES lines)${NC}"
else
    echo -e "${RED}✗ benches/arc_performance.rs not found${NC}"
    exit 1
fi
echo

# 4. List all benchmark groups
echo -e "${YELLOW}[4/5]${NC} Listing benchmark groups in arc_ops.rs..."
echo "Found benchmark functions:"
grep -E "^fn bench_" benches/arc_ops.rs | sed 's/fn /  - /' | sed 's/(.*$//'
echo

# 5. System check
echo -e "${YELLOW}[5/5]${NC} System readiness check..."

# Check CPU governor
if command -v cpupower &> /dev/null; then
    GOVERNOR=$(cpupower frequency-info -p 2>/dev/null | grep "governor" | awk '{print $NF}' | tr -d '"')
    if [ "$GOVERNOR" = "performance" ]; then
        echo -e "${GREEN}✓ CPU governor: performance${NC}"
    else
        echo -e "${YELLOW}⚠ CPU governor: $GOVERNOR (recommend 'performance')${NC}"
        echo "  Run: sudo cpupower frequency-set -g performance"
    fi
else
    echo -e "${YELLOW}⚠ cpupower not found (optional)${NC}"
fi

# Check system load
LOAD=$(uptime | awk -F'load average:' '{print $2}' | awk '{print $1}' | tr -d ',')
LOAD_INT=$(echo "$LOAD" | cut -d. -f1)
if [ "$LOAD_INT" -lt 2 ]; then
    echo -e "${GREEN}✓ System load: $LOAD (good)${NC}"
else
    echo -e "${YELLOW}⚠ System load: $LOAD (high - may affect results)${NC}"
fi

# Check available memory
MEM_FREE=$(free -g | grep "^Mem:" | awk '{print $7}')
if [ "$MEM_FREE" -gt 4 ]; then
    echo -e "${GREEN}✓ Free memory: ${MEM_FREE}GB${NC}"
else
    echo -e "${YELLOW}⚠ Free memory: ${MEM_FREE}GB (recommend >4GB)${NC}"
fi

echo
echo "========================================="
echo -e "${GREEN}Validation Complete${NC}"
echo "========================================="
echo
echo "Next steps:"
echo "1. Run quick test: cargo bench --bench arc_ops -- --quick"
echo "2. Run full suite: cargo bench --bench arc_ops"
echo "3. View results: firefox target/criterion/report/index.html"
echo
echo "For detailed instructions, see: RUN_ARC_BENCHMARKS.md"
