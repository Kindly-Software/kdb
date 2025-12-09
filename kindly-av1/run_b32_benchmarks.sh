#!/usr/bin/env bash
#
# B32 Comprehensive Encoder Benchmarks - kindly-av1 vs SVT-AV1
#
# [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
#
# B32 Framework Compliance:
# - MANDATORY execution on kindly-hub (consistent hardware)
# - 95% confidence intervals
# - 1000+ iterations
# - Fair baselines (SVT-AV1 1.7.0)
# - HTML reports generated
#
# Usage:
#   ./run_b32_benchmarks.sh [category]
#
# Categories:
#   all          - Run all benchmarks (default)
#   frame        - Frame encoding benchmarks
#   component    - Component benchmarks
#   end-to-end   - End-to-end sequence benchmarks
#   quality      - Quality tradeoff benchmarks
#

set -euo pipefail

# ============================================================================
# Configuration
# ============================================================================

# Remote execution on kindly-hub (MANDATORY for B32)
REMOTE_HOST="kindly-hub"
REMOTE_USER="samuel"
PROJECT_PATH="~/Primitives/kindly-av1"

# Benchmark category (default: all)
CATEGORY="${1:-all}"

# Hardware info (for report)
HARDWARE="AMD Ryzen 9 6900HX, 64GB DDR5-4800"

# ============================================================================
# Color Output
# ============================================================================

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*"
}

warning() {
    echo -e "${YELLOW}[WARNING]${NC} $*"
}

error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

header() {
    echo -e "${PURPLE}[BENCHMARK]${NC} $*"
}

# ============================================================================
# Helper Functions
# ============================================================================

check_remote_connection() {
    info "Checking connection to ${REMOTE_HOST}..."
    if ! ssh "${REMOTE_USER}@${REMOTE_HOST}" "echo 'Connection successful'" &>/dev/null; then
        error "Cannot connect to ${REMOTE_HOST}. Please check SSH configuration."
        exit 1
    fi
    success "Connection to ${REMOTE_HOST} verified"
}

check_lsyncd_sync() {
    info "Checking lsyncd sync status..."
    if journalctl --user -u lsyncd -n 20 | grep -q "ERROR"; then
        warning "lsyncd may have sync errors. Check: journalctl --user -u lsyncd -n 20"
    else
        success "lsyncd sync appears healthy"
    fi
}

verify_hardware() {
    info "Verifying hardware on ${REMOTE_HOST}..."
    local cpu_model
    cpu_model=$(ssh "${REMOTE_USER}@${REMOTE_HOST}" "lscpu | grep 'Model name' | cut -d':' -f2 | xargs")
    local mem_total
    mem_total=$(ssh "${REMOTE_USER}@${REMOTE_HOST}" "free -h | grep 'Mem:' | awk '{print \$2}'")

    info "Hardware: ${cpu_model}"
    info "Memory: ${mem_total}"

    if [[ ! "$cpu_model" =~ "Ryzen 9 6900HX" ]]; then
        warning "Expected AMD Ryzen 9 6900HX but found: ${cpu_model}"
    fi
}

run_remote_bench() {
    local bench_args="$1"
    local description="$2"

    header "${description}"
    info "Executing: cargo bench --bench encoder_bench ${bench_args}"

    # Run benchmark on kindly-hub
    ssh "${REMOTE_USER}@${REMOTE_HOST}" "cd ${PROJECT_PATH} && cargo bench --bench encoder_bench ${bench_args}"

    if [[ $? -eq 0 ]]; then
        success "Benchmark completed: ${description}"
    else
        error "Benchmark failed: ${description}"
        return 1
    fi
}

generate_summary_report() {
    info "Generating benchmark summary report..."

    # Fetch Criterion HTML reports (stored in target/criterion/)
    local report_dir="./benchmark_reports_$(date +%Y%m%d_%H%M%S)"
    mkdir -p "${report_dir}"

    info "Fetching Criterion HTML reports from ${REMOTE_HOST}..."
    scp -r "${REMOTE_USER}@${REMOTE_HOST}:${PROJECT_PATH}/target/criterion" "${report_dir}/" || true

    if [[ -d "${report_dir}/criterion" ]]; then
        success "HTML reports saved to: ${report_dir}/criterion"
        info "Open: ${report_dir}/criterion/report/index.html"
    else
        warning "No HTML reports found (may be first run)"
    fi
}

# ============================================================================
# Benchmark Execution
# ============================================================================

run_all_benchmarks() {
    header "B32 COMPREHENSIVE ENCODER BENCHMARKS"
    info "Hardware: ${HARDWARE}"
    info "Category: ${CATEGORY}"
    info "Remote Host: ${REMOTE_USER}@${REMOTE_HOST}"
    echo ""

    # Check prerequisites
    check_remote_connection
    check_lsyncd_sync
    verify_hardware
    echo ""

    case "${CATEGORY}" in
        all)
            info "Running ALL benchmark categories..."
            run_remote_bench "-- frame_encoding" "Frame Encoding Benchmarks"
            run_remote_bench "-- components" "Component Benchmarks"
            run_remote_bench "-- end_to_end" "End-to-End Sequence Benchmarks"
            run_remote_bench "-- quality" "Quality Tradeoff Benchmarks"
            ;;
        frame)
            info "Running FRAME ENCODING benchmarks..."
            run_remote_bench "-- frame_encoding" "Frame Encoding Benchmarks"
            ;;
        component)
            info "Running COMPONENT benchmarks..."
            run_remote_bench "-- components" "Component Benchmarks"
            ;;
        end-to-end)
            info "Running END-TO-END benchmarks..."
            run_remote_bench "-- end_to_end" "End-to-End Sequence Benchmarks"
            ;;
        quality)
            info "Running QUALITY TRADEOFF benchmarks..."
            run_remote_bench "-- quality" "Quality Tradeoff Benchmarks"
            ;;
        *)
            error "Unknown category: ${CATEGORY}"
            echo "Valid categories: all, frame, component, end-to-end, quality"
            exit 1
            ;;
    esac

    echo ""
    generate_summary_report
    echo ""
    success "B32 benchmarks complete!"
}

# ============================================================================
# Main Execution
# ============================================================================

main() {
    run_all_benchmarks
}

main "$@"
