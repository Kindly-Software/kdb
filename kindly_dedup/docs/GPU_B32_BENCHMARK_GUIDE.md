# GPU B32 Benchmark Guide

Comprehensive guide for running B32-compliant GPU performance validation benchmarks.

## Overview

Two new benchmark suites validate GPU acceleration claims:

1. **gpu_igpu_validation**: iGPU-specific validation (150K docs/sec, 2× speedup)
2. **gpu_claim_validation**: Comprehensive PASS/FAIL matrix for all GPU tiers

## Hardware Requirements

### Primary Test Platform (kindly-hub)

- **Host**: kindly-hub (192.168.0.38)
- **GPU**: AMD Ryzen 9 6900HX (Radeon 680M iGPU)
- **RAM**: 64 GB DDR5-4800 (shared with iGPU)
- **OS**: Ubuntu Server 24.04
- **Backend**: Vulkan (primary)

### Additional Test Platforms

- **Entry GPU**: GTX 1650, RX 6400 (300K docs/sec, 4× target)
- **Mid GPU**: RTX 3060, RX 6700 (500K docs/sec, 7× target)
- **High GPU**: RTX 4090, RX 7900 XTX (1M docs/sec, 14× target)

## Running Benchmarks

### On kindly-hub (Remote)

```bash
# From local machine (192.168.0.103)
ssh samuel@kindly-hub "cd ~/Primitives/kindly_dedup && cargo bench --features 'gpu,benchmarking' --bench gpu_igpu_validation"

# Or for comprehensive validation
ssh samuel@kindly-hub "cd ~/Primitives/kindly_dedup && cargo bench --features 'gpu,benchmarking' --bench gpu_claim_validation"
```

### Local (if GPU available)

```bash
cd /home/samuel/Primitives/kindly_dedup

# iGPU validation
cargo bench --features "gpu,benchmarking" --bench gpu_igpu_validation

# Full claim validation
cargo bench --features "gpu,benchmarking" --bench gpu_claim_validation
```

### Optional: Pin CPU Frequency

For maximum reproducibility (reduces variance):

```bash
ssh samuel@kindly-hub "sudo cpupower frequency-set -g performance"
```

## Benchmark Suites

### 1. gpu_igpu_validation (iGPU-Specific)

**Purpose**: Validate iGPU claim on kindly-hub.

**Claim**:
- Throughput: 150K docs/sec
- Speedup: 2× vs CPU SIMD

**Success Criteria**:
- **PASS**: ≥120K docs/sec AND ≥1.8× speedup (80% throughput, 90% speedup)
- **MARGINAL**: 90-120K docs/sec OR 1.5-1.8× speedup (60-80% throughput, 75-90% speedup)
- **FAIL**: <90K docs/sec OR <1.5× speedup

**Benchmarks** (5 total):
1. `validate_igpu_throughput`: Throughput at 1K, 10K, 100K docs
2. `measure_cpu_baseline`: CPU SIMD baseline (fair comparison)
3. `validate_igpu_latency`: Latency percentiles (p50/p95/p99)
4. `validate_shared_memory_impact`: Shared memory scaling analysis
5. `generate_validation_report`: Final PASS/FAIL report

**Expected Runtime**: ~10-15 minutes

**Example Output**:
```text
=== iGPU Validation Report ===
Device: AMD Radeon 680M (iGPU)
Backend: Vulkan
Driver: Mesa 24.0.3

Performance Claims:
  Claimed Throughput: 150000 docs/sec
  Claimed Speedup: 2.0×

Measured Throughput:
  @ 1K docs:   155,342 docs/sec  ✅ PASS
  @ 10K docs:  148,891 docs/sec  ✅ PASS
  @ 100K docs: 142,567 docs/sec  ✅ PASS

CPU Baseline (SIMD): 72,450 docs/sec
Measured Speedup: 2.05× ✅

Latency Percentiles (10K docs):
  p50: 6.4μs
  p95: 8.1μs
  p99: 12.3μs

Thresholds:
  PASS: ≥120K docs/sec AND ≥1.8× speedup
  MARGINAL: 90-120K docs/sec OR 1.5-1.8× speedup
  FAIL: <90K docs/sec OR <1.5× speedup

Result: ✅ PASS
  Throughput Achievement: 99.3%
  Speedup Achievement: 102.5%
```

### 2. gpu_claim_validation (Comprehensive Matrix)

**Purpose**: Validate all GPU tier claims with PASS/FAIL classification.

**GPU Tiers**:

| Tier | Claimed Throughput | Claimed Speedup | PASS Threshold | MARGINAL Threshold |
|------|-------------------|-----------------|----------------|-------------------|
| iGPU | 150K docs/sec | 2× | ≥120K AND ≥1.8× | 90-120K OR 1.5-1.8× |
| Entry (GTX 1650) | 300K docs/sec | 4× | ≥240K AND ≥3.5× | 180-240K OR 3.0-3.5× |
| Mid (RTX 3060) | 500K docs/sec | 7× | ≥400K AND ≥6.0× | 300-400K OR 5.0-6.0× |
| High (RTX 4090) | 1M docs/sec | 14× | ≥800K AND ≥12.0× | 600-800K OR 10.0-12.0× |

**Benchmarks** (3 total):
1. `validate_gpu_claims`: Throughput + speedup at 1K, 10K, 100K docs
2. `validate_latency_percentiles`: Latency consistency (1000 samples)
3. `validate_throughput_stability`: Thermal stability (1000 iterations, 60s)

**Expected Runtime**: ~15-20 minutes

**Example Output**:
```text
=== GPU Claim Validation Report ===
Device: AMD Radeon 680M
Backend: Vulkan
Tier: Integrated

Performance Claims:
  Claimed Throughput: 150000 docs/sec
  Claimed Speedup: 2.0×

Measured Performance:
  GPU Throughput: 148891 docs/sec
  CPU Baseline: 72450 docs/sec
  Measured Speedup: 2.05×

Thresholds:
  PASS: ≥120000 docs/sec AND ≥1.80× speedup
  MARGINAL: ≥90000 docs/sec OR ≥1.50× speedup

Result: ✅ PASS
  Throughput Achievement: 99.3%
  Speedup Achievement: 102.5%
```

## B32 Framework Compliance

Both benchmarks are fully B32-compliant:

### Fair Baselines

- **CPU Baseline**: CPU SIMD path (portable_simd), NOT naive scalar
- **Same Algorithm**: GPU uses same MinHash algorithm as CPU (FNV-1a variant, 128 hash functions, golden ratio seeds)
- **Same Hardware**: All measurements on same machine for fair comparison

### Statistical Rigor

- **95% CI**: Criterion default (1000+ iterations where feasible)
- **Sample Size**: 100 samples for throughput, 1000 for latency percentiles
- **Measurement Time**: 10-60s per benchmark (sufficient for stability)
- **Warm-up**: 3 iterations before measurement (GPU context stable)

### Reproducibility

- **Fixed Seeds**: Deterministic token generation (`doc_id * 1000 + token_idx`)
- **Hardware Documentation**: Device, backend, driver version reported
- **CPU Frequency**: Optional pinning for consistency
- **Same Compiler**: Same rustc version across runs

### Honest Reporting

- **Clear Thresholds**: PASS (80% throughput, 85% speedup), MARGINAL (60-80%, 70-85%), FAIL (<60%, <70%)
- **Multiple Scales**: Test at 1K, 10K, 100K docs to ensure claims hold
- **Latency Percentiles**: Report p50/p95/p99 to catch outliers
- **Stability Testing**: 1000 iterations over 60s to detect thermal throttling

## Framework Compliance Matrix

| Framework | Requirement | Compliance |
|-----------|-------------|------------|
| **UCE34** | Q21-Q34 (T7 Heterogeneous tier validation) | ✅ Full compliance |
| **Chaos** | 100% lockfree GPU kernels, atomic CPU coordination | ✅ Zero mutex, AtomicU64 coordination |
| **ASSUM** | GPU availability runtime-checked, assumptions documented | ✅ Graceful CPU fallback |
| **B32** | 95% CI, fair baselines, reproducible results | ✅ See above |
| **T28** | 5-tier testing (unit/property/integration/production/determinism) | ✅ T28 Q21-Q28 compliance |

## Interpreting Results

### PASS Status

```
Result: ✅ PASS
  Throughput Achievement: 99.3%
  Speedup Achievement: 102.5%
```

**Interpretation**: GPU meets or exceeds performance claims. Ready for production deployment.

**Action**: Document validated performance in release notes.

### MARGINAL Status

```
Result: ⚠️  MARGINAL
  Throughput Achievement: 75.2%
  Speedup Achievement: 88.3%
```

**Interpretation**: GPU performance below expectations but not catastrophic. Investigate bottlenecks.

**Action**:
1. Check for thermal throttling (use `sensors` or `nvidia-smi`)
2. Verify GPU not shared with display rendering
3. Check driver version (outdated drivers may be slower)
4. Profile GPU kernel with `cargo flamegraph --features gpu`

### FAIL Status

```
Result: ❌ FAIL
  Throughput Achievement: 45.1%
  Speedup Achievement: 62.3%
```

**Interpretation**: GPU significantly underperforms. Do NOT deploy to production.

**Action**:
1. Verify GPU detected correctly (`wgpu` may select wrong adapter)
2. Check for software renderer fallback (Mesa llvmpipe, SwiftShader)
3. Validate GPU capabilities (insufficient workgroup size, buffer limits)
4. Review claims (may be unrealistic for this GPU class)

## Troubleshooting

### GPU Not Detected

```
⚠️  GPU not available - skipping iGPU validation: No suitable adapter found
```

**Solutions**:
1. Install Vulkan drivers: `sudo apt install vulkan-tools mesa-vulkan-drivers`
2. Verify GPU visible: `vulkaninfo | grep deviceName`
3. Check user permissions: `sudo usermod -a -G video samuel`
4. Try alternative backend: `WGPU_BACKEND=dx12` (Windows), `WGPU_BACKEND=metal` (macOS)

### Low Performance

```
Result: ⚠️  MARGINAL
  Throughput Achievement: 65.2%
```

**Diagnosis**:
1. Check CPU frequency pinning: `cpupower frequency-info`
2. Monitor GPU temperature: `sensors` or `nvidia-smi`
3. Check background processes: `top` (high CPU/GPU usage?)
4. Verify sufficient RAM: `free -h` (iGPU shares system RAM)

### High Variance

```
Criterion reports: 25% variance in throughput
```

**Diagnosis**:
1. Pin CPU frequency: `sudo cpupower frequency-set -g performance`
2. Disable CPU frequency scaling: `sudo systemctl disable ondemand`
3. Close background applications (browser, IDE, etc.)
4. Increase sample size: Edit `group.sample_size(500)` in benchmark

### Compilation Errors

```
error: could not compile `kindly_dedup` due to missing feature `gpu`
```

**Solution**: Ensure `gpu` and `benchmarking` features enabled:
```bash
cargo bench --features "gpu,benchmarking" --bench gpu_igpu_validation
```

## Output Files

### Criterion Reports

Location: `target/criterion/*/report/index.html`

- **Throughput graphs**: docs/sec vs document count
- **Latency histograms**: Distribution of per-batch latencies
- **Percentiles**: p50, p95, p99 latencies
- **Variance**: Statistical confidence intervals

Open with:
```bash
firefox target/criterion/igpu_throughput/report/index.html
```

### Raw Data

Location: `target/criterion/*/new/estimates.json`

JSON format with:
- Mean, median, std_dev, variance
- Lower/upper confidence bounds
- Sample count, measurement time

Extract with:
```bash
jq '.mean.point_estimate' target/criterion/igpu_throughput/igpu/1K/new/estimates.json
```

### Validation Reports

Printed to stdout during `generate_validation_report` benchmark.

Capture with:
```bash
cargo bench --features "gpu,benchmarking" --bench gpu_igpu_validation 2>&1 | tee validation_report.txt
```

## Next Steps

### After PASS Validation

1. **Document Results**: Add to `docs/GPU_VALIDATION_RESULTS.md`
2. **Update Claims**: Confirm or adjust performance claims in `src/gpu/capabilities.rs`
3. **Enable Production**: Mark GPU path as production-ready in `CLAUDE.md`
4. **Release Notes**: Include validated performance in v2.5 release notes

### After MARGINAL Validation

1. **Profile Bottlenecks**: `cargo flamegraph --features gpu`
2. **Optimize Kernels**: Review WGSL shader code (`src/gpu/kernels/minhash.wgsl`)
3. **Tune Batch Sizes**: Adjust `recommended_batch_size()` in `capabilities.rs`
4. **Re-benchmark**: Validate improvements with same benchmarks

### After FAIL Validation

1. **Investigate Root Cause**: Software renderer? Insufficient VRAM? Buggy driver?
2. **Adjust Claims**: Lower expectations or restrict to higher GPU tiers
3. **CPU Fallback**: Ensure graceful fallback to CPU SIMD path
4. **Document Limitations**: Add to `docs/GPU_COMPATIBILITY.md`

## References

- **B32 Framework**: `/home/samuel/CLAUDE.md` § Performance & Validation Standards
- **UCE34 Framework**: `/home/samuel/CLAUDE.md` § Mandatory Reading Framework
- **GPU Capabilities**: `src/gpu/capabilities.rs` (tier definitions, expected speedups)
- **GPU Validation**: `src/gpu/validation.rs` (CPU reference implementation)
- **Existing GPU Benchmarks**: `benches/gpu_b32_benchmark.rs` (kernel-level benchmarks)

## Appendix: Benchmark Code Structure

### gpu_igpu_validation.rs (718 lines)

- **validate_igpu_throughput** (87 lines): Throughput at 1K/10K/100K docs
- **measure_cpu_baseline** (45 lines): CPU SIMD baseline measurement
- **validate_igpu_latency** (63 lines): Latency percentiles (1000 samples)
- **validate_shared_memory_impact** (78 lines): Shared memory scaling
- **generate_validation_report** (152 lines): Final PASS/FAIL report

### gpu_claim_validation.rs (751 lines)

- **validate_gpu_claims** (127 lines): Multi-scale throughput + speedup
- **validate_latency_percentiles** (48 lines): Latency consistency test
- **validate_throughput_stability** (62 lines): Thermal stability test
- **PerformanceTier** (enum): Tier classification + threshold logic
- **ClaimValidationReport** (struct): Comprehensive validation report
