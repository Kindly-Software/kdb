#!/usr/bin/env bash
#
# B32 Benchmark Runner for kindly-av1
#
# [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
#
# Runs all B32-compliant benchmarks on kindly-hub (192.168.0.38)
# with statistical rigor, fair baselines, and reproducibility.
#
# Usage:
#   ./run_b32_benchmarks.sh [OPTIONS]
#
# Options:
#   --all              Run all benchmarks
#   --bench <name>     Run specific benchmark (gpu_motion, encoding, transform, entropy, loop_filter)
#   --html             Generate HTML reports (Criterion default)
#   --compare <name>   Compare with baseline encoder (svt-av1, rav1e, libaom)
#   --flamegraph       Generate flamegraph for profiling
#   --help             Show this help message

set -euo pipefail

REMOTE_HOST="samuel@kindly-hub"
REMOTE_DIR="~/Primitives/kindly-av1"
BENCHMARKS=("gpu_motion" "encoding" "transform" "entropy" "loop_filter")

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Branding
echo -e "${PURPLE}[kindly-av1]${NC} B32 Benchmark Runner"
echo -e "${PURPLE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# Parse arguments
RUN_ALL=false
BENCH_NAME=""
GENERATE_HTML=false
COMPARE_WITH=""
FLAMEGRAPH=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --all)
            RUN_ALL=true
            shift
            ;;
        --bench)
            BENCH_NAME="$2"
            shift 2
            ;;
        --html)
            GENERATE_HTML=true
            shift
            ;;
        --compare)
            COMPARE_WITH="$2"
            shift 2
            ;;
        --flamegraph)
            FLAMEGRAPH=true
            shift
            ;;
        --help)
            grep '^#' "$0" | sed 's/^# //' | head -n 20
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Check remote host availability
echo -e "${YELLOW}[1/4] Checking remote host availability...${NC}"
if ! ssh -o ConnectTimeout=5 "$REMOTE_HOST" "echo 'Connected to kindly-hub'" > /dev/null 2>&1; then
    echo -e "${RED}ERROR: Cannot connect to kindly-hub (192.168.0.38)${NC}"
    echo -e "${RED}Ensure SSH is configured and kindly-hub is reachable${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Connected to kindly-hub${NC}"

# Sync code to remote (via lsyncd or manual rsync)
echo -e "${YELLOW}[2/4] Syncing code to kindly-hub...${NC}"
# lsyncd handles auto-sync, verify sync status
if systemctl --user is-active lsyncd > /dev/null 2>&1; then
    echo -e "${GREEN}✓ lsyncd is running (auto-sync enabled)${NC}"
else
    echo -e "${YELLOW}WARNING: lsyncd not running, using manual rsync${NC}"
    rsync -avz --exclude target --exclude .git "$PWD/" "$REMOTE_HOST:$REMOTE_DIR/"
fi

# Run benchmarks
echo -e "${YELLOW}[3/4] Running benchmarks on kindly-hub...${NC}"

if [ "$RUN_ALL" = true ]; then
    echo -e "${PURPLE}Running all benchmarks (this may take 10-30 minutes)${NC}"
    ssh "$REMOTE_HOST" "cd $REMOTE_DIR && cargo bench"
elif [ -n "$BENCH_NAME" ]; then
    echo -e "${PURPLE}Running benchmark: ${BENCH_NAME}_bench${NC}"
    ssh "$REMOTE_HOST" "cd $REMOTE_DIR && cargo bench --bench ${BENCH_NAME}_bench"
else
    echo -e "${RED}ERROR: Must specify --all or --bench <name>${NC}"
    exit 1
fi

# Generate reports
echo -e "${YELLOW}[4/4] Generating reports...${NC}"

if [ "$GENERATE_HTML" = true ]; then
    echo -e "${PURPLE}HTML reports available at: target/criterion/*/report/index.html${NC}"
fi

if [ "$FLAMEGRAPH" = true ]; then
    echo -e "${PURPLE}Generating flamegraph...${NC}"
    ssh "$REMOTE_HOST" "cd $REMOTE_DIR && cargo flamegraph --release --bench gpu_motion_bench"
    scp "$REMOTE_HOST:$REMOTE_DIR/flamegraph.svg" ./flamegraph.svg
    echo -e "${GREEN}✓ Flamegraph saved to flamegraph.svg${NC}"
fi

if [ -n "$COMPARE_WITH" ]; then
    echo -e "${PURPLE}Comparing with baseline: $COMPARE_WITH${NC}"
    # Comparison logic to be implemented (requires baseline benchmarks)
    echo -e "${YELLOW}WARNING: Comparison feature not yet implemented${NC}"
fi

echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}✓ Benchmarks complete!${NC}"
echo -e "${PURPLE}[kindly-av1]${NC} Review results in target/criterion/"
