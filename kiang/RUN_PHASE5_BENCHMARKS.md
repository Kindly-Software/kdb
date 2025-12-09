# Run Phase 5 Benchmarks - Quick Start Guide

## Prerequisites

```bash
# Ensure you're in the kiang directory
cd /home/samuel/Primitives/kiang

# Verify benchmarks compile
cargo check --benches
```

## Run Individual Benchmarks

### 1. Real Driver Overhead Benchmarks

Measures actual DRM ioctl performance vs simulated baseline:

```bash
cargo bench --bench real_driver_bench
```

**What it measures**:
- Device open latency
- GEM object creation
- VM_BIND operations
- Fence polling
- Complete submission workflow
- Batch scaling

**Expected output**:
```
device_open/simulated    time:   [~1μs]
device_open/real_drm     time:   [~100μs]
gem_create/4096/simulated    time:   [~5μs]
gem_create/4096/real_drm     time:   [~10μs]
```

### 2. Error Recovery Benchmarks

Measures GPU error detection and recovery latency:

```bash
cargo bench --bench error_recovery_bench
```

**What it measures**:
- Hang detection speed
- Context reset timing
- Recovery workflow latency
- Thermal throttling response
- Quality level transitions

**Expected output**:
```
hang_detection/single_check      time:   [<100ns]
context_reset/software_reset     time:   [~1ms]
recovery_workflow/complete       time:   [<5ms]
```

### 3. SIMD Batch Benchmarks

Compares scalar vs SIMD batch processing:

```bash
cargo bench --bench simd_batch_bench
```

**With nightly SIMD support**:
```bash
cargo +nightly bench --bench simd_batch_bench --features simd
```

**What it measures**:
- Scalar vs SIMD speedup
- Batch size crossover point
- Memory bandwidth utilization
- Alignment impact

**Expected output**:
```
scalar_vs_simd/64/scalar_naive      time:   [baseline]
scalar_vs_simd/64/scalar_optimized  time:   [~1.5x faster]
scalar_vs_simd/64/simd              time:   [~2-3x faster]
```

### 4. Metrics Collection Benchmarks

Measures monitoring overhead:

```bash
cargo bench --bench metrics_collection_bench
```

**What it measures**:
- Counter increment latency
- Snapshot overhead
- Concurrent update contention
- Export format performance

**Expected output**:
```
counter_increment/single_inc    time:   [10-20ns]
metrics_snapshot                time:   [<100ns]
hot_path_overhead/with_metrics  time:   [<5% overhead]
```

## Run All Phase 5 Benchmarks

```bash
# Run all 4 benchmarks sequentially
cargo bench --bench real_driver_bench \
            --bench error_recovery_bench \
            --bench simd_batch_bench \
            --bench metrics_collection_bench
```

Or simply:
```bash
# Run all benchmarks (includes Phase 1-4 benchmarks too)
cargo bench
```

## Save Results

### Baseline Results

```bash
# Create baseline for comparison
cargo bench --benches > baseline_results.txt
```

### Compare Against Baseline

```bash
# Make changes, then compare
cargo bench --benches > new_results.txt
diff baseline_results.txt new_results.txt
```

### Criterion HTML Reports

Criterion generates HTML reports automatically:
```bash
# Run benchmarks
cargo bench

# Open reports in browser
open target/criterion/report/index.html
```

## Advanced Options

### Specific Benchmark Function

```bash
# Run only GEM creation benchmarks
cargo bench --bench real_driver_bench -- gem_create

# Run only SIMD vs scalar comparison
cargo bench --bench simd_batch_bench -- scalar_vs_simd
```

### Custom Sample Size

```bash
# More samples for higher precision (slower)
cargo bench -- --sample-size 5000

# Fewer samples for quick validation
cargo bench -- --sample-size 100
```

### Warm Cache Testing

```bash
# Run with warm cache
cargo bench -- --warm-up-time 10
```

## Performance Targets

Reference targets from B32 framework:

### Real Driver Benchmarks
- Device open: <100μs
- GEM create: <10μs
- VM_BIND: <50μs
- Fence poll: <1μs

### Error Recovery Benchmarks
- Hang detection: <100μs
- Context reset: <1ms
- Full recovery: <5ms

### SIMD Benchmarks
- Small batches (<64): Scalar wins
- Medium batches (64-256): SIMD 2-3x
- Large batches (256+): SIMD 3-4x

### Metrics Benchmarks
- Counter increment: <20ns
- Snapshot: <100ns
- Hot path overhead: <5%

## Interpreting Results

### Criterion Output Format

```
test_name           time:   [lower_bound estimate upper_bound]
                    change: [-5.1234% -0.1234% +2.3456%] (p = 0.05)
                    Performance has improved.
```

- **estimate**: Best estimate of actual time
- **bounds**: 95% confidence interval
- **change**: Compared to previous run
- **p-value**: Statistical significance

### What to Look For

✅ **Good Results**:
- Estimates within target ranges
- Tight confidence intervals (<10% variation)
- Consistent across runs
- Scaling matches expectations

❌ **Investigate**:
- Estimates far above targets
- Wide confidence intervals (>20% variation)
- Large performance regressions
- Unexpected scaling behavior

## Troubleshooting

### Permission Denied

If real driver benchmarks fail:
```bash
# Check GPU device permissions
ls -la /dev/dri/card0

# May need to add user to video group
sudo usermod -a -G video $USER
```

### Benchmark Hangs

If benchmarks hang or timeout:
```bash
# Reduce sample size
cargo bench -- --sample-size 10

# Skip problematic benchmarks
cargo bench --bench metrics_collection_bench  # Skip driver tests
```

### Out of Memory

If system runs out of memory:
```bash
# Reduce batch sizes in code
# Or run benchmarks individually instead of all at once
cargo bench --bench real_driver_bench
cargo bench --bench error_recovery_bench
# ... etc
```

## Hardware Requirements

### Minimum

- CPU: x86_64 with SSE2
- RAM: 4GB
- OS: Linux with DRM support

### Recommended

- CPU: Intel Ultra 7 155H or similar (for realistic results)
- RAM: 16GB
- GPU: Intel Arc A-series (for real driver tests)
- Cooling: Active cooling (avoid thermal throttling)

### For Real Driver Tests

- Intel Arc GPU installed
- Intel Xe kernel driver loaded
- Permissions to access /dev/dri/card0

## Continuous Integration

### GitHub Actions Example

```yaml
name: Benchmarks

on: [push, pull_request]

jobs:
  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo bench --benches -- --sample-size 100
```

### Benchmark History Tracking

```bash
# Create benchmark history directory
mkdir -p benchmark_history

# Run and save results with timestamp
timestamp=$(date +%Y%m%d_%H%M%S)
cargo bench --benches > "benchmark_history/${timestamp}_results.txt"

# Compare latest with previous
latest=$(ls benchmark_history/*.txt | tail -1)
previous=$(ls benchmark_history/*.txt | tail -2 | head -1)
diff "$previous" "$latest"
```

## Next Steps

1. **Run baseline benchmarks** to establish performance reference
2. **Validate against B32 targets** from PHASE5_BENCHMARK_REPORT.md
3. **Identify optimization opportunities** where performance is below target
4. **Re-run after optimizations** to measure improvement

---

**For detailed benchmark descriptions and B32 compliance, see**: `PHASE5_BENCHMARK_REPORT.md`
