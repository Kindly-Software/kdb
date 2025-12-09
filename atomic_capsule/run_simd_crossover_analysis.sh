#!/bin/bash
# Run SIMD Crossover Analysis - Find Break-Even Point
#
# This script:
# 1. Compiles and runs realistic SIMD validation benchmarks
# 2. Saves results to file
# 3. Opens HTML report in browser
# 4. Provides instructions for analyzing results

set -e  # Exit on error

echo "========================================="
echo "SIMD Crossover Analysis - Benchmark Run"
echo "========================================="
echo ""
echo "Framework Compliance:"
echo "  - UCE33 Q10: Tier 2 SIMD Capsule"
echo "  - UCE33 Q30: Empirical Validation"
echo "  - B32 B1: Fair Baselines"
echo "  - B32 B2: Statistical Rigor (95% CI, 1000+ samples)"
echo "  - B32 B27: Honest Reporting"
echo ""
echo "Phase 5 Finding: SIMD SLOWER for 8 elements"
echo "Question: At what array size does SIMD become faster?"
echo ""

# Check if nightly Rust is available
if ! rustc --version | grep -q nightly; then
    echo "ERROR: Nightly Rust required for portable_simd feature"
    echo ""
    echo "Install nightly:"
    echo "  rustup install nightly"
    echo ""
    echo "Or use nightly for this directory:"
    echo "  rustup override set nightly"
    exit 1
fi

echo "Rust version:"
rustc --version
echo ""

# Run benchmark with output to file
OUTPUT_FILE="simd_crossover_results_$(date +%Y%m%d_%H%M%S).txt"

echo "Running benchmarks (this may take 10-15 minutes)..."
echo "Output will be saved to: $OUTPUT_FILE"
echo ""

cargo bench \
    --bench realistic_simd_validation_bench \
    --features nightly \
    2>&1 | tee "$OUTPUT_FILE"

echo ""
echo "========================================="
echo "Benchmark Complete!"
echo "========================================="
echo ""
echo "Results saved to: $OUTPUT_FILE"
echo ""
echo "HTML Report: target/criterion/report/index.html"
echo ""

# Try to open HTML report
if command -v xdg-open &> /dev/null; then
    echo "Opening HTML report in browser..."
    xdg-open target/criterion/report/index.html &
elif command -v open &> /dev/null; then
    echo "Opening HTML report in browser..."
    open target/criterion/report/index.html &
else
    echo "Open this file in your browser:"
    echo "  $(pwd)/target/criterion/report/index.html"
fi

echo ""
echo "========================================="
echo "Next Steps:"
echo "========================================="
echo ""
echo "1. Review results in HTML report"
echo ""
echo "2. Extract crossover points:"
echo "   - Find array size where SIMD >= 1.0× scalar (break-even)"
echo "   - Find array size where SIMD >= 2.0× scalar (2× speedup)"
echo "   - Find maximum speedup achieved"
echo ""
echo "3. Update documentation:"
echo "   - SIMD_CROSSOVER_ANALYSIS.md (fill in TBD values)"
echo "   - REALISTIC_SIMD_PERFORMANCE_GUIDE.md (update expectations)"
echo "   - PHASE5_API_REFERENCE.md (update performance claims)"
echo ""
echo "4. Look for patterns:"
echo "   - Which operations benefit most from SIMD?"
echo "   - Are realistic workloads (trading, brain) faster?"
echo "   - Does speedup scale linearly or saturate?"
echo ""
echo "5. Generate CSV for plotting:"
echo "   grep -E '(scalar|simd)' $OUTPUT_FILE > speedup_data.csv"
echo ""
echo "6. Commit results:"
echo "   git add $OUTPUT_FILE SIMD_CROSSOVER_ANALYSIS.md"
echo "   git commit -m 'feat: SIMD crossover analysis - empirical break-even points'"
echo ""
echo "========================================="
echo "Analysis Tips:"
echo "========================================="
echo ""
echo "Expected Findings:"
echo "  - 8 elements: 0.3-0.7× (SLOWER, matches Phase 5)"
echo "  - 64-128 elements: ~1.0× (break-even)"
echo "  - 256+ elements: 1.5-3× (SIMD wins)"
echo "  - 1024+ elements: 2-4× (realistic maximum)"
echo ""
echo "Red Flags:"
echo "  - Speedup > 6× (suspicious, check methodology)"
echo "  - High variance (>15%, may indicate instability)"
echo "  - Break-even < 32 elements (contradicts Phase 5)"
echo ""
echo "Honest Reporting (B32 B27):"
echo "  - Report ALL results (including failures)"
echo "  - Document unexpected findings"
echo "  - Update performance claims based on data"
echo ""
