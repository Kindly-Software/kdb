#!/bin/bash
# Miri Validation Script for AtomicCapsuleMap
# Validates undefined behavior, data races, and memory safety
#
# Usage: ./scripts/run_miri.sh [test_name]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=====================================${NC}"
echo -e "${BLUE}AtomicCapsuleMap Miri Validation${NC}"
echo -e "${BLUE}=====================================${NC}"
echo ""

# Check if nightly is installed
if ! rustup toolchain list | grep -q "nightly"; then
    echo -e "${YELLOW}Installing nightly toolchain...${NC}"
    rustup install nightly
fi

# Check if Miri is installed
if ! rustup component list --toolchain nightly | grep -q "miri.*installed"; then
    echo -e "${YELLOW}Installing Miri component...${NC}"
    rustup +nightly component add miri
fi

echo -e "${GREEN}Toolchain ready${NC}"
echo ""

# Set up Miri flags for comprehensive checking
export MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-strict-provenance"

# If test name provided, run specific test
if [ ! -z "$1" ]; then
    echo -e "${BLUE}Running specific test: $1${NC}"
    cargo +nightly miri test "$1" -- --nocapture
    exit 0
fi

# Run all test categories with Miri
echo -e "${BLUE}[1/5] Running unit tests under Miri...${NC}"
cargo +nightly miri test --lib 2>&1 | tee miri_unit.log
UNIT_RESULT=${PIPESTATUS[0]}

echo ""
echo -e "${BLUE}[2/5] Running basic integration tests under Miri...${NC}"
cargo +nightly miri test --test basic_tests 2>&1 | tee miri_basic.log
BASIC_RESULT=${PIPESTATUS[0]}

echo ""
echo -e "${BLUE}[3/5] Running atomic operations tests under Miri...${NC}"
cargo +nightly miri test --test atomic_ops_tests 2>&1 | tee miri_atomic.log
ATOMIC_RESULT=${PIPESTATUS[0]}

echo ""
echo -e "${BLUE}[4/5] Running generation counter tests under Miri...${NC}"
cargo +nightly miri test --test generation_tests 2>&1 | tee miri_generation.log
GENERATION_RESULT=${PIPESTATUS[0]}

echo ""
echo -e "${BLUE}[5/5] Running iteration tests under Miri...${NC}"
cargo +nightly miri test --test iteration_tests 2>&1 | tee miri_iteration.log
ITERATION_RESULT=${PIPESTATUS[0]}

# Note: Skip concurrent and stress tests as they're too slow under Miri
echo ""
echo -e "${YELLOW}Note: Skipping concurrent/stress tests (too slow under Miri)${NC}"
echo -e "${YELLOW}These are validated by ThreadSanitizer instead${NC}"

# Summary
echo ""
echo -e "${BLUE}=====================================${NC}"
echo -e "${BLUE}Miri Validation Summary${NC}"
echo -e "${BLUE}=====================================${NC}"
echo ""

ALL_PASSED=true

if [ $UNIT_RESULT -eq 0 ]; then
    echo -e "Unit tests:        ${GREEN}PASSED${NC}"
else
    echo -e "Unit tests:        ${RED}FAILED${NC}"
    ALL_PASSED=false
fi

if [ $BASIC_RESULT -eq 0 ]; then
    echo -e "Basic tests:       ${GREEN}PASSED${NC}"
else
    echo -e "Basic tests:       ${RED}FAILED${NC}"
    ALL_PASSED=false
fi

if [ $ATOMIC_RESULT -eq 0 ]; then
    echo -e "Atomic ops tests:  ${GREEN}PASSED${NC}"
else
    echo -e "Atomic ops tests:  ${RED}FAILED${NC}"
    ALL_PASSED=false
fi

if [ $GENERATION_RESULT -eq 0 ]; then
    echo -e "Generation tests:  ${GREEN}PASSED${NC}"
else
    echo -e "Generation tests:  ${RED}FAILED${NC}"
    ALL_PASSED=false
fi

if [ $ITERATION_RESULT -eq 0 ]; then
    echo -e "Iteration tests:   ${GREEN}PASSED${NC}"
else
    echo -e "Iteration tests:   ${RED}FAILED${NC}"
    ALL_PASSED=false
fi

echo ""

if $ALL_PASSED; then
    echo -e "${GREEN}✅ All Miri tests PASSED${NC}"
    echo -e "${GREEN}No undefined behavior detected${NC}"
    exit 0
else
    echo -e "${RED}❌ Some Miri tests FAILED${NC}"
    echo -e "${RED}Review logs above for undefined behavior${NC}"
    exit 1
fi
