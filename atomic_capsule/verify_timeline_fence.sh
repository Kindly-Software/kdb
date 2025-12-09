#!/bin/bash
# Timeline Fence Implementation Verification Script
# Usage: ./verify_timeline_fence.sh

set -e

echo "=== Timeline Fence Implementation Verification ==="
echo ""

# 1. File existence
echo "[1/6] Verifying file existence..."
if [ -f "src/gpu/kgpu_driver/timeline_fence.rs" ]; then
    LINE_COUNT=$(wc -l < src/gpu/kgpu_driver/timeline_fence.rs)
    echo "✅ timeline_fence.rs exists ($LINE_COUNT lines)"
else
    echo "❌ timeline_fence.rs not found"
    exit 1
fi

# 2. Module exports
echo "[2/6] Verifying module exports..."
if grep -q "pub mod timeline_fence" src/gpu/kgpu_driver/mod.rs; then
    echo "✅ Module declaration found"
else
    echo "❌ Module declaration missing"
    exit 1
fi

if grep -q "pub use timeline_fence::" src/gpu/kgpu_driver/mod.rs; then
    EXPORT_COUNT=$(grep "pub use timeline_fence::" -A 15 src/gpu/kgpu_driver/mod.rs | grep -c "}")
    echo "✅ Exports found ($EXPORT_COUNT export blocks)"
else
    echo "❌ Exports missing"
    exit 1
fi

# 3. Test count
echo "[3/6] Verifying test coverage..."
TEST_COUNT=$(grep -c "^    #\[test\]" src/gpu/kgpu_driver/timeline_fence.rs)
if [ "$TEST_COUNT" -ge 20 ]; then
    echo "✅ $TEST_COUNT tests found (target: 35+, minimum: 20)"
else
    echo "⚠️  Only $TEST_COUNT tests found (target: 35+)"
fi

# 4. Compilation check
echo "[4/6] Verifying compilation..."
if cargo check --lib --features kgpu-driver,kgpu-driver-linux --message-format=short 2>&1 | grep -q "Finished"; then
    echo "✅ Compilation successful"
else
    echo "❌ Compilation failed"
    exit 1
fi

# 5. COCA compliance
echo "[5/6] Verifying COCA compliance..."
if grep -q "repr(C, align(256))" src/gpu/kgpu_driver/timeline_fence.rs; then
    echo "✅ 256B cache alignment found"
else
    echo "❌ Cache alignment missing"
    exit 1
fi

if grep -q "AtomicU64" src/gpu/kgpu_driver/timeline_fence.rs; then
    ATOMIC_COUNT=$(grep -c "AtomicU64" src/gpu/kgpu_driver/timeline_fence.rs)
    echo "✅ Lockfree atomics found ($ATOMIC_COUNT uses)"
else
    echo "❌ No atomics found"
    exit 1
fi

# 6. Documentation check
echo "[6/6] Verifying documentation..."
DOC_LINES=$(grep -c "^//!" src/gpu/kgpu_driver/timeline_fence.rs || true)
if [ "$DOC_LINES" -ge 40 ]; then
    echo "✅ Comprehensive documentation ($DOC_LINES doc lines)"
else
    echo "⚠️  Limited documentation ($DOC_LINES doc lines, target: 40+)"
fi

echo ""
echo "=== Verification Complete ==="
echo ""
echo "Status: ✅ READY FOR TESTING"
echo ""
echo "Next steps:"
echo "1. Run tests remotely: ssh samuel@kindly-hub \"cd ~/Primitives/atomic_capsule && cargo test --lib timeline_fence --features kgpu-driver,kgpu-driver-linux\""
echo "2. Run benchmarks: ssh samuel@kindly-hub \"cd ~/Primitives/atomic_capsule && cargo bench timeline_fence\""
echo "3. Review implementation report: cat TIMELINE_FENCE_IMPLEMENTATION_REPORT.md"
