#!/bin/bash
# Verification script for monitoring implementation

set -e

echo "=========================================="
echo "Monitoring Implementation Verification"
echo "=========================================="
echo ""

CHECKS=0
PASS=0
FAIL=0

check() {
    local name=$1
    local command=$2
    
    CHECKS=$((CHECKS + 1))
    echo -n "[$CHECKS] $name ... "
    
    if eval "$command" > /dev/null 2>&1; then
        echo "✅ PASS"
        PASS=$((PASS + 1))
    else
        echo "❌ FAIL"
        FAIL=$((FAIL + 1))
    fi
}

# Verify Rust module
check "ObservabilityCapsule module" \
    "test -f /home/samuel/Primitives/atomic_capsule/src/http/observability.rs"

check "observability.rs (480+ lines)" \
    "test $(wc -l < /home/samuel/Primitives/atomic_capsule/src/http/observability.rs) -gt 480"

check "http/mod.rs exports observability" \
    "grep -q 'pub mod observability' /home/samuel/Primitives/atomic_capsule/src/http/mod.rs"

check "http/mod.rs re-exports types" \
    "grep -q 'pub use observability' /home/samuel/Primitives/atomic_capsule/src/http/mod.rs"

# Verify scripts
check "monitor.sh exists" \
    "test -f /home/samuel/Primitives/scripts/monitor.sh"

check "monitor.sh is executable" \
    "test -x /home/samuel/Primitives/scripts/monitor.sh"

check "alert.sh exists" \
    "test -f /home/samuel/Primitives/scripts/alert.sh"

check "alert.sh is executable" \
    "test -x /home/samuel/Primitives/scripts/alert.sh"

check "dashboard.sh exists" \
    "test -f /home/samuel/Primitives/scripts/dashboard.sh"

check "dashboard.sh is executable" \
    "test -x /home/samuel/Primitives/scripts/dashboard.sh"

# Verify configuration
check "Cron config exists" \
    "test -f /home/samuel/Primitives/config/atomic-capsule-monitor.cron"

check "Cron config has monitor job" \
    "grep -q 'monitor.sh' /home/samuel/Primitives/config/atomic-capsule-monitor.cron"

check "Cron config has alert job" \
    "grep -q 'alert.sh' /home/samuel/Primitives/config/atomic-capsule-monitor.cron"

# Verify documentation
check "MONITORING_SETUP.md exists" \
    "test -f /home/samuel/Primitives/docs/MONITORING_SETUP.md"

check "MONITORING_QUICK_REFERENCE.md exists" \
    "test -f /home/samuel/Primitives/docs/MONITORING_QUICK_REFERENCE.md"

check "MONITORING_IMPLEMENTATION_SUMMARY.md exists" \
    "test -f /home/samuel/Primitives/MONITORING_IMPLEMENTATION_SUMMARY.md"

# Verify logs directory
check "logs directory exists" \
    "test -d /home/samuel/Primitives/logs"

# Verify compilation
check "atomic_capsule builds successfully" \
    "cd /home/samuel/Primitives/atomic_capsule && cargo build --lib --features 'std' 2>&1 | grep -q 'Finished'"

echo ""
echo "=========================================="
echo "Summary: $PASS/$CHECKS checks passed"
echo "=========================================="

if [ $FAIL -gt 0 ]; then
    echo ""
    echo "⚠️  $FAIL checks failed - review above"
    exit 1
else
    echo ""
    echo "✅ All checks passed!"
    echo ""
    echo "📊 Deliverables Summary:"
    echo "  - ObservabilityCapsule Rust module (T1+T4)"
    echo "  - 3 monitoring scripts (monitor, alert, dashboard)"
    echo "  - Cron job configuration"
    echo "  - Comprehensive documentation"
    echo ""
    echo "📝 Next steps:"
    echo "1. View quick reference: cat /home/samuel/Primitives/docs/MONITORING_QUICK_REFERENCE.md"
    echo "2. Test monitor script: /home/samuel/Primitives/scripts/monitor.sh"
    echo "3. Test alert script: /home/samuel/Primitives/scripts/alert.sh"
    echo "4. View dashboard: /home/samuel/Primitives/scripts/dashboard.sh once"
    echo "5. Install cron jobs: sudo cp /home/samuel/Primitives/config/atomic-capsule-monitor.cron /etc/cron.d/"
    echo "6. Read full guide: cat /home/samuel/Primitives/docs/MONITORING_SETUP.md"
    exit 0
fi
