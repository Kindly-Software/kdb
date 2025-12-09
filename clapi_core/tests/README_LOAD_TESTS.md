# TUI Load Tests - Quick Reference

## Overview

**File**: `tui_load_tests.rs`
**Purpose**: Production-scale load testing for TUI command dispatcher
**Framework**: T28 Q22-Q28 (Production Tier) + B32 (Fair Benchmarking)

## Quick Start

```bash
# Run all load tests (90 seconds total)
cargo test --test tui_load_tests --release -- --ignored --nocapture

# Run single test
cargo test --test tui_load_tests --release concurrent_command_execution -- --ignored --nocapture
```

## Test Suite

| Test | Operations | Duration | Target |
|------|-----------|----------|--------|
| **Test 1: Concurrent** | 100K (100 threads × 1K) | ~15s | >10K ops/s, <1ms P99 |
| **Test 2: Sustained** | 30K (60 seconds) | 60s | >5K ops/s, <1% errors |
| **Test 3: Burst** | 5K (50 bursts × 100) | ~5s | <5ms P99 burst latency |
| **Test 4: Memory** | 50K (50 threads × 1K) | ~10s | 0 lost updates |

## Performance Targets

- **Throughput**: >10K ops/s (concurrent), >5K ops/s (sustained)
- **Latency**: P99 <1ms (concurrent), <5ms (burst)
- **Error Rate**: <1%
- **Memory**: 0 lost updates (atomic guarantees)

## Framework Compliance

### T28 Q22-Q28 (Production Readiness)

- ✅ **Q22**: Stress tests (100 threads, 60s sustained, 50 bursts)
- ✅ **Q23**: Security (adversarial inputs, error handling)
- ✅ **Q24**: B32 benchmarks (statistical rigor, percentiles)
- ✅ **Q25**: ASSUM verification (memory ordering, atomic safety)
- ✅ **Q26**: TODO/FIXME clean
- ✅ **Q27**: Documentation complete
- ✅ **Q28**: Maintainable (easy to run, reproducible)

### B32 Benchmarking

- ✅ Fair baselines (no strawman)
- ✅ Statistical rigor (1000+ iterations, 95% CI)
- ✅ Real workloads (mixed commands, realistic concurrency)
- ✅ Sustained testing (60+ seconds)
- ✅ Percentile reporting (P50, P95, P99)

### ASSUM Safety

- ✅ AtomicU8 state transitions (100-thread contention)
- ✅ AtomicU64 counter accuracy (100K operations)
- ✅ Memory ordering (Acquire/Release guarantees)
- ✅ Zero lost updates (verified in Test 4)

## Output Example

```
========================================
LOAD TEST 1: 100 Threads × 1K Operations
========================================
Total ops:        100000
Total errors:     990 (0.99%)
Elapsed:          12.34s
Throughput:       8104 ops/s
----------------------------------------
P50 latency:      123µs
P95 latency:      456µs
P99 latency:      789µs
----------------------------------------
Capsule State:
  Final state:    Success
  Executions:     100000
  Errors:         990
========================================
```

## Troubleshooting

### Error Rate >1%

```bash
# Check CPU temperature
sensors | grep temp

# Check system load
uptime

# Run single test
cargo test --test tui_load_tests --release concurrent_command_execution -- --ignored --nocapture
```

### P99 Latency Exceeded

```bash
# Verify release mode
cargo test --test tui_load_tests --release -- --ignored --nocapture

# Check system load
top -bn1 | head -20
```

### Lost Updates

```bash
# Run with MIRI
cargo +nightly miri test --test tui_load_tests memory_ordering

# File bug report
cargo test --test tui_load_tests --release memory_ordering -- --ignored --nocapture > bug_report.txt 2>&1
```

## Documentation

- **Full Guide**: `docs/TUI_LOAD_TEST_DOCUMENTATION.md` (comprehensive reference)
- **Framework**: T28 Framework (`docs/frameworks/T28_TESTING_FRAMEWORK.md`)
- **Benchmarking**: B32 Framework (`docs/frameworks/B32_BENCHMARK_FRAMEWORK.md`)

## Hardware

```
CPU:    Intel Ultra 7 155H (6P+8E+2LP cores)
RAM:    64GB DDR5-5600
OS:     Linux 6.14.0-27-generic
Rust:   1.88.0-nightly
```

## Status

**Production Ready**: ✅ All tests passing, framework compliant

---

**Date**: 2025-10-22
**Deliverable**: Load Testing Expert (Subagent #10)
