#!/bin/bash
# Quick error categorization for atomic_mcp_server compilation

cd /home/samuel/Primitives/atomic_mcp_server

echo "========================================="
echo "Compilation Error Analysis"
echo "Timestamp: $(date)"
echo "========================================="
echo ""

# Count total errors
TOTAL_ERRORS=$(cargo test --all-features --no-run 2>&1 | grep -c "^error\[")
echo "Total Errors: $TOTAL_ERRORS"
echo ""

# Categorize by error code
echo "Errors by Type:"
cargo test --all-features --no-run 2>&1 | grep "^error\[" | sort | uniq -c | sort -rn | head -20
echo ""

# Find affected files
echo "Most Affected Files:"
cargo test --all-features --no-run 2>&1 | grep "^  --> " | sed 's/.*--> //' | cut -d: -f1 | sort | uniq -c | sort -rn | head -20
echo ""

# Check if compilation would succeed
if [ $TOTAL_ERRORS -eq 0 ]; then
    echo "✓ Compilation: SUCCESS"
    exit 0
else
    echo "✗ Compilation: FAILED ($TOTAL_ERRORS errors)"
    exit 1
fi
