#!/bin/bash
# Sanitizer Validation Script for AtomicCapsuleMap
# Runs ThreadSanitizer, AddressSanitizer, and LeakSanitizer
#
# Usage: ./scripts/run_sanitizers.sh [tsan|asan|lsan|all]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=====================================${NC}"
echo -e "${BLUE}AtomicCapsuleMap Sanitizer Validation${NC}"
echo -e "${BLUE}=====================================${NC}"
echo ""

# Check if nightly is installed
if ! rustup toolchain list | grep -q "nightly"; then
    echo -e "${YELLOW}Installing nightly toolchain...${NC}"
    rustup install nightly
fi

# Determine target
TARGET="x86_64-unknown-linux-gnu"

# Add target if not already added
rustup target add "$TARGET" --toolchain nightly || true

echo -e "${GREEN}Toolchain ready${NC}"
echo ""

# Function to run ThreadSanitizer
run_tsan() {
    echo -e "${BLUE}=====================================${NC}"
    echo -e "${BLUE}Running ThreadSanitizer (TSan)${NC}"
    echo -e "${BLUE}=====================================${NC}"
    echo ""

    echo -e "${YELLOW}Detects: Data races, lock ordering violations${NC}"
    echo ""

    # Build with ThreadSanitizer
    RUSTFLAGS="-Z sanitizer=thread" cargo +nightly test \
        --target "$TARGET" \
        --tests \
        2>&1 | tee tsan_output.log

    TSAN_RESULT=${PIPESTATUS[0]}

    echo ""
    if [ $TSAN_RESULT -eq 0 ]; then
        echo -e "${GREEN}✅ ThreadSanitizer: PASSED (no data races detected)${NC}"
    else
        echo -e "${RED}❌ ThreadSanitizer: FAILED (data races detected)${NC}"
        echo -e "${RED}See tsan_output.log for details${NC}"
    fi
    echo ""

    return $TSAN_RESULT
}

# Function to run AddressSanitizer
run_asan() {
    echo -e "${BLUE}=====================================${NC}"
    echo -e "${BLUE}Running AddressSanitizer (ASan)${NC}"
    echo -e "${BLUE}=====================================${NC}"
    echo ""

    echo -e "${YELLOW}Detects: Buffer overflows, use-after-free, memory leaks${NC}"
    echo ""

    # Build with AddressSanitizer
    ASAN_OPTIONS=detect_leaks=1 RUSTFLAGS="-Z sanitizer=address" cargo +nightly test \
        --target "$TARGET" \
        --tests \
        2>&1 | tee asan_output.log

    ASAN_RESULT=${PIPESTATUS[0]}

    echo ""
    if [ $ASAN_RESULT -eq 0 ]; then
        echo -e "${GREEN}✅ AddressSanitizer: PASSED (no memory errors detected)${NC}"
    else
        echo -e "${RED}❌ AddressSanitizer: FAILED (memory errors detected)${NC}"
        echo -e "${RED}See asan_output.log for details${NC}"
    fi
    echo ""

    return $ASAN_RESULT
}

# Function to run LeakSanitizer
run_lsan() {
    echo -e "${BLUE}=====================================${NC}"
    echo -e "${BLUE}Running LeakSanitizer (LSan)${NC}"
    echo -e "${BLUE}=====================================${NC}"
    echo ""

    echo -e "${YELLOW}Detects: Memory leaks${NC}"
    echo ""

    # Build with LeakSanitizer
    RUSTFLAGS="-Z sanitizer=leak" cargo +nightly test \
        --target "$TARGET" \
        --tests \
        2>&1 | tee lsan_output.log

    LSAN_RESULT=${PIPESTATUS[0]}

    echo ""
    if [ $LSAN_RESULT -eq 0 ]; then
        echo -e "${GREEN}✅ LeakSanitizer: PASSED (no memory leaks detected)${NC}"
    else
        echo -e "${RED}❌ LeakSanitizer: FAILED (memory leaks detected)${NC}"
        echo -e "${RED}See lsan_output.log for details${NC}"
    fi
    echo ""

    return $LSAN_RESULT
}

# Main execution
SANITIZER="${1:-all}"

TSAN_PASSED=true
ASAN_PASSED=true
LSAN_PASSED=true

case "$SANITIZER" in
    tsan)
        run_tsan || TSAN_PASSED=false
        ;;
    asan)
        run_asan || ASAN_PASSED=false
        ;;
    lsan)
        run_lsan || LSAN_PASSED=false
        ;;
    all)
        echo -e "${BLUE}Running all sanitizers...${NC}"
        echo ""

        run_tsan || TSAN_PASSED=false
        run_asan || ASAN_PASSED=false
        run_lsan || LSAN_PASSED=false
        ;;
    *)
        echo -e "${RED}Unknown sanitizer: $SANITIZER${NC}"
        echo "Usage: $0 [tsan|asan|lsan|all]"
        exit 1
        ;;
esac

# Final summary
echo -e "${BLUE}=====================================${NC}"
echo -e "${BLUE}Sanitizer Validation Summary${NC}"
echo -e "${BLUE}=====================================${NC}"
echo ""

ALL_PASSED=true

if [ "$SANITIZER" == "all" ] || [ "$SANITIZER" == "tsan" ]; then
    if $TSAN_PASSED; then
        echo -e "ThreadSanitizer:   ${GREEN}PASSED${NC}"
    else
        echo -e "ThreadSanitizer:   ${RED}FAILED${NC}"
        ALL_PASSED=false
    fi
fi

if [ "$SANITIZER" == "all" ] || [ "$SANITIZER" == "asan" ]; then
    if $ASAN_PASSED; then
        echo -e "AddressSanitizer:  ${GREEN}PASSED${NC}"
    else
        echo -e "AddressSanitizer:  ${RED}FAILED${NC}"
        ALL_PASSED=false
    fi
fi

if [ "$SANITIZER" == "all" ] || [ "$SANITIZER" == "lsan" ]; then
    if $LSAN_PASSED; then
        echo -e "LeakSanitizer:     ${GREEN}PASSED${NC}"
    else
        echo -e "LeakSanitizer:     ${RED}FAILED${NC}"
        ALL_PASSED=false
    fi
fi

echo ""

if $ALL_PASSED; then
    echo -e "${GREEN}✅ All sanitizers PASSED${NC}"
    echo -e "${GREEN}No memory safety violations detected${NC}"
    exit 0
else
    echo -e "${RED}❌ Some sanitizers FAILED${NC}"
    echo -e "${RED}Review logs above for memory safety violations${NC}"
    exit 1
fi
