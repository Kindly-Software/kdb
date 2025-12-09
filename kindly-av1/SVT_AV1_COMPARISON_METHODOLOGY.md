# SVT-AV1 vs kindly-av1 Performance Comparison Methodology

## Version 1.0 - 2025-11-26

---

## Executive Summary

This document establishes a fair, reproducible methodology for comparing **kindly-av1** (GPU-first, lockfree AV1 encoder) against **SVT-AV1** (Intel/Netflix CPU-optimized reference encoder). The methodology prioritizes:

1. **Fair Baselines**: Compare optimized SVT-AV1 (not strawman configurations)
2. **Reproducibility**: Documented hardware, test clips, exact commands
3. **Comprehensive Metrics**: FPS, PSNR, SSIM, VMAF, BD-rate
4. **Differentiator Validation**: kindly-av1's unique GPU-first + checkpoint/resume capabilities

---

## 1. SVT-AV1 Performance Baseline

### 1.1 Preset Performance Targets (1080p)

Based on [SVT-AV1 v2.1.0 analysis](https://wiki.x266.mov/blog/svt-av1-second-deep-dive) and [benchmark data](https://openbenchmarking.org/test/pts/svt-av1):

| Preset | Use Case | Expected FPS @ 1080p | Quality vs Speed | Notes |
|--------|----------|---------------------|------------------|-------|
| **0** | Slow (archival) | 5-15 fps | Highest quality | Rarely worth it (diminishing returns) |
| **4** | Medium (optimal) | 30-60 fps | **Best efficiency** | King of optimal encoding |
| **8** | Fast (production) | 100-200 fps | Good quality/speed | Real-time capable @ 1080p |
| **10** | Very Fast | 150-250 fps | Moderate quality | Low-end real-time |
| **12** | Fastest | 200-300 fps | Lower quality | Livestreaming |

**Hardware Baseline**: AMD Ryzen 9 6900HX (8C/16T, 64GB DDR5-4800) - kindly-hub server

**Specific Benchmark Results**:
- **Ryzen 5 5600G (12 threads)**: Preset 10, CRF 30 → **67 fps** @ 1080p
- **Xeon Scalable Ice Lake**: Preset 8 → **204 fps** @ 1080p (v0.9.0+)
- **Xeon Platinum 8380 2P**: Preset 8 → **95 fps** @ 4K (scales to ~380 fps @ 1080p)

**kindly-hub Expected Performance** (8C/16T, interpolated):
- **Preset 0**: 8-12 fps @ 1080p
- **Preset 4**: 40-55 fps @ 1080p
- **Preset 8**: 120-180 fps @ 1080p

### 1.2 Preset Trade-offs (v2.1.0)

**Optimal Presets** (per [Codec Wiki analysis](https://wiki.x266.mov/blog/svt-av1-second-deep-dive)):
- **Presets 2-4**: Best quality/speed trade-off for non-realtime
- **Presets 5-8**: Good compromise for users who find 2-4 too slow
- **Presets 9-12**: Real-time encoding @ 1080p (even on low-end hardware)

**v2.1.0 Changes**:
- **Improved**: Presets -1, 0, 1, 3, 4, 5, 6, 12 (better efficiency)
- **Regressed**: Presets 8, 10 (slightly worse)
- **Unchanged**: Presets 2, 9
- **Removed**: Presets 7, 13 (merged into 6, 12)

---

## 2. Test Content and Quality Metrics

### 2.1 Recommended Test Clips

Based on [SVT-AV1 animation benchmarks](https://wiki.x266.mov/blog/svt-av1-deep-dive) and [standard methodologies](https://www.probe.dev/resources/video-quality-metrics-analysis):

#### Tier 1: Animation (SVT-AV1 Reference Suite)
| Clip | Duration | Resolution | Characteristics | Purpose |
|------|----------|------------|-----------------|---------|
| **Blue Lock** | 13s | 1920x804 | Rapid camera movement, complex geometry, high contrast | Motion + detail stress |
| **Spy x Family ED** | 5s | 1080p | Extremely high dynamic noise | Noise handling |
| **Jigokuraku (Hell's Paradise)** | 12s | 1080p | Huge static grain, dark scenery, action | Grain + dark scene stress |
| **Garden of Sinners** | 5s | 1080p | Clean 3DCG, fast-paced, explosions | Clean motion + effects |

#### Tier 2: Live-Action (Industry Standard)
| Clip | Source | Characteristics | Purpose |
|------|--------|-----------------|---------|
| **Bosphorus 4K** | [OpenBenchmarking](https://openbenchmarking.org/test/pts/svt-av1) | 4K cityscape | Standard SVT-AV1 benchmark |
| **ForBiggerFun.mp4** | [video-quality-metrics](https://github.com/CrypticSignal/video-quality-metrics) | Sample footage | Tool testing |
| **Elecard Standard** | Custom | 1080p, 25fps, 10s, progressive | Quality metric validation |

#### Tier 3: Synthetic (Edge Cases)
- **Flat color gradients**: Banding detection (10-bit depth critical)
- **High-frequency patterns**: Aliasing/ringing artifacts
- **Extreme motion**: Motion estimation stress
- **Dark scenes**: Low-light compression

**Lossless Encoding**: Use `x264 --qp 0` or losslessly cut from source for ground truth.

### 2.2 Quality Metrics

#### Primary Metrics (Per-Clip)
| Metric | Tool | Range | Interpretation | Weighting |
|--------|------|-------|----------------|-----------|
| **VMAF** | FFmpeg libvmaf | 0-100 | Perceptual quality (Netflix standard) | 40% |
| **SSIMULACRA2** | Third-party | 0-100 | Psychovisual quality (modern) | 30% |
| **XPSNR** | FFmpeg | dB | Extended PSNR (perceptual weighting) | 20% |
| **Butteraugli** | Third-party | Distance | Psychovisual difference | 10% |

**Deprecated Metrics** (report but don't optimize for):
- **PSNR**: Poor correlation with MOS, [tunes for PSNR by default](https://streaminglearningcenter.com/articles/svt-av1-and-libaom-tune-for-psnr-by-default.html)
- **SSIM**: Better than PSNR but still limited
- **VMAF (luma-only)**: [Unreliable chroma handling](https://wiki.x266.mov/blog/svt-av1-second-deep-dive)

#### VMAF Configuration (Enhanced)
```bash
# Compute across all 3 color planes (not just luma)
# Weight: Y=0.5, Cb=0.25, Cr=0.25
# Use VMAF neg model with motion component disabled
ffmpeg -i reference.y4m -i encoded.ivf \
  -lavfi "[0:v][1:v]libvmaf=model=version=vmaf_v0.6.1neg:n_threads=8:motion_disabled=1" \
  -f null -
```

#### Aggregate Metrics
| Metric | Formula | Purpose |
|--------|---------|---------|
| **BD-rate** | Bjøntegaard Delta | Bitrate savings at same quality (%) |
| **BD-PSNR** | Bjøntegaard Delta | Quality gain at same bitrate (dB) |
| **Encoding Efficiency** | (Bitrate reduction %) / (FPS reduction %) | Time-quality trade-off |

---

## 3. Benchmark Commands

### 3.1 SVT-AV1 Encoding

#### Basic Single-Pass (CRF Mode)
```bash
# Preset 0 (Slow, archival)
SvtAv1EncApp -i input.y4m -b output_p0.ivf \
  --preset 0 --crf 30 --passes 1 --keyint -2 \
  --input-depth 10 --film-grain 0 --tune 0 \
  --lp 0  # Auto-detect parallelism

# Preset 4 (Medium, optimal)
SvtAv1EncApp -i input.y4m -b output_p4.ivf \
  --preset 4 --crf 30 --passes 1 --keyint -2 \
  --input-depth 10 --film-grain 0 --tune 0 --lp 0

# Preset 8 (Fast, production)
SvtAv1EncApp -i input.y4m -b output_p8.ivf \
  --preset 8 --crf 30 --passes 1 --keyint -2 \
  --input-depth 10 --film-grain 0 --tune 0 --lp 0
```

#### Two-Pass VBR (Target Bitrate)
```bash
SvtAv1EncApp -i input.y4m -b output.ivf \
  --preset 4 --rc 1 --tbr 5000 --passes 2 \
  --stats svtav1_2pass.log --keyint 240 \
  --input-depth 10 --film-grain 0 --tune 0
```

#### FFmpeg Pipeline (10-bit)
```bash
ffmpeg -i input.mp4 -map 0:v:0 -pix_fmt yuv420p10le -f yuv4mpegpipe -strict -1 - | \
SvtAv1EncApp -i stdin --preset 6 --keyint 240 --input-depth 10 \
  --crf 30 --rc 0 --passes 1 --film-grain 0 -b output.ivf
```

**Parameter Notes**:
- `--keyint -2`: ~5 second GOP (recommended for seekability)
- `--keyint -1`: Infinite GOP (CRF only, best compression)
- `--tune 0`: Psychovisual (VQ) tune (default: `--tune 1` for PSNR)
- `--film-grain 0`: Disable synthetic grain (cleaner comparison)
- `--input-depth 10`: 10-bit encoding (prevents banding @ low bitrate)

### 3.2 kindly-av1 Encoding (Placeholder)

**Note**: Commands below are hypothetical pending kindly-av1 implementation.

#### GPU-Accelerated Single-Pass
```bash
# Fast (GPU default, equivalent to SVT-AV1 preset 8)
kindly-av1 encode -i input.y4m -o output.ivf \
  --quality medium --crf 30 --gpu auto \
  --checkpoint checkpoint.bin

# Medium (GPU optimized, equivalent to SVT-AV1 preset 4)
kindly-av1 encode -i input.y4m -o output.ivf \
  --quality high --crf 30 --gpu auto \
  --checkpoint checkpoint.bin

# Slow (GPU exhaustive, equivalent to SVT-AV1 preset 0)
kindly-av1 encode -i input.y4m -o output.ivf \
  --quality max --crf 30 --gpu auto \
  --checkpoint checkpoint.bin
```

#### Checkpoint/Resume (Unique Feature)
```bash
# Start encoding
kindly-av1 encode -i input.y4m -o output.ivf \
  --quality medium --crf 30 --gpu auto \
  --checkpoint checkpoint.bin

# Resume after interruption (CTRL+C, power loss, etc.)
kindly-av1 resume --checkpoint checkpoint.bin \
  -o output.ivf
```

#### CPU Fallback (Fair Comparison)
```bash
# Force CPU-only mode (for apples-to-apples with SVT-AV1)
kindly-av1 encode -i input.y4m -o output.ivf \
  --quality medium --crf 30 --gpu disabled \
  --threads 16
```

---

## 4. kindly-av1 Differentiators

### 4.1 GPU-First Architecture

**Claim**: 100-1000× speedup via GPU acceleration (T7 Heterogeneous tier)

**Validation Strategy**:
1. **Baseline**: SVT-AV1 preset 8 @ 120-180 fps (8C/16T CPU)
2. **GPU Encoding**: kindly-av1 @ TBD fps (AMD 680M iGPU or discrete GPU)
3. **Target**: ≥10× speedup (1200+ fps @ 1080p) for T7 tier claim

**Hardware Requirements**:
- **kindly-hub iGPU**: AMD Radeon 680M (12 CUs, 2.4 GHz)
- **Discrete GPU (optional)**: AMD RX 6000/7000 series or NVIDIA RTX 3000/4000

**Quality Trade-off**:
- GPU encoders typically sacrifice 5-15% quality for speed
- **Validation**: VMAF delta < 3 points @ same bitrate
- **Mitigation**: Offer GPU + CPU hybrid mode (T6 Mixed tier)

### 4.2 Lockfree Architecture

**Claim**: <10ns coordination overhead (vs 1-10μs mutex in SVT-AV1)

**Validation Strategy**:
1. **Profiling**: `cargo flamegraph --release` to measure coordination cost
2. **Comparison**: SVT-AV1 pthread mutex overhead (~1-5μs per lock)
3. **Target**: 100-1000× lower coordination latency

**Metrics**:
- **Latency percentiles**: p50, p95, p99, p99.9 (lockfree should have tight distribution)
- **Scalability**: Linear speedup to 16+ cores (no lock contention)

### 4.3 Checkpoint/Resume

**Unique Capability**: No other AV1 encoder supports mid-encode resume

**Use Cases**:
1. **Long encodes**: Multi-hour 4K/8K jobs (power loss recovery)
2. **Distributed encoding**: Cloud spot instances (preemption tolerance)
3. **Interactive encoding**: Pause/resume for laptop battery management

**Validation Strategy**:
1. **Checkpoint overhead**: <1% FPS penalty for checkpoint writes
2. **Resume accuracy**: Bit-identical output after resume
3. **Checkpoint size**: <10MB for 1080p encode state

---

## 5. Benchmark Execution Protocol

### 5.1 Hardware Configuration

**Primary Benchmark Platform**: kindly-hub (192.168.0.38)
- **CPU**: AMD Ryzen 9 6900HX (8C/16T, 3.3-4.9 GHz)
- **RAM**: 64 GB DDR5-4800
- **GPU**: AMD Radeon 680M (iGPU, 12 CUs @ 2.4 GHz)
- **Storage**: NVMe SSD (minimize I/O bottleneck)
- **OS**: Ubuntu Server 24.04 LTS (kernel 6.14.0-36)

**Thermal Management**:
- Ensure CPU/GPU temps < 85°C (throttling affects benchmarks)
- 5-minute cooldown between runs
- Monitor with `sensors` or `rocm-smi`

### 5.2 Test Execution Steps

#### Step 1: Environment Setup
```bash
# SSH to kindly-hub
ssh samuel@kindly-hub

# Set CPU governor (performance mode)
sudo cpupower frequency-set -g performance

# Disable turbo boost (consistent clocks)
echo 0 | sudo tee /sys/devices/system/cpu/cpufreq/boost

# Clear filesystem caches
sudo sync; sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'
```

#### Step 2: Run SVT-AV1 Baseline
```bash
cd ~/Primitives/kindly-av1/benchmarks

# Preset 0, 4, 8 (3 runs each for 95% CI)
for preset in 0 4 8; do
  for run in 1 2 3; do
    /usr/bin/time -v SvtAv1EncApp -i bosphorus_1080p.y4m \
      -b svt_p${preset}_run${run}.ivf \
      --preset $preset --crf 30 --passes 1 --keyint -2 \
      --input-depth 10 --film-grain 0 --tune 0 \
      2>&1 | tee svt_p${preset}_run${run}.log
    sleep 300  # 5-minute cooldown
  done
done
```

#### Step 3: Run kindly-av1 Comparison
```bash
# GPU mode (primary comparison)
for quality in fast medium slow; do
  for run in 1 2 3; do
    /usr/bin/time -v kindly-av1 encode \
      -i bosphorus_1080p.y4m -o kindly_${quality}_run${run}.ivf \
      --quality $quality --crf 30 --gpu auto \
      --checkpoint checkpoint_${quality}_${run}.bin \
      2>&1 | tee kindly_${quality}_run${run}.log
    sleep 300
  done
done

# CPU-only mode (fair comparison)
for quality in fast medium slow; do
  for run in 1 2 3; do
    /usr/bin/time -v kindly-av1 encode \
      -i bosphorus_1080p.y4m -o kindly_cpu_${quality}_run${run}.ivf \
      --quality $quality --crf 30 --gpu disabled --threads 16 \
      2>&1 | tee kindly_cpu_${quality}_run${run}.log
    sleep 300
  done
done
```

#### Step 4: Quality Metrics
```bash
# VMAF (enhanced multi-plane)
for file in *.ivf; do
  ffmpeg -i bosphorus_1080p.y4m -i $file \
    -lavfi "[0:v][1:v]libvmaf=model=version=vmaf_v0.6.1neg:n_threads=8:motion_disabled=1" \
    -f null - 2>&1 | tee ${file%.ivf}_vmaf.log
done

# SSIMULACRA2 (if available)
for file in *.ivf; do
  ssimulacra2 bosphorus_1080p.y4m $file > ${file%.ivf}_ssim2.log
done

# XPSNR
for file in *.ivf; do
  ffmpeg -i bosphorus_1080p.y4m -i $file \
    -lavfi "[0:v][1:v]xpsnr=stats_file=${file%.ivf}_xpsnr.log" \
    -f null -
done
```

### 5.3 Data Collection

Extract from logs:
- **Encoding time** (wall clock): `/usr/bin/time -v` → "Elapsed (wall clock) time"
- **CPU utilization**: `/usr/bin/time -v` → "Percent of CPU this job got"
- **Memory usage**: `/usr/bin/time -v` → "Maximum resident set size (kbytes)"
- **FPS**: `SvtAv1EncApp` output → "Average Speed" or calculate from time + frame count
- **Bitrate**: `ffprobe` → "bit_rate" or `size / duration`
- **VMAF**: `libvmaf` output → "VMAF score"
- **File size**: `ls -lh` or `/usr/bin/time -v` → "File system outputs"

---

## 6. Results Reporting Format

### 6.1 Performance Summary Table

| Encoder | Preset/Quality | FPS (avg) | FPS (95% CI) | CPU % | Memory (MB) | Bitrate (Mbps) | File Size (MB) |
|---------|----------------|-----------|--------------|-------|-------------|----------------|----------------|
| SVT-AV1 | Preset 0 | TBD | [TBD, TBD] | TBD | TBD | TBD | TBD |
| SVT-AV1 | Preset 4 | TBD | [TBD, TBD] | TBD | TBD | TBD | TBD |
| SVT-AV1 | Preset 8 | TBD | [TBD, TBD] | TBD | TBD | TBD | TBD |
| kindly-av1 (GPU) | Fast | TBD | [TBD, TBD] | TBD | TBD | TBD | TBD |
| kindly-av1 (GPU) | Medium | TBD | [TBD, TBD] | TBD | TBD | TBD | TBD |
| kindly-av1 (GPU) | Slow | TBD | [TBD, TBD] | TBD | TBD | TBD | TBD |
| kindly-av1 (CPU) | Fast | TBD | [TBD, TBD] | TBD | TBD | TBD | TBD |
| kindly-av1 (CPU) | Medium | TBD | [TBD, TBD] | TBD | TBD | TBD | TBD |
| kindly-av1 (CPU) | Slow | TBD | [TBD, TBD] | TBD | TBD | TBD | TBD |

### 6.2 Quality Metrics Table

| Encoder | Preset/Quality | VMAF | SSIMULACRA2 | XPSNR (dB) | Butteraugli | Weighted Score |
|---------|----------------|------|-------------|------------|-------------|----------------|
| SVT-AV1 | Preset 0 | TBD | TBD | TBD | TBD | TBD |
| SVT-AV1 | Preset 4 | TBD | TBD | TBD | TBD | TBD |
| SVT-AV1 | Preset 8 | TBD | TBD | TBD | TBD | TBD |
| kindly-av1 (GPU) | Fast | TBD | TBD | TBD | TBD | TBD |
| kindly-av1 (GPU) | Medium | TBD | TBD | TBD | TBD | TBD |
| kindly-av1 (GPU) | Slow | TBD | TBD | TBD | TBD | TBD |

**Weighted Score**: 0.4×VMAF + 0.3×SSIMULACRA2 + 0.2×XPSNR + 0.1×Butteraugli (normalized)

### 6.3 Rate-Distortion Curves

Plot for each test clip:
- **X-axis**: Bitrate (Mbps)
- **Y-axis**: VMAF score
- **Lines**: SVT-AV1 presets 0/4/8 vs kindly-av1 qualities fast/medium/slow
- **Convex hull**: Pareto-optimal points (best quality for given bitrate)

### 6.4 BD-Rate Comparison

| Comparison | BD-rate (%) | BD-VMAF (points) | Interpretation |
|------------|-------------|------------------|----------------|
| kindly-av1 GPU Fast vs SVT-AV1 P8 | TBD | TBD | TBD |
| kindly-av1 GPU Medium vs SVT-AV1 P4 | TBD | TBD | TBD |
| kindly-av1 GPU Slow vs SVT-AV1 P0 | TBD | TBD | TBD |
| kindly-av1 CPU Medium vs SVT-AV1 P4 | TBD | TBD | TBD |

**Interpretation**:
- **BD-rate < 0**: kindly-av1 uses less bitrate for same quality (better)
- **BD-VMAF > 0**: kindly-av1 has higher quality at same bitrate (better)

---

## 7. Acceptance Criteria

### 7.1 Performance Targets (kindly-av1)

| Metric | Minimum Target | Stretch Goal | Status |
|--------|----------------|--------------|--------|
| **GPU Fast FPS** | 300 fps @ 1080p (2× SVT-AV1 P8) | 1200 fps (10× T7 tier) | TBD |
| **GPU Medium FPS** | 100 fps @ 1080p (2× SVT-AV1 P4) | 500 fps (10× T7 tier) | TBD |
| **GPU Slow FPS** | 20 fps @ 1080p (2× SVT-AV1 P0) | 100 fps (10× T7 tier) | TBD |
| **CPU Medium FPS** | 40 fps @ 1080p (0.8× SVT-AV1 P4) | 55 fps (1× SVT-AV1 P4) | TBD |

### 7.2 Quality Targets (kindly-av1)

| Metric | Minimum Target | Stretch Goal | Status |
|--------|----------------|--------------|--------|
| **VMAF delta** (vs SVT-AV1 same preset) | ≥ -3 points | ≥ 0 points | TBD |
| **BD-rate** (vs SVT-AV1) | ≥ -15% (15% worse) | ≥ 0% (equal) | TBD |
| **Checkpoint overhead** | ≤ 5% FPS penalty | ≤ 1% FPS penalty | TBD |

### 7.3 Framework Compliance

| Framework | Requirement | Validation | Status |
|-----------|-------------|------------|--------|
| **UCE34** | Q10-Q12 tier selection documented | See `CLAUDE.md` | TBD |
| **B32** | 95% CI, 1000+ iterations | 3 runs per config (bootstrap CI) | TBD |
| **T28** | 5-tier testing (unit/property/integration/production/determinism) | See `tests/` | TBD |
| **ASSUM** | 99.5%+ safe code, all assumptions verified | See `#ASSUME` tags | TBD |
| **I20** | Integration validation (20/20 questions) | See integration tests | TBD |

---

## 8. Timeline and Deliverables

### Phase 1: Baseline Validation (1 week)
- [ ] Install SVT-AV1 v2.1.0+ on kindly-hub
- [ ] Download test clips (Bosphorus 4K, animation suite)
- [ ] Run SVT-AV1 presets 0/4/8 (3 runs each, 9 total)
- [ ] Validate FPS within ±10% of expected values
- [ ] Establish quality baselines (VMAF, SSIMULACRA2, XPSNR)

### Phase 2: kindly-av1 Implementation (4-6 weeks)
- [ ] Implement basic AV1 encoder (GOP structure, frame types)
- [ ] Implement GPU kernels (intra prediction, transform, quantization)
- [ ] Implement checkpoint/resume system (T9 Persistent tier)
- [ ] Implement lockfree pipeline coordination (T1 Atomic tier)
- [ ] Achieve parity with SVT-AV1 preset 8 quality (VMAF ≥ -3)

### Phase 3: Performance Optimization (2-3 weeks)
- [ ] Profile with `cargo flamegraph` (identify 70%+ bottleneck)
- [ ] Apply T7 GPU optimizations (target 10× speedup)
- [ ] Validate 95% CI benchmarks (3+ runs per config)
- [ ] Measure checkpoint overhead (<5% target)
- [ ] Generate rate-distortion curves and BD-rate analysis

### Phase 4: Documentation and Release (1 week)
- [ ] Complete comparison report (this methodology + results)
- [ ] Document differentiators (GPU-first, checkpoint/resume, lockfree)
- [ ] Create demo videos (checkpoint recovery, real-time encoding)
- [ ] Publish results (blog post, GitHub README, papers)

**Total Estimated Time**: 8-11 weeks

---

## 9. Known Limitations and Mitigations

### 9.1 SVT-AV1 Advantages

| Advantage | Impact | kindly-av1 Mitigation |
|-----------|--------|----------------------|
| **Mature codebase** (5+ years) | Superior quality at same bitrate | Incremental quality improvements over time |
| **CPU parallelization** (16+ cores) | Excellent CPU scaling | Leverage GPU parallelism instead (1000+ cores) |
| **Two-pass encoding** | 5-10% bitrate savings | Implement two-pass mode (future work) |
| **Film grain synthesis** | Better compression on grainy content | Implement grain analysis/synthesis |
| **Extensive tuning** (13 presets) | Fine-grained speed/quality control | Start with 3 presets, expand later |

### 9.2 kindly-av1 Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **GPU quality regression** | High | Users reject encoder | Hybrid GPU+CPU mode for critical quality |
| **Checkpoint file size** | Medium | Storage overhead | Compress checkpoint state (zstd) |
| **GPU availability** | Medium | CPU fallback is slow | Document GPU requirements clearly |
| **AV1 spec compliance** | Low | Playback failures | Extensive conformance testing |

---

## 10. References

### SVT-AV1 Performance and Analysis
- [Observing SVT-AV1 v2.1.0's improvements: A New Deep Dive | Codec Wiki](https://wiki.x266.mov/blog/svt-av1-second-deep-dive)
- [Deep Dive into SVT-AV1's Evolution (Part 1): Presets Analysis from v2.0 to v3.0 | Codec Wiki](https://wiki.x266.mov/blog/svt-av1-fourth-deep-dive-p1)
- [SVT-AV1 Performance Benchmarks - OpenBenchmarking.org](https://openbenchmarking.org/performance/test/pts/svt-av1/0a8013353397457951acce4684a0eb9de59394b6)
- [Comparing SVT-AV1 Presets: Size, Quality, and Speed with CRF Variations - OTTVerse](https://ottverse.com/analysis-of-svt-av1-presets-and-crf-values/)
- [AV1 is ready for prime time: SVT-AV1 beats x265 and libvpx in quality, bitrate and speed | by Ewout ter Hoeven | Medium](https://medium.com/@ewoutterhoeven/av1-is-ready-for-prime-time-svt-av1-beats-x265-and-libvpx-in-quality-bitrate-and-speed-31c1960703db)

### Command Line and Usage
- [SvtAv1EncApp: manual page for SvtAv1EncApp 3.1.2 | Man Page | Commands | svt-av1 | ManKier](https://www.mankier.com/1/SvtAv1EncApp)
- [SVT-AV1 Encoding Guide · GitHub](https://gist.github.com/dvaupel/716598fc9e7c2d436b54ae00f7a34b95)
- [Docs/Parameters.md · master · Alliance for Open Media / SVT-AV1 · GitLab](https://gitlab.com/AOMediaCodec/SVT-AV1/-/blob/master/Docs/Parameters.md)

### Quality Metrics and Testing
- [Video Quality Metrics: PSNR, SSIM, and Advanced Quality Analysis](https://www.probe.dev/resources/video-quality-metrics-analysis)
- [SVT-AV1 and Libaom Tune for PSNR by Default - Streaming Learning Center](https://streaminglearningcenter.com/articles/svt-av1-and-libaom-tune-for-psnr-by-default.html)
- [GitHub - CrypticSignal/video-quality-metrics: Uses FFmpeg to benchmark video encoders to compare VMAF, SSIM and PSNR](https://github.com/CrypticSignal/video-quality-metrics)
- [Encoding Animation with SVT-AV1: A Deep Dive | Codec Wiki](https://wiki.x266.mov/blog/svt-av1-deep-dive)

### Hardware Benchmarks
- [Intel To Ring In 2022 With New, Faster AV1 Encoder Release - Phoronix](https://www.phoronix.com/review/intel-svt-088)
- [4K-Soft Ltd. | Intel and Netflix's SVT-AV1 gains up to 40% performance boost, AMD Ryzen will benefit too](https://4k-soft.com/news/intel-and-netflix-s-svt-av1-gains-up-to-40-performance-boost-amd-ryzen-will-benefit-too)

---

## Appendix A: Test Clip Preparation

### Converting to Y4M (10-bit)
```bash
# From MP4/MKV source
ffmpeg -i input.mp4 -pix_fmt yuv420p10le -strict -1 output.y4m

# From raw YUV
ffmpeg -f rawvideo -pix_fmt yuv420p -s:v 1920x1080 -r 25 -i input.yuv \
  -pix_fmt yuv420p10le -strict -1 output.y4m
```

### Lossless Encoding (Ground Truth)
```bash
# x264 lossless (for reference)
ffmpeg -i input.mp4 -c:v libx264 -qp 0 -preset veryslow -pix_fmt yuv420p10le reference.mkv

# Extract to Y4M
ffmpeg -i reference.mkv -pix_fmt yuv420p10le -strict -1 reference.y4m
```

### Clipping Test Segments
```bash
# Extract 10 seconds starting at 1:30
ffmpeg -ss 00:01:30 -i input.mp4 -t 10 -c copy clip_10s.mp4
```

---

## Appendix B: Automated Benchmark Script

```bash
#!/bin/bash
# benchmark_svt_vs_kindly.sh

set -euo pipefail

# Configuration
INPUT="bosphorus_1080p.y4m"
RUNS=3
COOLDOWN=300  # 5 minutes

# Ensure performance mode
sudo cpupower frequency-set -g performance
echo 0 | sudo tee /sys/devices/system/cpu/cpufreq/boost

# SVT-AV1 benchmarks
for preset in 0 4 8; do
  for run in $(seq 1 $RUNS); do
    echo "Running SVT-AV1 preset $preset, run $run..."
    sudo sync; sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'
    /usr/bin/time -v SvtAv1EncApp -i $INPUT \
      -b svt_p${preset}_run${run}.ivf \
      --preset $preset --crf 30 --passes 1 --keyint -2 \
      --input-depth 10 --film-grain 0 --tune 0 \
      2>&1 | tee svt_p${preset}_run${run}.log
    sleep $COOLDOWN
  done
done

# kindly-av1 GPU benchmarks
for quality in fast medium slow; do
  for run in $(seq 1 $RUNS); do
    echo "Running kindly-av1 GPU $quality, run $run..."
    sudo sync; sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'
    /usr/bin/time -v kindly-av1 encode -i $INPUT \
      -o kindly_gpu_${quality}_run${run}.ivf \
      --quality $quality --crf 30 --gpu auto \
      --checkpoint checkpoint_gpu_${quality}_${run}.bin \
      2>&1 | tee kindly_gpu_${quality}_run${run}.log
    sleep $COOLDOWN
  done
done

# kindly-av1 CPU benchmarks
for quality in fast medium slow; do
  for run in $(seq 1 $RUNS); do
    echo "Running kindly-av1 CPU $quality, run $run..."
    sudo sync; sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'
    /usr/bin/time -v kindly-av1 encode -i $INPUT \
      -o kindly_cpu_${quality}_run${run}.ivf \
      --quality $quality --crf 30 --gpu disabled --threads 16 \
      2>&1 | tee kindly_cpu_${quality}_run${run}.log
    sleep $COOLDOWN
  done
done

echo "Benchmarks complete! Analyze logs for results."
```

---

**End of Document**

**Version**: 1.0
**Date**: 2025-11-26
**Author**: kindly-av1 Development Team
**Framework Compliance**: UCE34, B32, T28, ASSUM, I20
