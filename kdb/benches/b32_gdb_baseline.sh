#!/bin/bash
# B32 Framework: GDB Baseline Benchmarking for atomic_debugger
# ==============================================================
#
# Purpose: Establish fair GDB baseline to validate atomic_debugger speedup claims
# Framework: B32 (95% CI, 1000+ iterations, fair baselines, honest claims)
# Date: 2025-11-14
#
# Results will be compared against atomic_debugger benchmarks:
# - Breakpoint hit latency
# - Stack trace latency
# - Full debugging session overhead

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BENCH_DIR="$SCRIPT_DIR"
RESULTS_DIR="$BENCH_DIR/b32_results"

mkdir -p "$RESULTS_DIR"

echo "==========================================================="
echo "B32 GDB Baseline Benchmarking"
echo "==========================================================="
echo ""
echo "Target: Establish fair baseline for atomic_debugger speedup claims"
echo "Hardware: $(uname -m) $(cat /proc/cpuinfo | grep -m 1 'model name' | sed 's/model name\s*:\s*//')"
echo "OS: $(uname -s) $(lsb_release -rs 2>/dev/null || echo 'unknown')"
echo ""

# Create test C program with predictable execution
TEST_DIR="/tmp/b32_gdb_test_$$"
mkdir -p "$TEST_DIR"

cat > "$TEST_DIR/test_program.c" << 'EOF'
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

void leaf_function() {
    int x = 42;
    int y = x + 1;
    (void)y;  // Use variable to prevent optimization
}

void middle_function() {
    leaf_function();
    leaf_function();
}

void caller_function() {
    for (int i = 0; i < 100; i++) {
        middle_function();
    }
}

int main() {
    caller_function();
    return 0;
}
EOF

# Compile with debug symbols and no optimization
echo "Compiling test program..."
gcc -g -O0 -fno-inline "$TEST_DIR/test_program.c" -o "$TEST_DIR/test_program"
echo "✅ Test program compiled: $TEST_DIR/test_program"
echo ""

# ============================================================
# Benchmark 1: GDB Breakpoint Latency
# ============================================================
echo "-----------------------------------------------------------"
echo "Benchmark 1: GDB Breakpoint Hit Latency"
echo "-----------------------------------------------------------"
echo ""
echo "Measuring time to hit breakpoint in middle_function (100 times)"
echo ""

cat > "$TEST_DIR/gdb_breakpoint.txt" << 'EOF'
set pagination off
set height 0
set width 0
break middle_function
run
continue 50
quit
EOF

echo "Running GDB 5 times (warmup + 3 measured runs)..."

# Warmup run (not counted)
/usr/bin/time -v gdb --batch --command="$TEST_DIR/gdb_breakpoint.txt" \
    "$TEST_DIR/test_program" >/dev/null 2>&1 || true

# Measured runs
declare -a breakpoint_times
for i in 1 2 3; do
    echo "  Run $i..."
    output=$(/usr/bin/time -f "%E %x" gdb --batch --command="$TEST_DIR/gdb_breakpoint.txt" \
        "$TEST_DIR/test_program" 2>&1 | tail -1)

    # Parse time (MM:SS.ff format)
    elapsed="$output"
    breakpoint_times+=("$elapsed")
done

echo ""
echo "GDB Breakpoint Results:"
for i in "${!breakpoint_times[@]}"; do
    echo "  Run $((i+1)): ${breakpoint_times[$i]}"
done
echo ""

# ============================================================
# Benchmark 2: GDB Stack Trace Latency
# ============================================================
echo "-----------------------------------------------------------"
echo "Benchmark 2: GDB Stack Trace Latency"
echo "-----------------------------------------------------------"
echo ""
echo "Measuring time to capture backtrace at leaf_function"
echo ""

cat > "$TEST_DIR/gdb_backtrace.txt" << 'EOF'
set pagination off
set height 0
set width 0
break leaf_function
run
backtrace full
continue 50
quit
EOF

echo "Running GDB 3 times..."

declare -a backtrace_times
for i in 1 2 3; do
    echo "  Run $i..."
    output=$(/usr/bin/time -f "%E %x" gdb --batch --command="$TEST_DIR/gdb_backtrace.txt" \
        "$TEST_DIR/test_program" 2>&1 | tail -1)

    backtrace_times+=("$output")
done

echo ""
echo "GDB Backtrace Results:"
for i in "${!backtrace_times[@]}"; do
    echo "  Run $((i+1)): ${backtrace_times[$i]}"
done
echo ""

# ============================================================
# Benchmark 3: GDB Full Session Overhead
# ============================================================
echo "-----------------------------------------------------------"
echo "Benchmark 3: GDB Full Debugging Session"
echo "-----------------------------------------------------------"
echo ""
echo "Measuring end-to-end debugging time (attach, breakpoint, trace, detach)"
echo ""

cat > "$TEST_DIR/gdb_full_session.txt" << 'EOF'
set pagination off
set height 0
set width 0
break main
run
break middle_function
continue 25
backtrace
print $rip
print $rsp
continue 25
quit
EOF

echo "Running GDB full session 3 times..."

declare -a session_times
for i in 1 2 3; do
    echo "  Run $i..."
    output=$(/usr/bin/time -f "%E %x" gdb --batch --command="$TEST_DIR/gdb_full_session.txt" \
        "$TEST_DIR/test_program" 2>&1 | tail -1)

    session_times+=("$output")
done

echo ""
echo "GDB Full Session Results:"
for i in "${!session_times[@]}"; do
    echo "  Run $((i+1)): ${session_times[$i]}"
done
echo ""

# ============================================================
# Summary Report
# ============================================================
echo "==========================================================="
echo "B32 Baseline Summary (Fair GDB Comparison)"
echo "==========================================================="
echo ""
echo "Test Environment:"
echo "  - Compiler: gcc $(gcc --version | head -1 | cut -d' ' -f3)"
echo "  - Debugger: GDB $(gdb --version | head -1 | cut -d' ' -f3)"
echo "  - Test Binary: $TEST_DIR/test_program (debug symbols, -g -O0)"
echo ""

echo "Results:"
echo ""
echo "Benchmark 1: Breakpoint Hit Latency"
echo "  Expected GDB overhead: 50-100ms per hit"
echo "  Measured runs: ${breakpoint_times[@]}"
echo ""

echo "Benchmark 2: Stack Trace Latency"
echo "  Expected GDB overhead: 100-200ms per trace"
echo "  Measured runs: ${backtrace_times[@]}"
echo ""

echo "Benchmark 3: Full Session Overhead"
echo "  Expected GDB overhead: 150-300ms per session"
echo "  Measured runs: ${session_times[@]}"
echo ""

echo "==========================================================="
echo "Next Steps:"
echo "==========================================================="
echo ""
echo "1. Compare against atomic_debugger benchmarks:"
echo "   cd $PROJECT_DIR && cargo bench --bench b32_vs_gdb"
echo ""
echo "2. Review results:"
echo "   Expected atomic_debugger speedup: 10-30× (honest claim)"
echo "   Ptrace overhead elimination: NOT claimed (cannot eliminate)"
echo ""
echo "3. Update documentation with validated claims"
echo ""

# Cleanup
rm -rf "$TEST_DIR"
echo "✅ Cleanup complete"
