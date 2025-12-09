#!/bin/bash

# Migration Progress Tracker for Computational Capsule Verification
# Tracks the migration from manual macros to #[derive(ComputationalCapsule)]

echo "════════════════════════════════════════════════════════════════════════"
echo "           COMPUTATIONAL CAPSULE VERIFICATION MIGRATION TRACKER         "
echo "════════════════════════════════════════════════════════════════════════"
echo

SRC_DIR="${1:-src}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Count manual verification macros
VERIFY_PROPS=$(grep -r "verify_capsule_properties!" "$SRC_DIR" --include="*.rs" 2>/dev/null | wc -l)
VERIFY_ALIGN=$(grep -r "verify_alignment_only!" "$SRC_DIR" --include="*.rs" 2>/dev/null | wc -l)
MANUAL_SIZE=$(grep -r "assert_eq!(.*std::mem::size_of" "$SRC_DIR" --include="*.rs" 2>/dev/null | wc -l)
MANUAL_ALIGN=$(grep -r "assert_eq!(.*std::mem::align_of" "$SRC_DIR" --include="*.rs" 2>/dev/null | wc -l)

# Count derive usage
DERIVE_COUNT=$(grep -r "#\[derive.*ComputationalCapsule" "$SRC_DIR" --include="*.rs" 2>/dev/null | wc -l)

# Calculate totals
MANUAL_TOTAL=$((VERIFY_PROPS + VERIFY_ALIGN + MANUAL_SIZE + MANUAL_ALIGN))
TOTAL_CAPSULES=$((DERIVE_COUNT + MANUAL_TOTAL))

# Calculate progress
if [ $TOTAL_CAPSULES -gt 0 ]; then
    PROGRESS=$((DERIVE_COUNT * 100 / TOTAL_CAPSULES))
else
    PROGRESS=100
fi

# Display current state
echo -e "${BOLD}Current State:${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "✅ ${GREEN}$DERIVE_COUNT${NC} capsules using #[derive(ComputationalCapsule)]"
echo -e "❌ ${RED}$MANUAL_TOTAL${NC} manual verification sites remaining:"
echo -e "   • verify_capsule_properties!: ${YELLOW}$VERIFY_PROPS${NC}"
echo -e "   • verify_alignment_only!:     ${YELLOW}$VERIFY_ALIGN${NC}"
echo -e "   • manual size assertions:     ${YELLOW}$MANUAL_SIZE${NC}"
echo -e "   • manual align assertions:    ${YELLOW}$MANUAL_ALIGN${NC}"
echo

# Progress bar
echo -e "${BOLD}Migration Progress: ${PROGRESS}%${NC}"
echo -n "["
FILLED=$((PROGRESS / 2))
for ((i=0; i<50; i++)); do
    if [ $i -lt $FILLED ]; then
        echo -n "█"
    else
        echo -n "░"
    fi
done
echo "] $DERIVE_COUNT/$TOTAL_CAPSULES"
echo

# Priority breakdown (approximate based on file locations)
echo -e "${BOLD}Priority Breakdown:${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# P0: Protection & Core
P0_PROTECTION=$(grep -r "verify_.*!" "$SRC_DIR/protection" --include="*.rs" 2>/dev/null | wc -l)
P0_PATTERNS=$(grep -r "verify_.*!" "$SRC_DIR/patterns" --include="*.rs" 2>/dev/null | wc -l)
P0_TOTAL=$((P0_PROTECTION + P0_PATTERNS))

# P1: Active Development
P1_HTTP=$(grep -r "verify_.*!" "$SRC_DIR/http" --include="*.rs" 2>/dev/null | wc -l)
P1_CNLS=$(grep -r "verify_.*!" "$SRC_DIR/patterns/cnls" --include="*.rs" 2>/dev/null | wc -l)
P1_BTREE=$(grep -r "verify_.*!" "$SRC_DIR/collections/lockfree_btree" --include="*.rs" 2>/dev/null | wc -l)
P1_TOTAL=$((P1_HTTP + P1_CNLS + P1_BTREE))

# P2: Infrastructure
P2_PERSIST=$(grep -r "verify_.*!" "$SRC_DIR/persistence" --include="*.rs" 2>/dev/null | wc -l)
P2_COMPOSITE=$(grep -r "verify_.*!" "$SRC_DIR/composite" --include="*.rs" 2>/dev/null | wc -l)
P2_MMAP=$(grep -r "verify_.*!" "$SRC_DIR/mmap" --include="*.rs" 2>/dev/null | wc -l)
P2_TOTAL=$((P2_PERSIST + P2_COMPOSITE + P2_MMAP))

echo -e "${RED}P0 (Critical):${NC} $P0_TOTAL remaining"
echo -e "   • Protection system: $P0_PROTECTION"
echo -e "   • Core patterns: $P0_PATTERNS"
echo
echo -e "${YELLOW}P1 (Active Dev):${NC} $P1_TOTAL remaining"
echo -e "   • HTTP system: $P1_HTTP"
echo -e "   • CNLS quantum: $P1_CNLS"
echo -e "   • B-tree hybrid: $P1_BTREE"
echo
echo -e "${GREEN}P2 (Infrastructure):${NC} $P2_TOTAL remaining"
echo -e "   • Persistence: $P2_PERSIST"
echo -e "   • Composites: $P2_COMPOSITE"
echo -e "   • Memory mapping: $P2_MMAP"
echo

# Files with most manual verifications
echo -e "${BOLD}Top Files Needing Migration:${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${BLUE}File${NC} | ${BLUE}Count${NC}"
grep -r "verify_\(capsule_properties\|alignment_only\)!" "$SRC_DIR" --include="*.rs" 2>/dev/null | \
    cut -d: -f1 | sort | uniq -c | sort -rn | head -10 | \
    while read count file; do
        filename=$(basename "$file")
        dir=$(dirname "$file" | sed "s|$SRC_DIR/||")
        printf "%-40s %s\n" "$dir/$filename" "$count"
    done
echo

# Target metrics
echo -e "${BOLD}Target Metrics (v0.7.0):${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✓ Manual macros: 0 (currently: $MANUAL_TOTAL)"
echo "✓ Derive usage: ~215 (currently: $DERIVE_COUNT)"
echo "✓ Test pass rate: 100%"
echo "✓ Clippy warnings: 0"
echo "✓ Compile overhead: <20ms per capsule"
echo "✓ Runtime cost: 0ns"
echo "✓ Code reduction: 87.5%"
echo

# Recommendations
echo -e "${BOLD}Next Steps:${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [ $P0_TOTAL -gt 0 ]; then
    echo -e "1. ${RED}URGENT:${NC} Manually migrate $P0_TOTAL P0 critical capsules"
fi
if [ $P1_TOTAL -gt 0 ]; then
    echo -e "2. ${YELLOW}Semi-automate${NC} $P1_TOTAL P1 active development capsules"
fi
if [ $P2_TOTAL -gt 0 ]; then
    echo -e "3. ${GREEN}Automate${NC} $P2_TOTAL P2 infrastructure capsules"
fi
echo "4. Run: cargo test --all-features --lib"
echo "5. Run: cargo clippy -- -D warnings"
echo

# Save report
REPORT_FILE="migration_report_$(date +%Y%m%d_%H%M%S).txt"
{
    echo "Migration Report - $(date)"
    echo "========================="
    echo "Derive Count: $DERIVE_COUNT"
    echo "Manual Remaining: $MANUAL_TOTAL"
    echo "  - verify_capsule_properties: $VERIFY_PROPS"
    echo "  - verify_alignment_only: $VERIFY_ALIGN"
    echo "  - manual assertions: $((MANUAL_SIZE + MANUAL_ALIGN))"
    echo "Progress: $PROGRESS%"
} > "$REPORT_FILE"

echo "Report saved to: $REPORT_FILE"
