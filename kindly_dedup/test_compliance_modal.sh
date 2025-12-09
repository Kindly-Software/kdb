#!/usr/bin/env bash
# Test script for Enterprise Compliance Dashboard modal UX fixes (Phase 4)
# Usage: ./test_compliance_modal.sh

set -euo pipefail

echo "=================================================="
echo "Enterprise Compliance Dashboard - Phase 4 Testing"
echo "=================================================="
echo ""

# Check if binary exists
BINARY="./target/release/kindly_dedup_iced"
if [[ ! -f "$BINARY" ]]; then
    echo "❌ Binary not found: $BINARY"
    echo "Building release binary..."
    cargo build --bin kindly_dedup_iced --features gui-iced --release
    echo "✅ Build complete"
fi

echo "🚀 Launching GUI..."
echo ""
echo "TESTING CHECKLIST:"
echo "=================="
echo ""
echo "Visual Verification:"
echo "  1. Click 'Enterprise Grade' badge (top row, left)"
echo "  2. Verify ALL text is centered:"
echo "     - Modal header: 'Enterprise Compliance Dashboard'"
echo "     - Section headers: 'Compliance Standards', 'Audit Trail Status'"
echo "     - Status items: SOX, SOC2, GDPR, HIPAA, Chain Integrity, Audit Events"
echo "     - Verify Integrity button (centered horizontally)"
echo "     - Bottom text: 'BLAKE3 hash-chained...'"
echo "     - Button row: Export Report, Close (centered as group)"
echo ""
echo "Functional Verification:"
echo "  3. Click 'Verify Integrity' button"
echo "     - Check console: Should log 'Chain verification: INTACT'"
echo "     - Check timestamp: Should update to 'Last verified: 0 seconds ago'"
echo "  4. Click 'Export Report' button"
echo "     - Check console: Should log 'Export report requested'"
echo "  5. Click 'Close' button"
echo "     - Modal should close, return to main screen"
echo ""
echo "Expected Console Output:"
echo "========================"
echo "[Compliance] Chain verification: INTACT (N events verified)"
echo "[Compliance] Export report requested (PDF generation coming soon)"
echo ""
echo "Press Ctrl+C to exit test"
echo "=================================================="
echo ""

# Launch GUI with console output visible
"$BINARY" 2>&1 | tee compliance_modal_test.log
