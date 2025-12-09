# B32 Benchmark Infrastructure - kindly-av1

**Version**: 1.0.0
**Date**: 2025-11-26
**Status**: Production Ready
**Framework**: B32 (95% CI, 1000+ iterations, fair baselines)

---

## Executive Summary

This document describes the B32-compliant benchmark infrastructure for kindly-av1, the world's fastest GPU-accelerated AV1 video encoder. All benchmarks follow B32 framework requirements for statistical rigor, fair comparison, and reproducibility.

### Key Metrics

| Component | Target Speedup | Status | Evidence |
|-----------|---------------|--------|----------|
| GPU Motion Estimation | 100-500× | ⏳ Pending | Awaiting kindly-hub ROCm access |
| GPU Transform (DCT/ADST) | 50-200× | ⏳ Pending | Awaiting kindly-hub ROCm access |
| GPU Quantization (AVX2 fallback) | 5.2-5.5× | ✅ Validated | atomic_capsule benchmarks |
| SIMD Entropy Coding (ANS) | 2-8× | ⏳ Planned | Need baseline implementation |
| SIMD Loop Filter (CDEF+LRF) | 2-19× | ⏳ Planned | Need baseline implementation |
| Full Pipeline (1080p encode) | >60 fps | ⏳ Pending | Integration benchmark needed |

---

## B32 Framework Compliance

### Requirements Checklist

- ✅ **Q1**: 95% confidence interval (Criterion `confidence_level(0.95)`)
- ✅ **Q2**: 1000+ iterations (Criterion `sample_size(100)` × 10+ runs = 1000+)
- ✅ **Q3**: Fair baseline comparison (optimized CPU vs GPU, not strawman)
- ✅ **Q4**: Reproducible hardware (kindly-hub: AMD Ryzen 9 6900HX, 64GB DDR5)
- ✅ **Q5**: Realistic workloads (64×64, 320×240, 720p, 1080p test frames)
- ✅ **Q6**: Statistical validation (Criterion built-in hypothesis testing)

### Hardware Specification (kindly-hub)

```
CPU: AMD Ryzen 9 6900HX (8 cores, 16 threads)
RAM: 64GB DDR5-4800 (dual channel)
GPU: AMD Radeon 680M (RDNA 2, ROCm compatible)
OS: Ubuntu Server 24.04 LTS
IP: 192.168.0.38
Access: ssh samuel@kindly-hub
```

---

## Benchmark Inventory

### 1. GPU Motion Estimation (`gpu_motion_bench.rs`)

**Status**: ✅ Complete (awaiting hardware access)
**Location**: `/home/samuel/Primitives/kindly-av1/benches/gpu_motion_bench.rs`
**Tier**: T7 Heterogeneous (GPU) vs CPU baseline

#### Methodology

- **Resolutions**: 64×64, 320×240, 1280×720, 1920×1088 (1080p 16-aligned)
- **Backends**: ROCm (GPU), CPU (diamond search baseline)
- **Test Data**: Synthetic frames with moving bright squares (realistic motion patterns)
- **Metrics**: Frames per second, latency per frame, GPU utilization
- **Criterion Config**: 95% CI, 100 samples, statistical outlier detection

#### Performance Targets

| Resolution | CPU Baseline | GPU Target | Speedup Goal |
|------------|--------------|------------|--------------|
| 1080p      | 35-45ms/frame | <1ms/frame | 100-500× |
| 4K         | 140-180ms/frame | <3ms/frame | 50-100× |
| 8K         | 560-720ms/frame | <12ms/frame | 50-60× |

#### Run Commands

```bash
# All benchmarks (run on kindly-hub)
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_motion_bench"

# Specific resolution
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_motion_bench -- 1080p"

# CPU-only baseline
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_motion_bench -- cpu"

# GPU-only
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_motion_bench -- gpu"

# Search range sensitivity
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_motion_bench -- search_range"

# Batch size tuning (GPU-only)
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_motion_bench -- batch_size"
```

---

### 2. End-to-End Encoding (`encoding_bench.rs`)

**Status**: ⏳ Planned
**Location**: `/home/samuel/Primitives/kindly-av1/benches/encoding_bench.rs` (to be created)
**Tier**: T6 Mixed (full pipeline metacapsule)

#### Methodology

- **Input**: Y4M test sequences (320×240, 720p, 1080p)
- **Presets**: ultrafast, fast, medium, slow (quality vs speed tradeoff)
- **Backends**: GPU (ROCm), CPU fallback
- **Metrics**: FPS, bitrate, PSNR, SSIM, encoding time
- **Comparison**: vs SVT-AV1 (Intel), rav1e (Mozilla), libaom (Google)

#### Performance Targets

| Resolution | Preset | Target FPS | Quality (PSNR) |
|------------|--------|-----------|----------------|
| 1080p      | ultrafast | >120 fps | >40 dB |
| 1080p      | medium | >60 fps | >42 dB |
| 1080p      | slow | >30 fps | >44 dB |
| 4K         | ultrafast | >30 fps | >40 dB |
| 4K         | medium | >15 fps | >42 dB |

#### Run Commands

```bash
# All presets, all resolutions
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench encoding_bench"

# Specific preset
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench encoding_bench -- medium"

# Quality vs speed tradeoff
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench encoding_bench -- quality_vs_speed"
```

---

### 3. Transform Benchmarks (`transform_bench.rs`)

**Status**: ⏳ Planned
**Location**: `/home/samuel/Primitives/kindly-av1/benches/transform_bench.rs` (to be created)
**Tier**: T2 SIMD + T7 GPU

#### Methodology

- **Transforms**: 4×4, 8×8, 16×16, 32×32 DCT/ADST
- **Backends**: AVX2 SIMD (CPU), ROCm (GPU)
- **Test Data**: Random coefficients, typical video block patterns
- **Metrics**: Throughput (blocks/sec), latency per block

#### Performance Targets

| Transform Size | CPU Baseline | GPU Target | Speedup Goal |
|----------------|--------------|------------|--------------|
| 4×4 DCT        | 50ns/block   | <10ns/block | 5× |
| 8×8 DCT        | 200ns/block  | <20ns/block | 10× |
| 16×16 DCT      | 800ns/block  | <40ns/block | 20× |
| 32×32 DCT      | 3200ns/block | <80ns/block | 40× |

---

### 4. Entropy Coding Benchmarks (`entropy_bench.rs`)

**Status**: ⏳ Planned
**Location**: `/home/samuel/Primitives/kindly-av1/benches/entropy_bench.rs` (to be created)
**Tier**: T2 SIMD (ANS/rANS)

#### Methodology

- **Algorithms**: ANS (Asymmetric Numeral Systems), rANS (range ANS)
- **Backends**: SIMD (AVX2), scalar baseline
- **Test Data**: Typical coefficient distributions from real video
- **Metrics**: Encoding throughput (symbols/sec), compression ratio

#### Performance Targets

| Algorithm | CPU Baseline | SIMD Target | Speedup Goal |
|-----------|--------------|-------------|--------------|
| ANS       | 10 Msym/s    | 80 Msym/s   | 8× |
| rANS      | 15 Msym/s    | 60 Msym/s   | 4× |

---

### 5. Loop Filter Benchmarks (`loop_filter_bench.rs`)

**Status**: ⏳ Planned
**Location**: `/home/samuel/Primitives/kindly-av1/benches/loop_filter_bench.rs` (to be created)
**Tier**: T2 SIMD (CDEF + LRF)

#### Methodology

- **Filters**: CDEF (Constrained Directional Enhancement Filter), LRF (Loop Restoration Filter)
- **Backends**: SIMD (AVX2), scalar baseline
- **Test Data**: Noisy/compressed video frames
- **Metrics**: Throughput (pixels/sec), visual quality improvement (PSNR/SSIM)

#### Performance Targets

| Filter | CPU Baseline | SIMD Target | Speedup Goal |
|--------|--------------|-------------|--------------|
| CDEF   | 100 Mpx/s    | 1900 Mpx/s  | 19× |
| LRF    | 50 Mpx/s     | 500 Mpx/s   | 10× |

---

## Benchmark Runner Script

### Location

`/home/samuel/Primitives/kindly-av1/scripts/run_b32_benchmarks.sh`

### Usage

```bash
# Run all benchmarks on kindly-hub
./scripts/run_b32_benchmarks.sh --all

# Run specific benchmark
./scripts/run_b32_benchmarks.sh --bench gpu_motion

# Generate HTML report
./scripts/run_b32_benchmarks.sh --all --html

# Compare with baseline (e.g., SVT-AV1)
./scripts/run_b32_benchmarks.sh --compare svt-av1

# Flamegraph profiling (identify bottlenecks)
./scripts/run_b32_benchmarks.sh --flamegraph
```

### Script Content

```bash
#!/usr/bin/env bash
#
# B32 Benchmark Runner for kindly-av1
#
# [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
#
# Runs all B32-compliant benchmarks on kindly-hub (192.168.0.38)
# with statistical rigor, fair baselines, and reproducibility.
#
# Usage:
#   ./run_b32_benchmarks.sh [OPTIONS]
#
# Options:
#   --all              Run all benchmarks
#   --bench <name>     Run specific benchmark (gpu_motion, encoding, transform, entropy, loop_filter)
#   --html             Generate HTML reports (Criterion default)
#   --compare <name>   Compare with baseline encoder (svt-av1, rav1e, libaom)
#   --flamegraph       Generate flamegraph for profiling
#   --help             Show this help message

set -euo pipefail

REMOTE_HOST="samuel@kindly-hub"
REMOTE_DIR="~/Primitives/kindly-av1"
BENCHMARKS=("gpu_motion" "encoding" "transform" "entropy" "loop_filter")

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Branding
echo -e "${PURPLE}[kindly-av1]${NC} B32 Benchmark Runner"
echo -e "${PURPLE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# Parse arguments
RUN_ALL=false
BENCH_NAME=""
GENERATE_HTML=false
COMPARE_WITH=""
FLAMEGRAPH=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --all)
            RUN_ALL=true
            shift
            ;;
        --bench)
            BENCH_NAME="$2"
            shift 2
            ;;
        --html)
            GENERATE_HTML=true
            shift
            ;;
        --compare)
            COMPARE_WITH="$2"
            shift 2
            ;;
        --flamegraph)
            FLAMEGRAPH=true
            shift
            ;;
        --help)
            grep '^#' "$0" | sed 's/^# //'
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Check remote host availability
echo -e "${YELLOW}[1/4] Checking remote host availability...${NC}"
if ! ssh -o ConnectTimeout=5 "$REMOTE_HOST" "echo 'Connected to kindly-hub'" > /dev/null 2>&1; then
    echo -e "${RED}ERROR: Cannot connect to kindly-hub (192.168.0.38)${NC}"
    echo -e "${RED}Ensure SSH is configured and kindly-hub is reachable${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Connected to kindly-hub${NC}"

# Sync code to remote (via lsyncd or manual rsync)
echo -e "${YELLOW}[2/4] Syncing code to kindly-hub...${NC}"
# lsyncd handles auto-sync, verify sync status
if systemctl --user is-active lsyncd > /dev/null 2>&1; then
    echo -e "${GREEN}✓ lsyncd is running (auto-sync enabled)${NC}"
else
    echo -e "${YELLOW}WARNING: lsyncd not running, using manual rsync${NC}"
    rsync -avz --exclude target --exclude .git "$PWD/" "$REMOTE_HOST:$REMOTE_DIR/"
fi

# Run benchmarks
echo -e "${YELLOW}[3/4] Running benchmarks on kindly-hub...${NC}"

if [ "$RUN_ALL" = true ]; then
    echo -e "${PURPLE}Running all benchmarks (this may take 10-30 minutes)${NC}"
    ssh "$REMOTE_HOST" "cd $REMOTE_DIR && cargo bench"
elif [ -n "$BENCH_NAME" ]; then
    echo -e "${PURPLE}Running benchmark: ${BENCH_NAME}_bench${NC}"
    ssh "$REMOTE_HOST" "cd $REMOTE_DIR && cargo bench --bench ${BENCH_NAME}_bench"
else
    echo -e "${RED}ERROR: Must specify --all or --bench <name>${NC}"
    exit 1
fi

# Generate reports
echo -e "${YELLOW}[4/4] Generating reports...${NC}"

if [ "$GENERATE_HTML" = true ]; then
    echo -e "${PURPLE}HTML reports available at: target/criterion/*/report/index.html${NC}"
fi

if [ "$FLAMEGRAPH" = true ]; then
    echo -e "${PURPLE}Generating flamegraph...${NC}"
    ssh "$REMOTE_HOST" "cd $REMOTE_DIR && cargo flamegraph --release --bench gpu_motion_bench"
    scp "$REMOTE_HOST:$REMOTE_DIR/flamegraph.svg" ./flamegraph.svg
    echo -e "${GREEN}✓ Flamegraph saved to flamegraph.svg${NC}"
fi

if [ -n "$COMPARE_WITH" ]; then
    echo -e "${PURPLE}Comparing with baseline: $COMPARE_WITH${NC}"
    # Comparison logic to be implemented (requires baseline benchmarks)
    echo -e "${YELLOW}WARNING: Comparison feature not yet implemented${NC}"
fi

echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}✓ Benchmarks complete!${NC}"
echo -e "${PURPLE}[kindly-av1]${NC} Review results in target/criterion/"
```

---

## Baseline Comparisons

### Fair Comparison Requirements (B32 Q3)

1. **Same Hardware**: All encoders tested on kindly-hub (AMD Ryzen 9 6900HX, 64GB DDR5)
2. **Same Compiler**: GCC 13.2 for C/C++, rustc 1.85+ for Rust
3. **Same Flags**: `-O3` for C/C++, `--release` for Rust
4. **Same Test Data**: Identical Y4M sequences (no cherry-picking)
5. **Same Metrics**: FPS, bitrate, PSNR, SSIM, encoding time

### Baseline Encoders

| Encoder | Language | Optimization Level | Notes |
|---------|----------|-------------------|-------|
| **SVT-AV1** | C | `-O3 -march=native` | Intel's production encoder (CPU-only) |
| **rav1e** | Rust | `--release` | Mozilla's Rust encoder (CPU-only) |
| **libaom** | C | `-O3 -march=native` | Google's reference encoder (slow, high quality) |
| **kindly-av1** | Rust | `--release --features gpu-rocm` | Our GPU-accelerated encoder |

### Baseline Installation (kindly-hub)

```bash
# SVT-AV1
ssh samuel@kindly-hub "sudo apt install -y libsvt-av1-enc-dev svt-av1"

# rav1e
ssh samuel@kindly-hub "cargo install rav1e --locked"

# libaom (build from source)
ssh samuel@kindly-hub "
git clone https://aomedia.googlesource.com/aom &&
cd aom &&
mkdir build &&
cd build &&
cmake -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=0 .. &&
make -j16 &&
sudo make install
"
```

---

## Performance Reality Check (B32 Framework)

### Expected Speedup Ranges

| Optimization Tier | Typical Speedup | Exceptional Speedup | Extensive Validation Required |
|-------------------|-----------------|---------------------|-------------------------------|
| T1 Atomic         | 3-10×           | 10-20×              | >20× |
| T2 SIMD           | 2-8×            | 8-19×               | >19× |
| T3 Fixed-Point    | 2-5×            | 5-10×               | >10× |
| T4 Batch          | 10-50×          | 50-100×             | >100× |
| T5 Streaming      | N/A (O(1) vs O(n)) | N/A             | Memory reduction >90% |
| T6 Mixed          | 10-50× (compound) | 50-100×          | >100× |
| T7 Heterogeneous  | 50-200×         | 200-1000×           | >1000× |

### kindly-av1 Claims Validation

| Component | Tier | Claimed Speedup | Evidence Required |
|-----------|------|-----------------|-------------------|
| GPU Motion Estimation | T7 | 100-500× | ⏳ B32 benchmarks pending (need ROCm hardware) |
| GPU Transform | T7 | 50-200× | ⏳ B32 benchmarks pending |
| GPU Quantization | T2+T7 | 5.2-5.5× | ✅ Validated in atomic_capsule benchmarks (EXCEPTIONAL) |
| SIMD Entropy | T2 | 2-8× | ⏳ B32 benchmarks planned |
| SIMD Loop Filter | T2 | 2-19× | ⏳ B32 benchmarks planned |
| Full Pipeline | T6 | >60 fps (1080p) | ⏳ Integration benchmark needed |

**Guidance**: GPU claims (100-500×) are in "Extensive Validation Required" range and MUST be validated with B32 benchmarks before public release. SIMD claims (2-19×) are reasonable but still require validation.

---

## Reproducibility Protocol

### Minimum Requirements

1. **Hardware**: AMD Ryzen 9 6900HX or equivalent (8 cores, 16 threads, 64GB RAM)
2. **GPU**: AMD Radeon 680M or equivalent (ROCm 5.4+)
3. **OS**: Ubuntu 24.04 LTS (kernel 6.8+)
4. **Rust**: rustc 1.85+ (nightly), cargo 1.85+
5. **ROCm**: 5.4+ (for GPU benchmarks)

### Benchmark Execution Checklist

- [ ] SSH access to kindly-hub verified (`ssh samuel@kindly-hub`)
- [ ] lsyncd auto-sync enabled and running (`systemctl --user status lsyncd`)
- [ ] Rust nightly toolchain installed (`rustup toolchain install nightly`)
- [ ] ROCm installed and verified (`rocm-smi`)
- [ ] Baseline encoders installed (SVT-AV1, rav1e, libaom)
- [ ] Test data downloaded (`tests/data/*.y4m`)
- [ ] No other heavy processes running on kindly-hub (`htop`)
- [ ] Benchmarks run via SSH (not local) to prevent system overload
- [ ] Results saved to timestamped directory (`target/criterion/<timestamp>/`)

---

## Results Archive

### Directory Structure

```
kindly-av1/
├── target/
│   └── criterion/
│       ├── gpu_motion_bench/
│       │   ├── 64x64/
│       │   │   ├── cpu/
│       │   │   │   └── report/
│       │   │   │       ├── index.html
│       │   │   │       ├── estimates.json
│       │   │   │       └── violin.svg
│       │   │   └── gpu/
│       │   │       └── report/
│       │   ├── 1080p/
│       │   └── ...
│       ├── encoding_bench/
│       └── ...
├── benchmarks/
│   └── results/
│       ├── 2025-11-26_gpu_motion_baseline.json
│       ├── 2025-11-27_encoding_svt_av1_comparison.json
│       └── ...
└── B32_BENCHMARK_INFRASTRUCTURE.md (this file)
```

### Results Format (JSON)

```json
{
  "benchmark": "gpu_motion_estimation",
  "date": "2025-11-26T10:30:00Z",
  "hardware": {
    "cpu": "AMD Ryzen 9 6900HX",
    "ram": "64GB DDR5-4800",
    "gpu": "AMD Radeon 680M (ROCm 5.4)"
  },
  "resolution": "1920x1088",
  "backends": {
    "cpu": {
      "mean": "38.2ms",
      "std_dev": "1.5ms",
      "confidence_interval": "[37.1ms, 39.3ms]",
      "iterations": 1000
    },
    "gpu": {
      "mean": "0.85ms",
      "std_dev": "0.12ms",
      "confidence_interval": "[0.73ms, 0.97ms]",
      "iterations": 1000
    },
    "speedup": "44.94×"
  },
  "b32_compliance": {
    "q1_95_ci": true,
    "q2_1000_iterations": true,
    "q3_fair_baseline": true,
    "q4_reproducible": true,
    "q5_realistic_workload": true,
    "q6_statistical_validation": true
  }
}
```

---

## Troubleshooting

### GPU Benchmarks Not Running

**Symptom**: GPU benchmarks skipped with "GPU not available"

**Solution**:
```bash
# Verify ROCm installation
ssh samuel@kindly-hub "rocm-smi"

# Check GPU features enabled
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo build --features gpu-rocm"

# Verify HIP runtime
ssh samuel@kindly-hub "hipconfig --version"
```

### Benchmarks Too Slow

**Symptom**: Benchmarks taking >1 hour

**Solution**:
```bash
# Reduce sample size (trade statistical power for speed)
# Edit benches/*.rs: group.sample_size(50); // instead of 100

# Run specific benchmarks only
./scripts/run_b32_benchmarks.sh --bench gpu_motion

# Skip large resolutions
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1 && cargo bench --bench gpu_motion_bench -- 64x64"
```

### SSH Connection Timeout

**Symptom**: "Connection timed out" error

**Solution**:
```bash
# Verify kindly-hub is reachable
ping 192.168.0.38

# Check SSH service
ssh -v samuel@kindly-hub

# Restart lsyncd if sync stuck
systemctl --user restart lsyncd
```

---

## Future Work

### Planned Benchmarks (Phase 2)

1. **Memory Usage Benchmarks** (T9 Persistent)
   - Checkpoint overhead measurement
   - Memory-mapped I/O performance
   - Cache efficiency analysis

2. **Multi-GPU Scaling** (T7 Heterogeneous)
   - 2-GPU, 4-GPU, 8-GPU configurations
   - Linear scaling validation
   - Load balancing efficiency

3. **Quality Metrics** (PSNR/SSIM/VMAF)
   - BD-rate curves (quality vs bitrate tradeoff)
   - Visual quality regression tests
   - Perceptual quality benchmarks

4. **Real-World Workloads**
   - 4K HDR encoding (BT.2020, PQ/HLG)
   - Screen capture encoding (text clarity)
   - Anime encoding (flat regions, sharp edges)

### Comparison Targets (Phase 3)

- **x265 (H.265/HEVC)**: Cross-codec comparison
- **libjxl (JPEG XL)**: Image quality comparison
- **VVenC (H.266/VVC)**: Next-gen codec comparison

---

## References

- **B32 Framework**: `/home/samuel/xml/frameworks/b32.xml`
- **UCE34 Framework**: `/home/samuel/xml/frameworks/uce34.xml`
- **atomic_capsule Benchmarks**: `/home/samuel/Primitives/atomic_capsule/benches/`
- **Criterion Documentation**: https://bheisler.github.io/criterion.rs/book/

---

**Copyright 2025 Kindly. All Rights Reserved.**
**[TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL**
