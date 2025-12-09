# ROCm Profiling Guide for Video Encoding Kernels

**Target**: AMD Ryzen 9 6900HX RDNA2 iGPU (Radeon 680M, gfx1030)
**Date**: 2025-12-01

---

## Quick Start (30 seconds)

```bash
# Basic timing profile
rocprof --timestamp on --basenames on -o timing.csv ./encoder --frames 10

# Hardware counters (occupancy, memory)
rocprof -i metrics.txt -o counters.csv ./encoder --frames 100

# Visual profiling (RGP)
rocprof --sys-trace --roctx-trace ./encoder --frames 10
# Open encoder.rpd in RGP GUI
```

---

## rocprof: Hardware Counter Collection

### Metrics File Templates

#### 1. Occupancy & Compute (`occupancy_metrics.txt`)
```
pmc: Wavefronts VALUInsts SALUInsts
pmc: VALUUtilization VALUBusy SALUBusy
pmc: GPUBusy
```

**Usage**:
```bash
rocprof -i occupancy_metrics.txt -o occupancy.csv ./encoder

# Analyze results
awk -F, 'NR>1 {valu+=$5; gpu+=$7} END {
    print "Avg VALU Utilization:", valu/(NR-1) "%"
    print "Avg GPU Busy:", gpu/(NR-1) "%"
}' occupancy.csv
```

**Target Metrics**:
- `VALUUtilization`: >80% (minimize idle VALU cycles)
- `VALUBusy`: >85% (VALU actively executing)
- `GPUBusy`: >90% (GPU not stalling on memory/control)

---

#### 2. LDS & Memory (`memory_metrics.txt`)
```
pmc: LDSInsts LDSBankConflict
pmc: FetchSize WriteSize
pmc: TCC_HIT_sum TCC_MISS_sum
pmc: TCC_EA_WRREQ_sum TCC_EA_WRREQ_64B_sum
pmc: MemUnitStalled MemUnitBusy WriteUnitStalled
```

**Usage**:
```bash
rocprof -i memory_metrics.txt -o memory.csv ./encoder

# Analyze L2 hit rate
awk -F, 'NR>1 {hit+=$4; miss+=$5} END {
    total=hit+miss
    print "L2 Hit Rate:", (hit/total)*100 "%"
}' memory.csv

# Analyze LDS conflicts
awk -F, 'NR>1 {conflict+=$2} END {
    print "Avg LDS Bank Conflict %:", conflict/(NR-1)
}' memory.csv
```

**Target Metrics**:
- `LDSBankConflict`: <5% (XOR-swizzle should eliminate)
- L2 hit rate: `TCC_HIT / (TCC_HIT + TCC_MISS)` >90%
- `MemUnitStalled`: Minimize (indicates memory bottleneck)

---

#### 3. Comprehensive Profile (`all_metrics.txt`)
```
# Pass 1: Compute
pmc: Wavefronts VALUInsts SALUInsts SFetchInsts FlatVMemInsts
pmc: VALUUtilization VALUBusy SALUBusy

# Pass 2: Memory
pmc: LDSInsts LDSBankConflict
pmc: FetchSize WriteSize L2CacheHit

# Pass 3: Cache
pmc: TCC_HIT_sum TCC_MISS_sum
pmc: TCC_EA_WRREQ_sum TCC_EA_WRREQ_64B_sum

# Pass 4: Stalls
pmc: MemUnitStalled MemUnitBusy WriteUnitStalled
pmc: GPUBusy
```

**Note**: Multi-pass profiling required (hardware counter limit). Each `pmc:` line is one pass.

**Usage**:
```bash
rocprof -i all_metrics.txt -o comprehensive.csv ./encoder --frames 100
```

---

### Kernel-Specific Filtering

```bash
# Profile only motion_estimation kernel
rocprof -i metrics.txt --kernel-names motion_estimation -o me_profile.csv ./encoder

# Profile multiple specific kernels
rocprof -i metrics.txt --kernel-names "motion_estimation transform_kernel" -o profile.csv ./encoder
```

---

### Timeline Tracing (HSA/HIP API)

```bash
# Trace HIP API calls + kernel launches
rocprof --hip-trace -o hip_trace.csv ./encoder

# Trace HSA low-level events
rocprof --hsa-trace -o hsa_trace.csv ./encoder

# Combine API + kernel timing
rocprof --hip-trace --timestamp on --basenames on -o timeline.csv ./encoder
```

**Output columns**:
- `Name`: Kernel or API function name
- `Start(ns)`: Timestamp in nanoseconds
- `Dur(ns)`: Duration in nanoseconds
- `Queue`: HIP stream ID

**Analyze overlap**:
```python
import pandas as pd
df = pd.read_csv('timeline.csv')

# Find overlapping kernels (concurrent execution)
for i, row in df.iterrows():
    overlaps = df[(df['Start(ns)'] < row['Start(ns)'] + row['Dur(ns)']) &
                  (df['Start(ns)'] + df['Dur(ns)'] > row['Start(ns)']) &
                  (df.index != i)]
    if len(overlaps) > 0:
        print(f"{row['Name']} overlaps with {len(overlaps)} kernels")
```

---

## rocprof-compute: Advanced Analysis

### System Speed-of-Light (SOL)

```bash
# Profile with SOL analysis
rocprof-compute profile -n encoder_sol --no-roof -- ./encoder --frames 10

# Analyze system-level SOL
rocprof-compute analyze -p workloads/encoder_sol/gfx1030/ --sol
```

**Output**:
```
System Speed-of-Light (RDNA2 gfx1030):
- Compute (VALU):        78.3% of peak  [Target: >80%]
- Memory (L2 → HBM):     45.2% of peak  [Good: <70%]
- LDS Utilization:       12.1% of peak  [Good: <50%]
- Wavefront Occupancy:   75.0% (12/16)  [Target: >75%]
```

**Interpretation**:
- Compute SOL high (>80%): Compute-bound (good for video encoding)
- Memory SOL high (>70%): Memory-bound (optimize coalescing, L2 hit rate)
- Occupancy low (<50%): Reduce VGPR pressure, increase work per thread

---

### Kernel-Level SOL

```bash
# Profile specific kernel
rocprof-compute profile -n me_kernel -b motion_estimation --no-roof -- ./encoder

# Analyze kernel SOL
rocprof-compute analyze -p workloads/me_kernel/gfx1030/ -b motion_estimation --sol
```

**Output**:
```
Kernel: motion_estimation
- VALU Active:           82.1% (Excellent)
- VALU Stalled:          10.3% (Memory wait)
- LDS Busy:              15.4% (Low conflict)
- L1 Hit Rate:           94.2% (Excellent)
- L2 Hit Rate:           88.7% (Good, target: >90%)
- Occupancy:             75.0% (12/16 waves) ✅
```

---

### Memory Chart Analysis

```bash
# Visualize memory hierarchy usage
rocprof-compute analyze -p workloads/encoder_sol/gfx1030/ --mem-chart
```

**Output** (text-based chart):
```
Memory Hierarchy Bandwidth (GB/s):
Register File:  [████████████████████] 2048.0  (87% utilized)
LDS:            [████████            ] 512.0   (42% utilized)
L1 Cache:       [███████████         ] 256.0   (58% utilized)
L2 Cache:       [██████              ] 128.0   (31% utilized)
HBM (DDR5):     [██                  ] 38.4    (19% of 204.8 peak)
```

**Interpretation**:
- Register file high (>80%): Good (data cached in VGPRs)
- LDS medium (30-50%): Expected for ME kernel (64x64 window)
- L2 low (<40%): Good (not memory-bound)
- HBM low (<30%): Excellent (coalesced access, good L2 hit rate)

---

### Roofline Analysis

```bash
# Generate roofline model
rocprof-compute profile -n encoder_roof --roof-only -- ./encoder --frames 10

# Plot roofline
rocprof-compute analyze -p workloads/encoder_roof/gfx1030/ --roof --output roofline.png
```

**Roofline Interpretation**:
- **Above memory roof**: Compute-bound (good for video encoding)
- **Below memory roof**: Memory-bound (optimize coalescing, L2 hit rate)
- **Arithmetic intensity**: FLOP/byte ratio (ME: low ~0.5, Transform: high ~5)

**Target**:
- Motion estimation: Close to memory roof (memory-intensive)
- Transform/Quantization: Above memory roof (compute-intensive)

---

## RGP (Radeon GPU Profiler): Visual Analysis

### Generate RGP Trace

```bash
# System trace with roctx markers
rocprof --sys-trace --roctx-trace ./encoder --frames 10

# Output: encoder.rpd (load in RGP GUI)
```

### RGP GUI Analysis Workflow

#### 1. **Overview** (Summary)
- Total GPU time: Target >90% busy
- Wavefront occupancy: Target >75% (12/16 waves)
- Kernel count: Verify expected kernel launches

#### 2. **Events → Wavefront Occupancy**
- Timeline view: Visualize occupancy over time
- Look for dips <50%: Indicates low occupancy (VGPR issue)

#### 3. **Pipeline → Instruction Timing**
- VALU Busy %: Target >80%
- SALU Busy %: Should be low (<20%)
- LDS Busy %: ME kernel ~15%, Transform ~30%

#### 4. **Memory → L2 Cache**
- Hit Rate: Target >90%
- Miss Rate: <10%
- Visualize hot cache lines (identify reference frame locality)

#### 5. **LDS Bank Conflicts** (RDNA3+ feature)
- Heatmap view: Shows conflict hotspots
- Target: <5% conflicts (green zones)
- Red zones: Fix with XOR-swizzle

#### 6. **Barrier Analysis**
- `__syncthreads()` overhead: Should be <5% of kernel time
- Frequent barriers: Reduce or merge barrier regions

---

### RGP Command-Line (rocprof-sys)

```bash
# Export RGP data to JSON for scripting
rocprof-sys --output encoder.json ./encoder --frames 10

# Query specific metrics
jq '.kernels[] | select(.name=="motion_estimation") | {occupancy, valu_busy}' encoder.json
```

---

## Profiling Motion Estimation Kernel

### Metrics Template (`me_metrics.txt`)
```
# Pass 1: Compute
pmc: Wavefronts VALUInsts VALUUtilization VALUBusy

# Pass 2: LDS
pmc: LDSInsts LDSBankConflict

# Pass 3: Memory
pmc: FetchSize WriteSize TCC_HIT_sum TCC_MISS_sum

# Pass 4: Stalls
pmc: MemUnitStalled MemUnitBusy
```

### Expected Profile (Optimized ME Kernel)
```
Kernel: motion_estimation (16×16 block, 64×64 search window)
- Wavefronts: 8100 (1920×1080 / 256 blocks)
- VALUUtilization: 75-85% (memory-bound, expected)
- LDSBankConflict: <5% (XOR-swizzle applied)
- L2 Hit Rate: >90% (reference frame cached)
- Occupancy: 12/16 waves (75%)
```

### Commands
```bash
# Profile ME kernel only
rocprof -i me_metrics.txt --kernel-names motion_estimation -o me_profile.csv ./encoder

# Visual analysis
rocprof --sys-trace --roctx-trace --kernel-names motion_estimation ./encoder --frames 10
# Open in RGP, focus on motion_estimation events
```

---

## Profiling Transform Kernel

### Metrics Template (`transform_metrics.txt`)
```
# Pass 1: Compute (high VALU utilization expected)
pmc: Wavefronts VALUInsts VALUUtilization VALUBusy

# Pass 2: LDS (DCT coefficients cached)
pmc: LDSInsts LDSBankConflict

# Pass 3: Memory (low memory traffic expected)
pmc: FetchSize WriteSize L2CacheHit
```

### Expected Profile (Optimized Transform Kernel)
```
Kernel: transform_kernel (16×16 DCT)
- Wavefronts: 8100 (1920×1080 / 256 blocks)
- VALUUtilization: >90% (compute-bound, wave64 mode)
- LDSBankConflict: <3% (row/column access optimized)
- Memory traffic: Low (input/output only, no repeated loads)
- Occupancy: 16/16 waves (100%, wave64 mode)
```

---

## Profiling Multi-Stream Pipeline

### Tracing Stream Overlap

```bash
# Trace all streams with API calls
rocprof --hip-trace --timestamp on -o pipeline_trace.csv ./encoder --frames 40
```

### Analyze Overlap (Python Script)

```python
import pandas as pd
import matplotlib.pyplot as plt

df = pd.read_csv('pipeline_trace.csv')

# Filter kernel events
kernels = df[df['Name'].str.contains('motion_estimation|transform|quantize')]

# Plot timeline
fig, ax = plt.subplots(figsize=(12, 6))
for stream in kernels['Queue'].unique():
    stream_df = kernels[kernels['Queue'] == stream]
    ax.barh(stream, stream_df['Dur(ns)']/1e6, left=stream_df['Start(ns)']/1e6)

ax.set_xlabel('Time (ms)')
ax.set_ylabel('Stream ID')
ax.set_title('Multi-Stream Pipeline Overlap')
plt.savefig('pipeline_overlap.png')
```

**Target**: >90% overlap (minimal gaps between kernels across streams)

---

## Common Profiling Patterns

### Pattern 1: Identify Bottleneck Kernel

```bash
# Profile all kernels, sort by total time
rocprof --timestamp on --basenames on -o timing.csv ./encoder --frames 100

# Analyze (bash)
awk -F, 'NR>1 {time[$1]+=$5} END {for (k in time) print k, time[k]}' timing.csv | sort -k2 -rn
```

**Output**:
```
motion_estimation   12500000 ns  (50% of total)
transform_kernel     5000000 ns  (20% of total)
quantize_kernel      3000000 ns  (12% of total)
entropy_coding       4500000 ns  (18% of total)
```

**Action**: Optimize `motion_estimation` first (50% of total time, highest impact)

---

### Pattern 2: Validate LDS Bank Conflict Fix

```bash
# Before XOR-swizzle
rocprof -i memory_metrics.txt -o before.csv ./encoder --frames 100

# Apply XOR-swizzle in kernel code

# After XOR-swizzle
rocprof -i memory_metrics.txt -o after.csv ./encoder --frames 100

# Compare LDS conflicts
awk -F, 'NR>1 {sum+=$2; cnt++} END {print sum/cnt}' before.csv
awk -F, 'NR>1 {sum+=$2; cnt++} END {print sum/cnt}' after.csv
```

**Expected**:
- Before: 15-25% LDS conflict ratio
- After: <5% LDS conflict ratio (3-5× speedup)

---

### Pattern 3: Occupancy vs VGPR Trade-off

```bash
# Profile occupancy
rocprof -i occupancy_metrics.txt -o occupancy.csv ./encoder

# Check VGPR usage in compilation
hipcc -c motion_estimation.hip -Rpass-analysis=kernel-resource-usage | grep VGPRs

# Experiment: Reduce unroll factor
sed -i 's/#pragma unroll 4/#pragma unroll 2/' motion_estimation.hip
hipcc -c motion_estimation.hip -Rpass-analysis=kernel-resource-usage | grep VGPRs

# Re-profile
rocprof -i occupancy_metrics.txt -o occupancy_reduced.csv ./encoder
```

**Optimization**: Balance VGPR usage vs occupancy (sweet spot: 12-16 waves)

---

### Pattern 4: Thermal Throttling Detection

```bash
# Monitor GPU temp while profiling
watch -n 1 rocm-smi -t &

# Long-running profile
rocprof -i all_metrics.txt -o thermal_profile.csv ./encoder --frames 1000

# Analyze GPU busy % over time
awk -F, 'NR>1 {print NR-1, $(NF)}' thermal_profile.csv > gpu_busy_time.txt

# Plot (gnuplot)
gnuplot -e "plot 'gpu_busy_time.txt' with lines title 'GPU Busy %'"
```

**Detection**: GPU busy % drops after 2-3 minutes → thermal throttling (>85°C)

---

## Integration with Rust Benchmarks

### Criterion Integration

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::process::Command;

fn bench_with_profiling(c: &mut Criterion) {
    let mut group = c.benchmark_group("hip_video_encoder");

    group.bench_function("motion_estimation", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            // Run encoder with profiling
            Command::new("rocprof")
                .args(&["-i", "me_metrics.txt", "-o", "me_bench.csv"])
                .args(&["./encoder", "--frames", &iters.to_string()])
                .output()
                .expect("Failed to run encoder");

            start.elapsed()
        });
    });

    group.finish();

    // Parse rocprof output for metrics
    let output = std::fs::read_to_string("me_bench.csv").unwrap();
    println!("\nProfiling Results:\n{}", output);
}

criterion_group!(benches, bench_with_profiling);
criterion_main!(benches);
```

### Remote Execution (kindly-hub)

```bash
# SSH to remote, run benchmark with profiling
ssh samuel@kindly-hub "cd ~/Primitives/atomic_capsule && \
    rocprof -i me_metrics.txt -o me_remote.csv \
    cargo bench --bench hip_video_bench -- --warm-up-time 10"

# Fetch results
scp samuel@kindly-hub:~/Primitives/atomic_capsule/me_remote.csv ./
```

---

## Target Metrics Summary (Quick Reference)

| Metric | Target | Excellent | Critical |
|--------|--------|-----------|----------|
| **Occupancy** | >75% (12/16) | >90% (15/16) | <50% (8/16) |
| **VALU Utilization** | >80% | >90% | <60% |
| **LDS Bank Conflict** | <5% | <1% | >10% |
| **L2 Hit Rate** | >90% | >95% | <80% |
| **GPU Busy** | >85% | >95% | <70% |
| **Pipeline Overlap** | >90% | >95% | <70% |
| **Memory Stall** | <15% | <10% | >25% |
| **VGPR Usage** | <200 | <150 | >240 |

---

## Profiling Checklist

### Before Optimization
- [ ] **Baseline profile**: Capture all metrics before changes
- [ ] **Identify bottleneck**: Which kernel consumes most time?
- [ ] **Check occupancy**: Is it <50%? (VGPR issue)
- [ ] **Check LDS conflicts**: Is it >10%? (Layout issue)
- [ ] **Check L2 hit rate**: Is it <80%? (Tiling issue)

### After Optimization
- [ ] **Re-profile**: Same metrics, same conditions
- [ ] **Compare**: Occupancy increased? Conflicts decreased?
- [ ] **Validate speedup**: FPS improvement matches metric improvement
- [ ] **Document**: Record metric changes in git commit

### Production Validation
- [ ] **Long-running profile**: 1000+ frames to detect thermal throttling
- [ ] **Multi-run consistency**: 10 runs, CoV <5% on key metrics
- [ ] **Remote execution**: Profile on kindly-hub for reproducibility
- [ ] **Archival**: Save `.csv` and `.rpd` files for future comparison

---

## Troubleshooting

### Issue: rocprof command not found
```bash
# Add ROCm to PATH
export PATH=/opt/rocm/bin:$PATH
export LD_LIBRARY_PATH=/opt/rocm/lib:$LD_LIBRARY_PATH

# Verify
which rocprof  # Should be /opt/rocm/bin/rocprof
```

### Issue: "No kernel found" in rocprof output
**Cause**: Kernel name mismatch or kernel not launched

**Fix**:
```bash
# Don't filter by kernel name initially
rocprof --timestamp on -o all_kernels.csv ./encoder

# Check kernel names in output
awk -F, 'NR>1 {print $1}' all_kernels.csv | sort -u

# Use exact kernel name
rocprof --kernel-names "motion_estimation_kernel" -o me.csv ./encoder
```

### Issue: "Resource limit exceeded" in multi-pass profiling
**Cause**: Too many metrics in single `pmc:` line

**Fix**: Split into multiple passes (max 4-6 counters per pass)
```
# Bad (too many)
pmc: Wavefronts VALUInsts SALUInsts LDSInsts FetchSize WriteSize TCC_HIT TCC_MISS

# Good (split into 2 passes)
pmc: Wavefronts VALUInsts SALUInsts LDSInsts
pmc: FetchSize WriteSize TCC_HIT_sum TCC_MISS_sum
```

### Issue: RGP file too large (>1GB)
**Cause**: Too many frames profiled

**Fix**: Reduce frame count
```bash
# Limit to 10 frames
rocprof --sys-trace --roctx-trace ./encoder --frames 10
```

---

**Last Updated**: 2025-12-01
**Next Review**: After Phase 2 (Occupancy Optimization)
**Related**: See `ROCM_OPTIMIZATION_CHECKLIST.md` for full workflow
