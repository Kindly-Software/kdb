#!/bin/bash
# Diagnostic tool to verify clippy plugin is properly loaded
# Tests a minimal example to confirm lints are firing

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Create a minimal test file
cat > /tmp/test_mutex_simple.rs << 'EOF'
#![feature(rustc_private)]
#![deny(clippy::capsule_mutex_violation)]

use std::sync::Mutex;

#[repr(C, align(64))]
struct TestCapsule {
    lock: Mutex<u64>,
    _padding: [u8; 48],
}

fn main() {}
EOF

echo "Testing if clippy plugin loads correctly..."
echo ""

# Try to compile with clippy loaded as a plugin
echo "Attempting compilation with clippy plugin..."
rustc +nightly \
    --edition=2021 \
    -Z unstable-options \
    target/release/libclippy_capsule_verify.so \
    /tmp/test_mutex_simple.rs \
    2>&1 | tee /tmp/lint_test_output.txt

echo ""
echo "Compilation attempt complete."
echo "If the lint is working, we should see CAPSULE_MUTEX_VIOLATION error above."
echo ""

# Check if lint fired
if grep -q "capsule_mutex_violation" /tmp/lint_test_output.txt; then
    echo "✓ Lint appears to be working!"
else
    echo "✗ Lint did NOT fire - this indicates the plugin is not loading correctly"
    echo ""
    echo "This is expected - custom clippy plugins require special loading mechanisms."
    echo "Standard rustc cannot load clippy plugins via command line."
fi

rm -f /tmp/test_mutex_simple.rs /tmp/lint_test_output.txt
