# LOS B32 Performance Benchmarks - Execution Guide

**Location**: `/home/samuel/Primitives/atomic_capsule/benches/los_b32_bench.rs`

**Framework**: Criterion (B32 compliant: 1000+ iterations, 95% CI, fair baselines)

---

## Quick Start

### Run All Benchmarks (Locally - Quick Validation)
```bash
cargo bench --bench los_b32_bench --features "los,los-avx2"
```

### Run on Remote (MANDATORY for B32 Validation)
```bash
# Per remote-execution-mandate from CLAUDE.md
ssh samuel@kindly-hub "cd ~/Primitives/atomic_capsule && cargo bench --bench los_b32_bench --features 'los,los-avx2'"
```

**Why Remote?**: Consistent hardware (AMD Ryzen 9 6900HX, 64GB DDR5-4800) for reproducible B32 results. Prevents local system overload.

---

## Benchmark Categories

### Category 1: Single Ray Latency (ns/ray)
| Benchmark | Capsule | Samples | Expected Performance |
|-----------|---------|---------|----------------------|
| `sparse_50_samples` | SparseLosScalarCapsule | 50 | <50ns (baseline scalar) |
| `tactical_200_samples` | TacticalLosSimdCapsule | 200 | <100ns (40× vs scalar) |
| `dense_avx2_500_samples` | DenseLosAvx2Capsule | 500 | <5-8ns/sample (AVX2 8× speedup) |
| `metacapsule_auto_dispatch_200` | LosMetacapsule | 200 | <100ns (auto-dispatch overhead <10%) |

### Category 2: Batch Throughput (rays/sec)
| Benchmark | Batch Size | Expected Performance |
|-----------|------------|----------------------|
| `batched_4_rays_100_samples` | 4 rays | 2-4× vs sequential |
| `batched_8_rays_100_samples` | 8 rays | 2-4× vs sequential |
| `metacapsule_batch_4_auto` | 4 rays | Near-optimal dispatch |
| `metacapsule_batch_8_auto` | 8 rays | Near-optimal dispatch |

### Category 3: Scaling (varying ray length)
| Benchmark | Ray Length | Purpose |
|-----------|------------|---------|
| `metacapsule_auto/50` | 50 samples | Short rays |
| `metacapsule_auto/200` | 200 samples | Medium rays |
| `metacapsule_auto/500` | 500 samples | Long rays |
| `metacapsule_auto/1000` | 1000 samples | Very long rays |

### Category 4: Real-world Scenarios
| Benchmark | Description | Volume |
|-----------|-------------|--------|
| `grid_los_100x100_10k_rays` | Grid visibility query | 10,000 rays |
| `radial_sweep_360_rays` | 360-degree sweep from center | 360 rays |
| `random_rays_1000_mixed_lengths` | Random origins/targets | 1,000 rays |

### Category 5: Comparison Groups (Direct Speedup Validation)
| Benchmark | Comparison | Expected Speedup |
|-----------|------------|------------------|
| `sparse_vs_tactical` | Scalar vs SIMD | 2-4× (portable_simd) |
| `tactical_vs_dense_avx2` | portable_simd vs AVX2 | 2-8× (AVX2 8-wide) |
| `single_vs_batched` | Sequential vs Horizontal SIMD | 2-4× (SoA batch) |

---

## Feature Flags

### Minimal (no SIMD)
```bash
cargo bench --bench los_b32_bench --features "los"
```
- Runs sparse + tactical (scalar fallback)
- Skips AVX2 benchmarks

### Full (AVX2 + SIMD)
```bash
cargo bench --bench los_b32_bench --features "los,los-avx2"
```
- Includes all benchmarks
- AVX2 8-wide unrolled kernel
- portable_simd tactical path

---

## B32 Compliance Checklist

### ✓ Fair Baselines
- **Sparse**: Optimized scalar (not strawman)
- **Tactical**: portable_simd (cross-platform SIMD)
- **Dense**: AVX2 8× unrolled (production kernel)
- **Batched**: Horizontal SIMD (SoA layout)

### ✓ Hardware Info
- Embedded in benchmark names (e.g., `dense_avx2_500_samples`)
- Remote execution on known hardware (kindly-hub)

### ✓ Statistical Rigor
- Criterion default: 1000+ iterations
- 95% confidence intervals
- Warm-up iterations before measurement

### ✓ Reproducibility
- `black_box()` prevents compiler optimization
- Fixed map sizes and cover densities
- Consistent ray geometries

### ✓ Reality Check
- **Typical**: 10-50% speedup claims (validated)
- **Exceptional**: 2-10× speedup claims (extensive validation)
- **Breakthrough**: 100×+ speedup claims (requires exceptional validation)

---

## Expected Results (Target Performance)

### Single Ray Latency
- **Sparse (50 samples)**: ~40-60ns (scalar baseline)
- **Tactical (200 samples)**: ~100ns (40× vs scalar = 2.5ns/sample)
- **Dense AVX2 (500 samples)**: ~2.5-4ms total (5-8ns/sample)
- **Metacapsule Auto**: <10% dispatch overhead

### Batch Throughput
- **Batched 4 rays**: 2-4× vs sequential
- **Batched 8 rays**: 2-4× vs sequential (full SIMD lane utilization)

### Scaling
- Linear scaling with ray length (O(n) samples)
- Auto-classification accuracy >95%

### Real-world Scenarios
- **Grid 10K rays**: <10ms total (1μs/ray average)
- **Radial sweep 360**: <100μs total (280ns/ray)
- **Random 1000**: <500μs total (500ns/ray)

### Comparison Groups
- **Sparse → Tactical**: 2-4× (SIMD advantage)
- **Tactical → Dense AVX2**: 2-8× (AVX2 8-wide)
- **Single → Batched**: 2-4× (horizontal SIMD)

---

## Troubleshooting

### Compilation Errors
```bash
# Missing LOS feature
cargo bench --bench los_b32_bench --features "los"

# Missing AVX2 feature
cargo bench --bench los_b32_bench --features "los,los-avx2"
```

### Remote Execution Issues
```bash
# Check sync status
journalctl --user -u lsyncd -n 20

# Restart sync if needed
systemctl --user restart lsyncd

# Manual sync
rsync -av ~/Primitives/atomic_capsule/ samuel@kindly-hub:~/Primitives/atomic_capsule/
```

### Performance Anomalies
1. **Lower than expected**: Check for background processes on kindly-hub
2. **Higher variance**: Increase iterations via `--bench --warm-up-time 10`
3. **Crashes**: Check map buffer allocation (requires 32B alignment)

---

## Results Analysis

### Criterion Output Format
```
single_ray_latency/sparse_50_samples
                        time:   [45.231 ns 45.789 ns 46.412 ns]
                        change: [-2.1% +0.5% +3.2%] (p = 0.44 > 0.05)
                        No change in performance detected.
```

### Key Metrics
- **time**: Median ± 95% CI
- **change**: vs previous baseline
- **p-value**: Statistical significance (p < 0.05 = significant)

### Speedup Calculation
```
Speedup = baseline_time / optimized_time

Example:
Sparse:   45.789 ns
Tactical: 11.234 ns
Speedup = 45.789 / 11.234 = 4.08× ✓ (within 2-4× expected)
```

---

## B32 Report Template

```markdown
# LOS B32 Benchmark Results - [Date]

**Hardware**: AMD Ryzen 9 6900HX, 64GB DDR5-4800 (kindly-hub)
**Features**: los, los-avx2
**Iterations**: 1000+ (Criterion default)
**Confidence**: 95% CI

## Single Ray Latency
| Capsule | Samples | Median (ns) | 95% CI | Speedup vs Sparse |
|---------|---------|-------------|--------|-------------------|
| Sparse | 50 | X.XX ns | [X.XX, X.XX] | 1.00× (baseline) |
| Tactical | 200 | X.XX ns | [X.XX, X.XX] | X.XX× |
| Dense AVX2 | 500 | X.XX μs | [X.XX, X.XX] | X.XX× |
| Metacapsule | 200 | X.XX ns | [X.XX, X.XX] | X.XX× |

## Batch Throughput
| Configuration | Median (μs) | Throughput (rays/sec) | Speedup vs Sequential |
|---------------|-------------|----------------------|----------------------|
| Single×8 | X.XX μs | X.XX M rays/sec | 1.00× (baseline) |
| Batched 8 | X.XX μs | X.XX M rays/sec | X.XX× |

## Comparison Groups
| Comparison | Baseline (ns) | Optimized (ns) | Speedup | Expected | Status |
|------------|---------------|----------------|---------|----------|--------|
| Sparse → Tactical | X.XX | X.XX | X.XX× | 2-4× | ✓/✗ |
| Tactical → Dense AVX2 | X.XX | X.XX | X.XX× | 2-8× | ✓/✗ |
| Single → Batched | X.XX | X.XX | X.XX× | 2-4× | ✓/✗ |

## Validation Summary
- ✓ Fair baselines: All capsules use production-optimized code
- ✓ Statistical rigor: 95% CI, 1000+ iterations
- ✓ Hardware consistency: kindly-hub (AMD Ryzen 9 6900HX)
- ✓ Reproducibility: black_box(), fixed seeds, warm-up
- ✓/✗ Performance claims: Within expected ranges (see table above)
```

---

## References

- **B32 Framework**: `/home/samuel/CLAUDE.md` § Performance & Validation Standards
- **LOS Module**: `/home/samuel/Primitives/atomic_capsule/src/los/`
- **Remote Execution**: `/home/samuel/CLAUDE.md` § Infrastructure § remote-execution-protocol
- **Criterion Docs**: https://bheisler.github.io/criterion.rs/book/

---

**Last Updated**: 2025-11-25
**Status**: Production-ready B32 benchmarks
**Compliance**: UCE34 Q10/Q11/Q12, B32 K1-K70, T28 (benchmark coverage)
