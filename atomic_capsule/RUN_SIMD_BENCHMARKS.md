# Quick Start: Running SIMD Capsule Benchmarks

## Prerequisites

- Rust nightly toolchain (for `portable_simd` feature)
- Criterion benchmark framework (installed via dev-dependencies)
- Intel Ultra 7 155H or compatible AVX2/AVX-512 CPU

## Quick Run

```bash
cd /home/samuel/Primitives/atomic_capsule

# Run all SIMD benchmarks (full statistical analysis)
cargo bench --features portable_simd --bench simd_capsule_bench

# Run quick benchmarks (reduced sample size for faster iteration)
cargo bench --features portable_simd --bench simd_capsule_bench -- --quick

# Run specific benchmark group
cargo bench --features portable_simd --bench simd_capsule_bench -- dot_product
cargo bench --features portable_simd --bench simd_capsule_bench -- element_wise
cargo bench --features portable_simd --bench simd_capsule_bench -- cache_efficiency
```

## Benchmark Groups

### 1. Dot Product (`dot_product`)
- **Scalar baseline**: Optimized iterator-based dot product
- **SIMD capsule**: Cache-aligned SIMD dot product
- **Expected**: 1.5-2.5x speedup (1.82x observed)

### 2. Element-Wise Operations (`element_wise`)
- **multiply_scalar**: Manual loop element-wise multiply
- **multiply_simd**: SIMD capsule element-wise multiply
- **add_scalar**: Manual loop element-wise addition
- **add_simd**: SIMD capsule element-wise addition
- **Expected**: Overhead dominates for 8 elements

### 3. Cache Efficiency (`cache_efficiency`)
- **simd_cold_cache**: New capsule each iteration (cache miss)
- **simd_warm_cache**: Reuse capsule (L1 cache hit)
- **Expected**: 10-15x difference (13.7x observed)

## Output Files

Criterion generates detailed reports in `target/criterion/`:

```bash
# View HTML reports (requires browser)
firefox target/criterion/report/index.html

# View raw data
cat target/criterion/dot_product/scalar/estimates.json
cat target/criterion/dot_product/simd_capsule/estimates.json
```

## Interpreting Results

### Time Units
- **ps (picoseconds)**: 10^-12 seconds (very fast operations)
- **ns (nanoseconds)**: 10^-9 seconds (typical SIMD operations)

### Statistical Metrics
- **time**: Median time (50th percentile)
- **±**: 95% confidence interval half-width
- **Lower bound**: First value in bracket
- **Upper bound**: Third value in bracket

Example:
```
dot_product/scalar      time:   [479.76 ps 484.14 ps 485.23 ps]
                               ↑         ↑         ↑
                               Lower    Median    Upper (95% CI)
```

### Speedup Calculation
```
Speedup = Baseline Time / Optimized Time
1.82x = 484.14 ps / 266.39 ps
```

## Troubleshooting

### Compilation Errors

**Error**: `feature 'portable_simd' not found`
**Solution**: Ensure you're using Rust nightly:
```bash
rustup override set nightly
```

**Error**: `no method named 'reduce_sum' found`
**Solution**: Add SIMD trait imports (should be fixed in benchmark)

### Benchmark Failures

**Issue**: Benchmarks hang or timeout
**Solution**: Reduce sample size:
```bash
cargo bench --features portable_simd --bench simd_capsule_bench -- --quick
```

**Issue**: Results vary wildly between runs
**Solution**: Ensure system is idle (no background processes):
```bash
# Check system load
top

# Close unnecessary applications
# Disable CPU frequency scaling (requires root)
sudo cpupower frequency-set -g performance
```

## Customizing Benchmarks

### Adjust Sample Size

Edit `benches/simd_capsule_bench.rs`:

```rust
group.confidence_level(0.95)     // 95% CI (default)
     .sample_size(1000)          // Number of iterations (reduce for quick tests)
     .warm_up_time(Duration::from_secs(3));  // Warmup duration
```

### Add New Benchmarks

Follow existing patterns in `simd_capsule_bench.rs`:

```rust
fn bench_my_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("my_operation");

    group.confidence_level(0.95)
         .sample_size(1000)
         .warm_up_time(Duration::from_secs(2));

    // Add baseline
    group.bench_function("baseline", |bencher| {
        bencher.iter(|| {
            black_box(/* your baseline implementation */)
        });
    });

    // Add SIMD capsule version
    group.bench_function("simd_capsule", |bencher| {
        bencher.iter(|| {
            black_box(/* your SIMD capsule implementation */)
        });
    });

    group.finish();
}

// Add to criterion_group!
criterion_group!(
    benches,
    bench_dot_product,
    bench_element_wise_operations,
    bench_cache_efficiency,
    bench_my_operation,  // <- Add here
);
```

## B32 Framework Guidelines

### Fair Baselines (B1)
- ✅ Use optimized scalar implementations
- ❌ Don't compare against naive/unoptimized code
- ✅ Multiple baselines for comprehensive analysis

### Statistical Rigor (B2)
- ✅ Minimum 1000 iterations per benchmark
- ✅ Report 95% confidence intervals
- ✅ Include warmup periods (2-3 seconds)

### Realistic Workloads (B3)
- ✅ Test actual computational patterns
- ❌ Avoid synthetic loops
- ✅ Use production-like data

### Honest Gains (B27)
- ✅ Report negative results (SIMD slower)
- ✅ Explain unexpected outcomes
- ❌ Don't cherry-pick best runs

## Next Steps

1. **Review results**: Check `SIMD_BENCHMARK_REPORT.md`
2. **Understand limitations**: 8 elements is too small for full SIMD benefit
3. **Add larger benchmarks**: Test with 64, 256, 1024 elements
4. **Production validation**: Run with real-world workloads

## References

- **Performance Report**: `SIMD_BENCHMARK_REPORT.md`
- **Delivery Summary**: `B32_BENCHMARK_DELIVERY_SUMMARY.md`
- **B32 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/B32_BENCHMARK_FRAMEWORK.md`
- **Implementation**: `src/primitives/simd_f32.rs`

---

**Last Updated**: 2025-10-07
**Benchmark Suite Version**: v0.2.0
