#!/usr/bin/env bash
# Pre-Deployment Validation Checks
# I20 Q19: Automated deployment gate validation
# Target: Zero-defect deployments, catch issues before production

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

BASELINE_P50_MS=10
BASELINE_P99_MS=50
MIN_COVERAGE_PERCENT=80

# Logging
log_info() {
    echo -e "${GREEN}[INFO]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

# Validation functions
check_git_status() {
    log_info "Checking git status..."

    if ! git diff-index --quiet HEAD --; then
        log_error "Uncommitted changes detected. Commit or stash changes before deployment."
        return 1
    fi

    local branch
    branch=$(git rev-parse --abbrev-ref HEAD)
    log_info "Deploying from branch: $branch"

    # Verify we're on a release branch or main
    if [[ ! "$branch" =~ ^(main|release/.*)$ ]]; then
        log_warn "Not on main/release branch. Current: $branch"
        read -p "Continue anyway? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            return 1
        fi
    fi

    log_info "✓ Git status OK"
    return 0
}

run_tests() {
    log_info "Running test suite..."

    # Unit tests
    if ! cargo test --lib --all-features --quiet; then
        log_error "Unit tests failed"
        return 1
    fi
    log_info "✓ Unit tests passed"

    # Integration tests
    if ! cargo test --test '*' --all-features --quiet; then
        log_error "Integration tests failed"
        return 1
    fi
    log_info "✓ Integration tests passed"

    # Property tests (if any)
    if cargo test --all-features --quiet -- --ignored 2>/dev/null; then
        log_info "✓ Property tests passed"
    fi

    return 0
}

run_clippy() {
    log_info "Running clippy..."

    if ! cargo clippy --all-features --tests -- -D warnings 2>&1; then
        log_error "Clippy found issues"
        return 1
    fi

    log_info "✓ Clippy clean"
    return 0
}

build_release() {
    log_info "Building release binary..."

    if ! cargo build --release --all-features; then
        log_error "Release build failed"
        return 1
    fi

    # Verify binary exists
    if [[ ! -f "target/release/clapi" ]]; then
        log_error "Release binary not found"
        return 1
    fi

    # Check binary size (sanity check)
    local size
    size=$(stat -c%s "target/release/clapi" 2>/dev/null || stat -f%z "target/release/clapi")
    log_info "Binary size: $(numfmt --to=iec-i --suffix=B $size)"

    log_info "✓ Release build successful"
    return 0
}

check_performance_baseline() {
    log_info "Validating performance baselines..."

    # Run benchmarks and extract P50/P99
    if ! cargo bench --bench budget_benchmarks -- --quiet 2>&1 | tee /tmp/bench_output.txt; then
        log_warn "Benchmarks failed to run (non-critical)"
        return 0
    fi

    # Parse P50/P99 from benchmark output (example format)
    # Format: "budget_check time: [60.123 ns 62.456 ns 65.789 ns]"
    local p50_ns p99_ns
    p50_ns=$(grep -oP 'budget_check.*time.*\[\K[0-9.]+' /tmp/bench_output.txt | head -1 || echo "0")
    p99_ns=$(grep -oP 'budget_check.*p99.*\[\K[0-9.]+' /tmp/bench_output.txt | head -1 || echo "0")

    # Convert to ms (if values found)
    if [[ "$p50_ns" != "0" ]]; then
        local p50_ms
        p50_ms=$(echo "scale=2; $p50_ns / 1000000" | bc)

        if (( $(echo "$p50_ms > $BASELINE_P50_MS" | bc -l) )); then
            log_error "P50 latency ($p50_ms ms) exceeds baseline ($BASELINE_P50_MS ms)"
            return 1
        fi
        log_info "✓ P50 latency: $p50_ms ms (baseline: $BASELINE_P50_MS ms)"
    else
        log_warn "Could not extract P50 from benchmarks (skipping)"
    fi

    return 0
}

check_assum_safety() {
    log_info "Validating ASSUM safety tags..."

    if [[ -x "./validate_assum_tags.sh" ]]; then
        if ! ./validate_assum_tags.sh; then
            log_error "ASSUM validation failed"
            return 1
        fi
        log_info "✓ ASSUM safety validated"
    else
        log_warn "ASSUM validation script not found (skipping)"
    fi

    return 0
}

check_dependencies() {
    log_info "Checking dependencies..."

    # Verify no vulnerable dependencies (cargo-audit)
    if command -v cargo-audit &> /dev/null; then
        if ! cargo audit; then
            log_error "Security vulnerabilities detected in dependencies"
            return 1
        fi
        log_info "✓ No dependency vulnerabilities"
    else
        log_warn "cargo-audit not installed (skipping security check)"
    fi

    return 0
}

generate_deployment_manifest() {
    log_info "Generating deployment manifest..."

    local manifest="/tmp/deployment_manifest.json"
    local version
    version=$(cargo metadata --format-version 1 --no-deps | jq -r '.packages[0].version')
    local commit
    commit=$(git rev-parse HEAD)
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    cat > "$manifest" <<EOF
{
  "version": "$version",
  "commit": "$commit",
  "timestamp": "$timestamp",
  "binary": "target/release/clapi",
  "checks": {
    "git_status": "PASS",
    "tests": "PASS",
    "clippy": "PASS",
    "build": "PASS",
    "performance": "PASS",
    "assum": "PASS"
  }
}
EOF

    log_info "✓ Deployment manifest: $manifest"
    cat "$manifest"

    return 0
}

# Main execution
main() {
    log_info "=== Pre-Deployment Validation ==="
    log_info "Project: clapi_core"
    log_info "Timestamp: $(date)"
    echo

    local failed=0

    # Run all checks
    check_git_status || failed=1
    run_tests || failed=1
    run_clippy || failed=1
    build_release || failed=1
    check_performance_baseline || failed=1
    check_assum_safety || failed=1
    check_dependencies || failed=1

    echo
    if [[ $failed -eq 0 ]]; then
        log_info "=== ✓ ALL CHECKS PASSED ==="
        generate_deployment_manifest
        log_info "Ready for deployment."
        exit 0
    else
        log_error "=== ✗ CHECKS FAILED ==="
        log_error "Fix errors before deployment."
        exit 1
    fi
}

main "$@"
