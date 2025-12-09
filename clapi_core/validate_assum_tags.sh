#!/bin/bash
# ASSUM Tag Validation Script for Phase 2
# Validates that all #ASSUME tags have corresponding #VERIFY tags

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=== ASSUM Tag Validation Script ==="
echo ""

# Find all Rust source files in src/proxy
PROXY_DIR="src/proxy"

if [ ! -d "$PROXY_DIR" ]; then
    echo -e "${YELLOW}Warning: $PROXY_DIR not found (Phase 2 not implemented yet)${NC}"
    echo ""
    echo "This script will validate ASSUM tags once Phase 2 implementation begins."
    echo ""
    echo "Expected ASSUM tags in Phase 2:"
    echo "  - #ASSUME_TOKIO_SAFE + #VERIFY_NO_BLOCKING"
    echo "  - #ASSUME_TIMEOUT_SUFFICIENT + #VERIFY_TIMEOUT_TUNED"
    echo "  - #ASSUME_GRACEFUL_SHUTDOWN + #VERIFY_SHUTDOWN_CLEAN"
    echo "  - #ASSUME_BUDGET_ATOMIC + #VERIFY_BUDGET_CORRECTNESS"
    echo "  - #ASSUME_DASHMAP_LOCKFREE + #VERIFY_DASHMAP_PERFORMANCE"
    echo "  - #ASSUME_TOCTOU_ELIMINATED + #VERIFY_ABA_FREE"
    echo "  - #ASSUME_SERDE_SAFE + #VERIFY_SERDE_FUZZ"
    echo "  - #ASSUME_NO_JSON_BOMB + #VERIFY_SIZE_LIMIT"
    echo "  - #ASSUME_ERROR_HANDLED + #VERIFY_NO_PANIC"
    echo "  - #ASSUME_BUDGET_REFUNDED + #VERIFY_REFUND_CORRECTNESS"
    echo "  - #ASSUME_AUDIT_LOGGED + #VERIFY_AUDIT_COMPLETE"
    echo "  - #ASSUME_WAL_CRASH_SAFE + #VERIFY_CRASH_RECOVERY"
    echo "  - #ASSUME_APPEND_ONLY + #VERIFY_IMMUTABILITY"
    echo "  - #ASSUME_CLIENT_THREAD_SAFE + #VERIFY_THREAD_SAFE"
    echo "  - #ASSUME_POOL_EXHAUSTION_HANDLED + #VERIFY_POOL_LIMITS"
    echo "  - #ASSUME_TIMEOUT_ENFORCED + #VERIFY_TIMEOUT_FIRES"
    echo ""
    echo "Run this script after implementing Phase 2 components."
    exit 0
fi

# Count ASSUM tags
echo "Counting ASSUM tags in $PROXY_DIR..."
ASSUME_COUNT=$(grep -r "#ASSUME_" "$PROXY_DIR" --include="*.rs" | wc -l)
VERIFY_COUNT=$(grep -r "#VERIFY_" "$PROXY_DIR" --include="*.rs" | wc -l)

echo "  #ASSUME tags found: $ASSUME_COUNT"
echo "  #VERIFY tags found: $VERIFY_COUNT"
echo ""

# Check for orphan ASSUME tags (no corresponding VERIFY)
echo "Checking for orphan #ASSUME tags (no #VERIFY)..."

ORPHANS=0
while IFS= read -r file; do
    # Extract ASSUME tags from file
    ASSUME_TAGS=$(grep "#ASSUME_" "$file" | sed -E 's/.*#(ASSUME_[A-Z_]+).*/\1/')

    for assume_tag in $ASSUME_TAGS; do
        # Convert ASSUME to VERIFY
        verify_tag="${assume_tag/ASSUME_/VERIFY_}"

        # Check if VERIFY exists in same file or nearby files
        if ! grep -q "#$verify_tag" "$file"; then
            echo -e "  ${RED}✗${NC} $file: $assume_tag missing corresponding $verify_tag"
            ORPHANS=$((ORPHANS + 1))
        fi
    done
done < <(find "$PROXY_DIR" -name "*.rs")

if [ $ORPHANS -eq 0 ]; then
    echo -e "  ${GREEN}✓${NC} All #ASSUME tags have corresponding #VERIFY tags"
else
    echo -e "  ${RED}✗${NC} Found $ORPHANS orphan #ASSUME tags"
fi
echo ""

# Check for orphan VERIFY tags (no corresponding ASSUME)
echo "Checking for orphan #VERIFY tags (no #ASSUME)..."

VERIFY_ORPHANS=0
while IFS= read -r file; do
    # Extract VERIFY tags from file
    VERIFY_TAGS=$(grep "#VERIFY_" "$file" | sed -E 's/.*#(VERIFY_[A-Z_]+).*/\1/')

    for verify_tag in $VERIFY_TAGS; do
        # Convert VERIFY to ASSUME
        assume_tag="${verify_tag/VERIFY_/ASSUME_}"

        # Check if ASSUME exists in same file
        if ! grep -q "#$assume_tag" "$file"; then
            echo -e "  ${YELLOW}⚠${NC} $file: $verify_tag missing corresponding $assume_tag"
            VERIFY_ORPHANS=$((VERIFY_ORPHANS + 1))
        fi
    done
done < <(find "$PROXY_DIR" -name "*.rs")

if [ $VERIFY_ORPHANS -eq 0 ]; then
    echo -e "  ${GREEN}✓${NC} All #VERIFY tags have corresponding #ASSUME tags"
else
    echo -e "  ${YELLOW}⚠${NC} Found $VERIFY_ORPHANS orphan #VERIFY tags"
fi
echo ""

# Summary
echo "=== Validation Summary ==="
if [ $ORPHANS -eq 0 ] && [ $VERIFY_ORPHANS -eq 0 ]; then
    echo -e "${GREEN}✓ PASS${NC}: All ASSUM tags properly paired"
    exit 0
else
    echo -e "${RED}✗ FAIL${NC}: Found unpaired ASSUM tags"
    echo "  Orphan #ASSUME tags: $ORPHANS"
    echo "  Orphan #VERIFY tags: $VERIFY_ORPHANS"
    exit 1
fi
