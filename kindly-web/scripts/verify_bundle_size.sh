#!/bin/bash
# Bundle Size Verification Script
# T28 Q24: Performance regression testing
# Target: <380KB gzipped WASM bundle

set -e

echo "===== Bundle Size Verification ====="
echo "Building release bundle..."

# Build release version
trunk build --release

# Check if build directory exists
if [ ! -d "dist" ]; then
    echo "ERROR: dist/ directory not found. Build may have failed."
    exit 1
fi

# Find WASM files
WASM_FILES=$(find dist -name "*.wasm" -type f)

if [ -z "$WASM_FILES" ]; then
    echo "ERROR: No WASM files found in dist/"
    exit 1
fi

echo ""
echo "===== WASM Bundle Sizes ====="

# Analyze each WASM file
for file in $WASM_FILES; do
    echo ""
    echo "File: $file"

    # Uncompressed size
    SIZE_BYTES=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file" 2>/dev/null)
    SIZE_KB=$((SIZE_BYTES / 1024))
    echo "  Uncompressed: ${SIZE_KB} KB (${SIZE_BYTES} bytes)"

    # Gzipped size
    GZIP_BYTES=$(gzip -c "$file" | wc -c)
    GZIP_KB=$((GZIP_BYTES / 1024))
    echo "  Gzipped:      ${GZIP_KB} KB (${GZIP_BYTES} bytes)"

    # Check against target (<380KB gzipped)
    TARGET_KB=380
    if [ $GZIP_KB -lt $TARGET_KB ]; then
        echo "  ✅ PASS: Under ${TARGET_KB}KB target"
    else
        echo "  ❌ FAIL: Exceeds ${TARGET_KB}KB target"
        exit 1
    fi

    # Compression ratio
    RATIO=$(awk "BEGIN {printf \"%.1f\", 100 * (1 - $GZIP_BYTES / $SIZE_BYTES)}")
    echo "  Compression:  ${RATIO}%"
done

echo ""
echo "===== JS Bundle Sizes ====="

# Find JS files
JS_FILES=$(find dist -name "*.js" -type f)

for file in $JS_FILES; do
    echo ""
    echo "File: $file"

    SIZE_BYTES=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file" 2>/dev/null)
    SIZE_KB=$((SIZE_BYTES / 1024))
    echo "  Uncompressed: ${SIZE_KB} KB"

    GZIP_BYTES=$(gzip -c "$file" | wc -c)
    GZIP_KB=$((GZIP_BYTES / 1024))
    echo "  Gzipped:      ${GZIP_KB} KB"
done

echo ""
echo "===== Total Bundle Size ====="

# Total uncompressed
TOTAL_BYTES=$(find dist -type f \( -name "*.wasm" -o -name "*.js" \) -exec stat -c%s {} + 2>/dev/null | awk '{s+=$1} END {print s}')
if [ -z "$TOTAL_BYTES" ]; then
    # macOS fallback
    TOTAL_BYTES=$(find dist -type f \( -name "*.wasm" -o -name "*.js" \) -exec stat -f%z {} + 2>/dev/null | awk '{s+=$1} END {print s}')
fi
TOTAL_KB=$((TOTAL_BYTES / 1024))
echo "Total uncompressed: ${TOTAL_KB} KB"

# Total gzipped (approximate - sum of individual gzips)
TOTAL_GZIP=0
for file in $WASM_FILES $JS_FILES; do
    GZIP_BYTES=$(gzip -c "$file" | wc -c)
    TOTAL_GZIP=$((TOTAL_GZIP + GZIP_BYTES))
done
TOTAL_GZIP_KB=$((TOTAL_GZIP / 1024))
echo "Total gzipped:      ${TOTAL_GZIP_KB} KB"

echo ""
echo "===== Verification Complete ====="
echo "✅ All bundle size targets met"

exit 0
