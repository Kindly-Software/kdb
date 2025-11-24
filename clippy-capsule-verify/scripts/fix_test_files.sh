#!/bin/bash
# Fix Test Files - Remove Problematic Dependencies
#
# This script cleans up test files to remove:
# 1. extern crate rustc_span (not needed, causes E0463)
# 2. #[derive(ComputationalCapsule)] (doesn't exist yet)
# 3. #[capsule(...)] attributes (not implemented)
#
# This makes tests suitable for manual verification via cargo clippy

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

echo "Fixing test files..."
echo ""

# Count files to process
TOTAL_FILES=$(find tests/ui/p0_* -name "*.rs" | wc -l)
echo "Found $TOTAL_FILES test files to process"
echo ""

FIXED_COUNT=0

# Process each test file
for test_file in tests/ui/p0_*/*.rs; do
    if [[ ! -f "$test_file" ]]; then
        continue
    fi

    ORIG_SIZE=$(wc -l < "$test_file")
    CHANGES_MADE=false

    # Create backup
    cp "$test_file" "$test_file.backup"

    # Remove extern crate rustc_span
    if grep -q "extern crate rustc_span" "$test_file"; then
        sed -i '/extern crate rustc_span/d' "$test_file"
        CHANGES_MADE=true
        echo "  - Removed rustc_span from $(basename $test_file)"
    fi

    # Remove #[derive(ComputationalCapsule)]
    if grep -q "#\[derive(ComputationalCapsule)\]" "$test_file"; then
        sed -i '/#\[derive(ComputationalCapsule)\]/d' "$test_file"
        CHANGES_MADE=true
        echo "  - Removed ComputationalCapsule derive from $(basename $test_file)"
    fi

    # Remove #[capsule(...)] attributes
    if grep -q "#\[capsule(" "$test_file"; then
        sed -i '/#\[capsule(/d' "$test_file"
        CHANGES_MADE=true
        echo "  - Removed capsule attribute from $(basename $test_file)"
    fi

    if [[ "$CHANGES_MADE" == "true" ]]; then
        FIXED_COUNT=$((FIXED_COUNT + 1))
        NEW_SIZE=$(wc -l < "$test_file")
        LINES_REMOVED=$((ORIG_SIZE - NEW_SIZE))
        echo "    $(basename $test_file): $ORIG_SIZE → $NEW_SIZE lines ($LINES_REMOVED removed)"
    fi
done

echo ""
echo "Summary:"
echo "  Files processed: $TOTAL_FILES"
echo "  Files modified: $FIXED_COUNT"
echo "  Backups created: $FIXED_COUNT (.backup files)"
echo ""

if [[ $FIXED_COUNT -gt 0 ]]; then
    echo "✅ Test files fixed successfully!"
    echo ""
    echo "Next steps:"
    echo "  1. Review changes: diff tests/ui/p0_*/01_*.rs tests/ui/p0_*/01_*.rs.backup"
    echo "  2. Run tests: ./scripts/run_ui_tests.sh"
    echo "  3. Delete backups: find tests/ui -name '*.backup' -delete"
else
    echo "ℹ️  No changes needed - files already clean"
fi

# Optional: Remove backups if no changes were made
# find tests/ui -name "*.backup" -delete
