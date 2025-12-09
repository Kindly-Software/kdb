#!/usr/bin/env bash
#
# Add license protection to all benchmark files
#
# Usage: ./scripts/add_benchmark_protection.sh
#

set -euo pipefail

BENCHES_DIR="/home/samuel/Primitives/kindly_dedup/benches"
PROTECTION_MODULE="benchmark_protection.rs"

# List of benchmark files to protect (excluding baselines and protection module itself)
BENCHMARK_FILES=$(find "$BENCHES_DIR" -name "*.rs" -type f ! -name "$PROTECTION_MODULE" ! -path "*/baselines/*" | sort)

# Count files
TOTAL_FILES=$(echo "$BENCHMARK_FILES" | wc -l)

echo "=== Adding License Protection to Benchmarks ==="
echo "Total files to modify: $TOTAL_FILES"
echo ""

# Counter
COUNT=0

# Process each file
for FILE in $BENCHMARK_FILES; do
    COUNT=$((COUNT + 1))
    BASENAME=$(basename "$FILE")
    BENCHMARK_NAME="${BASENAME%.rs}"

    echo "[$COUNT/$TOTAL_FILES] Processing: $BASENAME"

    # Calculate relative path to protection module
    REL_PATH=$(python3 -c "import os.path; print(os.path.relpath('$BENCHES_DIR/$PROTECTION_MODULE', os.path.dirname('$FILE')))")

    # Check if already protected
    if grep -q "require_valid_license" "$FILE"; then
        echo "  ⏭️  Already protected, skipping"
        continue
    fi

    # Check if it has a criterion_group! or criterion_main! (valid benchmark)
    if ! grep -q "criterion_group\|criterion_main" "$FILE"; then
        echo "  ⏭️  Not a Criterion benchmark, skipping"
        continue
    fi

    # Backup original file
    cp "$FILE" "$FILE.bak"

    # Add module import at top (after use statements)
    # Find the last line with "use " and add after it
    LAST_USE_LINE=$(grep -n "^use " "$FILE" | tail -1 | cut -d: -f1)

    if [ -n "$LAST_USE_LINE" ]; then
        # Insert protection module import
        sed -i "${LAST_USE_LINE}a\\
\\
// Benchmark protection (centralized module)\\
#[path = \"$REL_PATH\"]\\
mod benchmark_protection;\\
use benchmark_protection::require_valid_license;" "$FILE"

        echo "  ✓ Added module import at line $LAST_USE_LINE"
    else
        echo "  ⚠️  Could not find use statements, manual intervention needed"
        mv "$FILE.bak" "$FILE"
        continue
    fi

    # Find first benchmark function and add protection call
    # Look for patterns like: fn benchmark_name(c: &mut Criterion) {
    FIRST_BENCH=$(grep -n "^fn.*\(c:.*Criterion\)" "$FILE" | head -1 | cut -d: -f1)

    if [ -n "$FIRST_BENCH" ]; then
        # Add protection call as first line of function
        FIRST_BENCH=$((FIRST_BENCH + 1))  # Line after fn declaration (opening brace)
        sed -i "${FIRST_BENCH}a\\
    // CRITICAL: Require valid license to run benchmarks (prevents competitor access)\\
    require_valid_license(\"$BENCHMARK_NAME\");" "$FILE"

        echo "  ✓ Added protection call to first benchmark function"
    else
        echo "  ⚠️  Could not find benchmark function, manual intervention needed"
        mv "$FILE.bak" "$FILE"
        continue
    fi

    # Remove backup if successful
    rm "$FILE.bak"

    echo "  ✅ Protected: $BASENAME"
    echo ""
done

echo "=== Protection Summary ==="
echo "Total files processed: $COUNT"
echo "Protection module: $BENCHES_DIR/$PROTECTION_MODULE"
echo ""
echo "To test:"
echo "  cargo bench --features 'benchmarking,meta-capsule' -- --test  # With license"
echo "  cargo bench --features 'benchmarking' -- --test               # Without license (dev mode)"
