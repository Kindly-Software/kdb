#!/bin/bash
# Coverage Dashboard (P1 E9)
# Enforce 80% minimum code coverage using cargo tarpaulin
#
# Usage:
#   ./scripts/coverage.sh         # Run and generate HTML report
#   ./scripts/coverage.sh --ci    # Run in CI mode (fail if <80%)
#
# Performance Targets:
# - Coverage analysis: <5 minutes for full codebase
# - HTML report generation: <10 seconds
# - CI-friendly output (exit code 0/1)

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
MIN_COVERAGE=80.0
OUTPUT_DIR="target/coverage"
REPORT_HTML="$OUTPUT_DIR/index.html"

# Parse arguments
CI_MODE=false
if [[ "$1" == "--ci" ]]; then
    CI_MODE=true
fi

# Check if tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${RED}Error: cargo-tarpaulin not installed${NC}"
    echo "Install with: cargo install cargo-tarpaulin"
    exit 1
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

echo -e "${GREEN}=== Running Coverage Analysis ===${NC}"
echo "Minimum coverage threshold: ${MIN_COVERAGE}%"
echo "Output directory: $OUTPUT_DIR"
echo ""

# Run tarpaulin
# - --skip-clean: Reuse build cache (faster)
# - --out Html: Generate HTML report
# - --out Xml: Generate Codecov/Coveralls compatible report
# - --engine llvm: Use LLVM-based coverage (more accurate)
# - --exclude-files 'tests/*': Exclude test files from coverage
# - --timeout 300: 5 minute timeout per test
if [ "$CI_MODE" = true ]; then
    echo -e "${YELLOW}Running in CI mode (strict enforcement)${NC}"

    # CI mode: JSON output for parsing, fail on low coverage
    cargo tarpaulin \
        --skip-clean \
        --out Html \
        --out Json \
        --output-dir "$OUTPUT_DIR" \
        --engine llvm \
        --exclude-files 'tests/*' 'benches/*' \
        --timeout 300 \
        --fail-under "$MIN_COVERAGE" \
        --color always

    EXIT_CODE=$?

    # Parse JSON to get exact coverage
    if [ -f "$OUTPUT_DIR/tarpaulin-report.json" ]; then
        COVERAGE=$(jq -r '.coverage' "$OUTPUT_DIR/tarpaulin-report.json")
        echo ""
        echo -e "${GREEN}=== Coverage Report ===${NC}"
        echo "Total coverage: ${COVERAGE}%"
        echo "Threshold: ${MIN_COVERAGE}%"

        # Check threshold
        if (( $(echo "$COVERAGE < $MIN_COVERAGE" | bc -l) )); then
            echo -e "${RED}FAIL: Coverage ${COVERAGE}% below threshold ${MIN_COVERAGE}%${NC}"
            exit 1
        else
            echo -e "${GREEN}PASS: Coverage ${COVERAGE}% meets threshold${NC}"
        fi
    fi

    exit $EXIT_CODE
else
    # Development mode: HTML report only, don't fail
    cargo tarpaulin \
        --skip-clean \
        --out Html \
        --output-dir "$OUTPUT_DIR" \
        --engine llvm \
        --exclude-files 'tests/*' 'benches/*' \
        --timeout 300 \
        --color always

    echo ""
    echo -e "${GREEN}=== Coverage Report Generated ===${NC}"
    echo "HTML report: file://$PWD/$REPORT_HTML"
    echo ""
    echo "To open in browser:"
    echo "  firefox $REPORT_HTML"
    echo "  xdg-open $REPORT_HTML"
fi
