# ROCm/HIP Video Encoding Optimization Checklist

**Target**: AMD Ryzen 9 6900HX RDNA2 iGPU (Radeon 680M)
**Date**: 2025-12-01

---

## Pre-Implementation Checklist

### Hardware Verification
- [ ] **Confirm RDNA2 architecture**: `rocminfo | grep "Name:" | grep gfx1030`
- [ ] **Check memory**: `free -h` (minimum 16GB, recommended 32GB+)
- [ ] **Verify ROCm version**: `rocm-smi --version` (6.0+ required)
- [ ] **Thermal baseline**: `rocm-smi -t` (idle <60°C, load <85°C target)
- [ ] **GPU availability**: `rocminfo | grep "Compute Unit:"` (expect 12 CUs)

### Software Dependencies
- [ ] **ROCm 6.0+**: Full stack (runtime, compiler, profiler)
- [ ] **hipcc compiler**: `which hipcc` (should be `/opt/rocm/bin/hipcc`)
- [ ] **rocprof**: `which rocprof` (profiling tool)
- [ ] **rocprof-compute**: `which rocprof-compute` (advanced profiling)
- [ ] **RGP**: Download from [GPUOpen](https://gpuopen.com/radeon-gpu-profiler/) (GUI profiler)

### Project Structure
```
atomic_capsule/src/encoder/
├── hip_kernels/
│   ├── motion_estimation.hip    # ME kernel (wave32)
│   ├── transform.hip             # DCT/DST (wave64)
│   ├── quantization.hip          # Q16.16 fixed-point
│   └── entropy_coding.hip        # CABAC/range coder
├── host/
│   ├── pipeline.rs               # 4-stream async orchestration
│   ├── frame_queue.rs            # FrameQueueCapsule (T1)
│   └── profiling.rs              # rocprof integration
└── tests/
    ├── unit_tests.rs             # Kernel correctness
    ├── bench.rs                  # B32 benchmarks
    └── determinism.rs            # T28 Q29-Q35 tests
```

---

## Memory Optimization Checklist

### Global Memory Coalescing
- [ ] **128-byte alignment**: All `hipMalloc` buffers aligned (verify with `hipGetDeviceProperties`)
- [ ] **Access pattern**: Consecutive threads → consecutive addresses
  ```cpp
  // ✅ Good: Coalesced
  float pixel = frame[blockIdx.x * 16 + threadIdx.x];

  // ❌ Bad: Strided (32× bandwidth waste)
  float pixel = frame[threadIdx.x * stride];
  ```
- [ ] **Padding**: Round row widths to multiples of 32 (wavefront size)
  ```cpp
  int padded_width = ((width + 31) / 32) * 32;
  hipMalloc(&d_frame, padded_width * height * sizeof(float));
  ```

### LDS (Local Data Share) Optimization
- [ ] **Size budget**: Motion estimation <64KB (current: 4.5KB ✅)
- [ ] **XOR-swizzle for bank conflicts**:
  ```cpp
  __shared__ float lds_ref[64][64];  // 4KB

  // Swizzled index to avoid bank conflicts
  int swizzled_x = x ^ (y / 32);
  lds_ref[y][swizzled_x] = ref_block[y * 64 + x];
  ```
- [ ] **Profile bank conflicts**: `rocprof -i metrics.txt` (target: `LDSBankConflict < 5%`)
- [ ] **Avoid stride-32**: Never access `lds[i * 32]` (causes 32-way conflict)

### Pinned Memory (Critical for Async)
- [ ] **Allocate with hipHostMalloc**:
  ```cpp
  float *h_frame_pinned;
  hipHostMalloc(&h_frame_pinned, 1920 * 1080 * 3 / 2);  // YUV420
  ```
- [ ] **Never use malloc**: Regular malloc forces synchronous transfers
- [ ] **Verify async capability**: Check `hipDeviceProp_t::asyncEngineCount > 0`

### L2 Cache Utilization
- [ ] **Block tiling**: Process 16×16 blocks (256 bytes fits in L2 cache line)
- [ ] **Reference frame locality**: Keep 3 reference frames in GPU memory (9.3 MB total)
- [ ] **Profile hit rate**: `TCC_HIT_sum / (TCC_HIT_sum + TCC_MISS_sum) > 0.90`

---

## Compute Optimization Checklist

### Occupancy Tuning
- [ ] **Target**: >75% occupancy (12+/16 wavefronts per SIMD)
- [ ] **Check VGPR usage**: `hipcc -c kernel.hip -Rpass-analysis=kernel-resource-usage`
  ```
  Expected output:
  VGPRs: 75 (29% of 256) ✅
  SGPRs: 24
  Occupancy: 16 waves / 16 slots (100%) ✅
  ```
- [ ] **Reduce register pressure**:
  - Limit `#pragma unroll` factor
  - Stream data from LDS instead of caching in VGPRs
  - Use `__restrict__` on pointer parameters
- [ ] **Increase work per thread**: If occupancy still low, merge adjacent blocks

### Wavefront Size Selection
- [ ] **Motion estimation**: Compile for **wave32** (better occupancy)
  ```bash
  hipcc -mwavefrontsize64=false motion_estimation.hip -o me_kernel.o
  ```
- [ ] **Transform/Quantization**: Compile for **wave64** (2× VALU throughput)
  ```bash
  hipcc -mwavefrontsize64=true transform.hip -o transform_kernel.o
  ```
- [ ] **Verify**: `rocprof --stats ./encoder | grep "Wave Size"`

### VALU Utilization
- [ ] **Target**: >80% utilization (minimize control flow divergence)
- [ ] **Avoid warp divergence**:
  ```cpp
  // ❌ Bad: Divergent branches
  if (threadIdx.x < 8) {
      // Half the wavefront idle
  }

  // ✅ Good: Uniform control flow
  if (blockIdx.x < grid_threshold) {
      // Entire wavefront executes
  }
  ```
- [ ] **Profile**: `rocprof -i metrics.txt` → check `VALUUtilization`

### SIMD Intrinsics
- [ ] **SAD (motion estimation)**: Use `__builtin_amdgcn_sad_u8x4`
  ```cpp
  unsigned sad = __builtin_amdgcn_sad_u8x4(curr_pixels, ref_pixels, 0);
  ```
- [ ] **FMA (transform)**: Use `__builtin_amdgcn_fma_f32`
- [ ] **Dot product**: Use `__builtin_amdgcn_fdot2`

---

## Synchronization Optimization Checklist

### Stream-Based Pipelining
- [ ] **Create 4 streams**:
  ```cpp
  hipStream_t streams[4];
  for (int i = 0; i < 4; i++) {
      hipStreamCreate(&streams[i]);
  }
  ```
- [ ] **Pipeline pattern**: H2D → ME → Transform → Quantize → D2H (all async)
  ```cpp
  hipMemcpyAsync(d_frame[s], h_frame[s], size, H2D, streams[s]);
  motion_estimation<<<grid, block, 0, streams[s]>>>(d_frame[s]);
  transform_kernel<<<grid, block, 0, streams[s]>>>(d_transform[s]);
  quantize_kernel<<<grid, block, 0, streams[s]>>>(d_quantized[s]);
  hipMemcpyAsync(h_bitstream[s], d_bitstream[s], size, D2H, streams[s]);
  ```
- [ ] **Tune GPU_MAX_HW_QUEUES**: `export GPU_MAX_HW_QUEUES=4` (optimal for 4 streams)

### Event-Based Synchronization
- [ ] **Use hipEvent instead of hipStreamSynchronize**:
  ```cpp
  hipEvent_t me_done;
  hipEventCreate(&me_done);
  hipEventRecord(me_done, streams[0]);

  // Wait in different stream
  hipStreamWaitEvent(streams[1], me_done, 0);
  ```
- [ ] **Reference frame dependency**: Frame N+1 ME waits for Frame N completion

### Async Transfer Validation
- [ ] **Check all hipMemcpyAsync use pinned memory**:
  ```cpp
  hipError_t err = hipMemcpyAsync(...);
  if (err != hipSuccess) {
      // Likely unpinned memory (forced sync)
      printf("Error: %s\n", hipGetErrorString(err));
  }
  ```
- [ ] **Profile transfer time**: Should be <5ms for 3.1 MB YUV420 frame

---

## Profiling and Validation Checklist

### rocprof Hardware Counters
- [ ] **Create metrics.txt**:
  ```
  pmc: Wavefronts VALUInsts VALUUtilization VALUBusy
  pmc: LDSInsts LDSBankConflict MemUnitStalled
  pmc: TCC_HIT_sum TCC_MISS_sum
  pmc: GPUBusy FetchSize WriteSize
  ```
- [ ] **Run profiling**: `rocprof -i metrics.txt -o profile.csv ./encoder --frames 100`
- [ ] **Validate targets**:
  - VALUUtilization: >80% ✅
  - LDSBankConflict: <5% ✅
  - L2 hit rate: (TCC_HIT / (TCC_HIT + TCC_MISS)) >90% ✅
  - GPUBusy: >85% (high GPU utilization) ✅

### rocprof-compute Advanced Analysis
- [ ] **Profile with roofline**:
  ```bash
  rocprof-compute profile -n me_kernel --roof-only ./encoder
  ```
- [ ] **Analyze occupancy**:
  ```bash
  rocprof-compute analyze -p workloads/me_kernel/gfx1030/ --sol
  ```
- [ ] **Memory chart**: Visualize L1/L2/HBM bandwidth usage

### RGP Visual Profiling
- [ ] **Generate trace**: `rocprof --sys-trace --roctx-trace ./encoder`
- [ ] **Open in RGP GUI**: Load `.rpd` file
- [ ] **Inspect**:
  - Wavefront occupancy (timeline view)
  - LDS bank conflict heatmap
  - Memory chart (L2 hit rate visualization)

### Thermal Monitoring
- [ ] **Before**: `rocm-smi -t` (baseline temp)
- [ ] **During**: `watch -n 1 rocm-smi -t` (monitor real-time)
- [ ] **Target**: <85°C (prevent throttling)
- [ ] **Mitigation**: Active cooling if sustained encode >80°C

---

## Testing Checklist (T28 Compliance)

### Q1-Q7: Unit Tests (Kernel Correctness)
- [ ] **SAD accuracy**: HIP vs reference C++ (max error: 0)
  ```rust
  #[test]
  fn test_sad_accuracy() {
      let hip_sad = hip_compute_sad(&curr, &ref);
      let cpu_sad = cpu_compute_sad(&curr, &ref);
      assert_eq!(hip_sad, cpu_sad);  // Bit-exact match
  }
  ```
- [ ] **DCT orthogonality**: `DCT(IDCT(x)) == x` (error <1e-4)
- [ ] **Quantization**: Q16.16 vs FP32 (error <1 LSB)
- [ ] **LDS bank conflicts**: Synthetic test (measure <5%)

### Q8-Q14: Property Tests
- [ ] **Memory alignment**: All `hipMalloc` returns 256-byte aligned
- [ ] **Stream independence**: Parallel streams produce identical output vs sequential
- [ ] **Occupancy invariant**: All kernels ≥50% occupancy (8/16 waves)
- [ ] **VGPR constraint**: No kernel >256 VGPRs (no spilling)

### Q15-Q21: Integration Tests
- [ ] **Multi-frame encode**: 100 frames, verify bitstream continuity
- [ ] **Stream synchronization**: All `hipStreamSynchronize` succeed
- [ ] **Memory leaks**: Run with `rocm-gdb`, check `hipDeviceReset` for leaks
- [ ] **Thermal stability**: 10-minute encode, temp <85°C (no throttle)

### Q22-Q28: Performance Benchmarks (B32)
- [ ] **Baseline**: CPU-only rav1e at speed 6 (1-5 FPS)
- [ ] **Optimized**: HIP compute shaders (target: 15-30 FPS)
- [ ] **95% CI**: 1000+ frames, report confidence interval
- [ ] **Hardware consistency**: Disable turbo boost, remote execution on kindly-hub
  ```bash
  ssh samuel@kindly-hub "cd ~/Primitives/atomic_capsule && cargo bench --bench hip_video_bench"
  ```

### Q29-Q35: Determinism Tests
- [ ] **Bit-identical output**: 10 runs, SHA-256 hash match
  ```bash
  for i in {1..10}; do
      ./encoder --input test.yuv --output test_$i.ivf --seed 42
      sha256sum test_$i.ivf >> hashes.txt
  done
  sort -u hashes.txt | wc -l  # Should be 1
  ```
- [ ] **Sources of non-determinism**:
  - FP32 rounding: Use deterministic quantization
  - Atomic race: Replace atomic adds with reductions
  - Multi-stream race: Synchronize frame dependencies

---

## Performance Validation Checklist

### Target Metrics (1080p30 AV1)
- [ ] **FPS**: 15-30 (10-30× vs CPU baseline)
- [ ] **Occupancy**: >75% (12/16 wavefronts)
- [ ] **VALU Utilization**: >80%
- [ ] **LDS Bank Conflicts**: <5%
- [ ] **L2 Hit Rate**: >90%
- [ ] **Pipeline Efficiency**: >90% (ratio of compute time to total time)

### B32 Validation Requirements
- [ ] **Same hardware**: kindly-hub (AMD Ryzen 9 6900HX)
- [ ] **Baseline**: Optimized baseline (not strawman CPU implementation)
- [ ] **Iterations**: 1000+ frames for 95% CI
- [ ] **Reproducibility**: 10 runs, CoV <5%
- [ ] **Documentation**: Record all hardware settings, driver versions

### Exceptional Claims (30-50× speedup)
Requires ALL of:
- [ ] Occupancy: 15/16 wavefronts (93%+)
- [ ] LDS conflicts: <1% (XOR-swizzle perfect)
- [ ] L2 hit rate: >95%
- [ ] No thermal throttling: Active cooling, GPU <80°C
- [ ] Wave64 mode: 2× VALU throughput for transforms
- [ ] Async overlap: >95% pipeline efficiency

---

## Chaos Compliance Checklist

### Lockfree Mandate
- [ ] **No mutex/RwLock**: All coordination via atomics
- [ ] **DualAtomicU64**: Frame queue head/tail in single 64-byte cache line
  ```rust
  #[repr(C, align(64))]
  struct FrameQueueCapsule {
      head_tail: DualAtomicU64,  // (u32, u32) packed
      generation: AtomicU64,
      frames: [*mut u8; 16],
      _padding: [u8; 64 - 24],
  }
  ```
- [ ] **SWeMR pattern**: Single-writer (encoder), multiple-reader (ME kernels)

### Cache Alignment
- [ ] **64-byte**: Host-side capsules (CPU cache line)
- [ ] **128-byte**: GPU buffer alignment (coalescing requirement)
- [ ] **Padding**: Explicit `_padding` field to prevent false sharing

### Generation Counters (ABA Prevention)
- [ ] **Frame queue**: `generation: AtomicU64` incremented on push/pop
- [ ] **Reference frames**: Track generation to detect stale references
- [ ] **Audit trail**: Link frame ID to generation for Q34 integrity

### Verification (#[derive(ComputationalCapsule)])
- [ ] **Automatic**: Apply `#[derive(ComputationalCapsule)]` to all host capsules
- [ ] **Manual check**: Size ≤1024 bytes, alignment 64/128 bytes
- [ ] **Clippy**: Run `cargo clippy -- -D clippy::capsule_mutex_violation`

---

## Auditability Checklist (Q34)

### Hash-Chain Audit Trail
- [ ] **Entry structure**:
  ```rust
  #[repr(C, align(64))]
  struct EncodingAuditEntry {
      prev_hash: [u8; 32],      // SHA-256 of previous entry
      frame_id: u64,
      timestamp_ns: u64,
      qp: u8,
      encoding_time_us: u32,
      occupancy_percent: u8,
      lds_conflicts_percent: u8,
      // ...
  }
  ```
- [ ] **Hash computation**: SHA-256 over entire entry (except `prev_hash`)
- [ ] **Chain verification**: `entry[i].prev_hash == hash(entry[i-1])`
- [ ] **Tamper detection**: Any modification breaks chain

### Compliance Mapping
- [ ] **SOX**: Audit trail with cryptographic integrity ✅
- [ ] **SOC2**: Performance metrics logged per frame ✅
- [ ] **GDPR**: No PII in audit entries ✅
- [ ] **HIPAA**: Not applicable (video encoding, not healthcare) N/A

---

## Critical Path Optimization Priority

### Phase 1: Foundation (Week 1)
1. **Memory setup**: Pinned buffers, 128-byte alignment
2. **Basic kernels**: ME (naive), transform (naive)
3. **Single stream**: Sequential H2D → Compute → D2H
4. **Validation**: Correctness tests, decoder compliance

### Phase 2: Occupancy (Week 2)
1. **Profile baseline**: rocprof metrics (expect <50% occupancy)
2. **VGPR optimization**: Reduce register pressure (target: <150 VGPRs)
3. **Workgroup tuning**: Increase threads per block
4. **Validate**: Occupancy >75%, benchmark FPS

### Phase 3: Memory (Week 3)
1. **LDS XOR-swizzle**: Implement bank conflict avoidance
2. **Coalesced access**: Verify 128-byte aligned loads
3. **L2 tiling**: Block processing for cache locality
4. **Profile**: LDS conflicts <5%, L2 hit rate >90%

### Phase 4: Pipelining (Week 4)
1. **4-stream setup**: Async H2D/compute/D2H overlap
2. **Event sync**: Reference frame dependencies
3. **GPU_MAX_HW_QUEUES tuning**: Optimal stream count
4. **Validate**: Pipeline efficiency >90%, FPS 15-30×

### Phase 5: Polish (Week 5)
1. **Thermal optimization**: Active cooling, burst encoding
2. **Determinism**: Fix non-deterministic sources
3. **B32 benchmarking**: 1000+ frames, 95% CI
4. **Documentation**: Trade secret marking, UCE34 compliance report

---

## Common Pitfalls and Solutions

### Pitfall 1: Unpinned Memory → Synchronous Transfer
**Symptom**: `hipMemcpyAsync` is slow, no overlap visible in profiler
**Solution**: Use `hipHostMalloc` instead of `malloc`
```cpp
// ❌ Bad
float *h_frame = (float*)malloc(size);

// ✅ Good
float *h_frame;
hipHostMalloc(&h_frame, size);
```

### Pitfall 2: LDS Bank Conflicts → 2-5× Slowdown
**Symptom**: `LDSBankConflict > 10%` in rocprof output
**Solution**: XOR-swizzle LDS indices
```cpp
// ❌ Bad: Stride-32 causes conflicts
lds[y * 64 + x]

// ✅ Good: XOR-swizzled
lds[(y ^ (x / 32)) * 64 + x]
```

### Pitfall 3: Low Occupancy → Poor Latency Hiding
**Symptom**: `Occupancy < 50%` (8/16 wavefronts), high `MemUnitStalled`
**Solution**: Reduce VGPR usage, increase threads per block
```cpp
// Check VGPR usage
hipcc -c kernel.hip -Rpass-analysis=kernel-resource-usage
// If VGPRs > 180, reduce #pragma unroll factor
```

### Pitfall 4: Thermal Throttling → 25% Performance Loss
**Symptom**: FPS drops after 2-3 minutes, `rocm-smi -t` shows >85°C
**Solution**: Active cooling or burst encoding pattern
```rust
// Burst pattern: Encode 5 frames, sleep 1s
for chunk in frames.chunks(5) {
    encode_chunk(chunk);
    std::thread::sleep(Duration::from_secs(1));
}
```

### Pitfall 5: Non-Deterministic Output
**Symptom**: Same input produces different SHA-256 hashes across runs
**Solution**: Fix FP32 rounding, atomic races, thread scheduling
```cpp
// Use deterministic quantization
int16_t quant = (int16_t)floorf(coeff * quant_scale + 0.5f);  // Round ties to even

// Replace atomic adds with reductions
__syncthreads();
float sum = block_reduce_sum(local_value);
if (threadIdx.x == 0) {
    histogram[bin] = sum;  // No atomics
}
```

---

## Quick Reference Commands

```bash
# Verify RDNA2 architecture
rocminfo | grep -A5 "Name:.*gfx"

# Compile ME kernel for wave32
hipcc -mwavefrontsize64=false -O3 --offload-arch=gfx1030 \
      motion_estimation.hip -o me_kernel.o

# Compile transform for wave64
hipcc -mwavefrontsize64=true -O3 --offload-arch=gfx1030 \
      transform.hip -o transform_kernel.o

# Profile with hardware counters
rocprof -i metrics.txt -o profile.csv ./encoder --frames 100

# Analyze occupancy
rocprof-compute profile -n encoder --roof-only ./encoder
rocprof-compute analyze -p workloads/encoder/gfx1030/ --sol

# Visual profiling (RGP)
rocprof --sys-trace --roctx-trace ./encoder --frames 10

# Benchmark (remote execution)
ssh samuel@kindly-hub "cd ~/Primitives/atomic_capsule && \
    cargo bench --bench hip_video_bench -- --warm-up-time 10"

# Monitor thermal
watch -n 1 rocm-smi -t

# Determinism test
for i in {1..10}; do
    ./encoder --input test.yuv --output test_$i.ivf --seed 42
    sha256sum test_$i.ivf >> hashes.txt
done
sort -u hashes.txt | wc -l  # Expect: 1
```

---

## Success Criteria (Exit Checklist)

### Minimum Viable (MVP)
- [ ] Encoder produces valid AV1 bitstream (dav1d decodes without errors)
- [ ] FPS: 10-15 (5-10× vs CPU baseline)
- [ ] Occupancy: >50% (8/16 wavefronts)
- [ ] LDS conflicts: <10%
- [ ] All T28 tests pass

### Production Ready
- [ ] FPS: 15-25 (10-20× vs CPU baseline)
- [ ] Occupancy: >75% (12/16 wavefronts)
- [ ] LDS conflicts: <5%
- [ ] L2 hit rate: >90%
- [ ] Pipeline efficiency: >85%
- [ ] Thermal stable: <85°C for 10-minute encode
- [ ] Deterministic: 10 runs, bit-identical output

### Exceptional (Stretch Goal)
- [ ] FPS: 25-35 (20-30× vs CPU baseline)
- [ ] Occupancy: >90% (15/16 wavefronts)
- [ ] LDS conflicts: <1% (XOR-swizzle perfect)
- [ ] L2 hit rate: >95%
- [ ] Pipeline efficiency: >95%
- [ ] No thermal throttling: Active cooling, GPU <80°C
- [ ] Wave64 transforms: 2× VALU throughput validated

---

**Last Updated**: 2025-12-01
**Next Review**: After Phase 2 (Occupancy Optimization)
