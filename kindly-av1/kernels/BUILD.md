# HIP Motion Estimation Kernel - Build Guide

## Prerequisites

**ROCm 6.0+** installed on kindly-hub (192.168.0.38)

```bash
# Verify ROCm installation
rocm-smi --showversion
hipcc --version
```

Expected output:
```
ROCm version: 6.0.2
HIP version: 6.0.32831
```

## Build Commands

### 1. Simple Build (Executable)

Compiles kernel to standalone executable for testing:

```bash
hipcc -O3 --offload-arch=gfx1035 -ffast-math \
    -o motion_estimation.out \
    motion_estimation.hip
```

**Options**:
- `-O3`: Maximum optimization
- `--offload-arch=gfx1035`: Target AMD Radeon 680M (RDNA2)
- `-ffast-math`: Aggressive floating-point optimizations

### 2. Kernel Code Object (Recommended)

Compiles to relocatable GPU code for runtime loading:

```bash
hipcc --genco -O3 --amdgpu-target=gfx1035 -ffast-math \
    -o motion_estimation.co \
    motion_estimation.hip
```

**Usage in Rust**:
```rust
use hip_sys::*;

unsafe {
    let mut module = std::ptr::null_mut();
    hipModuleLoad(&mut module, c"motion_estimation.co".as_ptr());

    let mut kernel = std::ptr::null_mut();
    hipModuleGetFunction(&mut kernel, module, c"motion_estimation_sad_kernel".as_ptr());
}
```

### 3. Debug Build

Includes debug symbols for profiling:

```bash
hipcc -O0 -g --offload-arch=gfx1035 \
    -o motion_estimation_debug.out \
    motion_estimation.hip
```

### 4. Assembly Output (Analysis)

View generated GPU assembly code:

```bash
hipcc -S -O3 --offload-arch=gfx1035 -ffast-math \
    -o motion_estimation.s \
    motion_estimation.hip
```

## Remote Build (on kindly-hub)

From local machine (192.168.0.103):

```bash
# Copy source to remote
scp motion_estimation.hip samuel@kindly-hub:~/kernels/

# Build remotely
ssh samuel@kindly-hub "cd ~/kernels && \
    hipcc --genco -O3 --amdgpu-target=gfx1035 -ffast-math \
    -o motion_estimation.co motion_estimation.hip"

# Copy binary back
scp samuel@kindly-hub:~/kernels/motion_estimation.co .
```

Or use lsyncd (auto-sync):

```bash
# Verify sync is active
journalctl --user -u lsyncd -n 20

# Build on remote (auto-synced)
ssh samuel@kindly-hub "cd ~/Primitives/kindly-av1/kernels && \
    hipcc --genco -O3 --amdgpu-target=gfx1035 -ffast-math \
    -o motion_estimation.co motion_estimation.hip"
```

## Verification

### 1. Check Binary Format

```bash
file motion_estimation.co
```

Expected: `motion_estimation.co: ELF 64-bit LSB relocatable, x86-64`

### 2. List Kernel Symbols

```bash
nm motion_estimation.co | grep motion_estimation
```

Expected: `motion_estimation_sad_kernel`

### 3. ROCProf Analysis

Profile kernel performance:

```bash
rocprof --stats motion_estimation.out
```

### 4. GPU Query

Verify target GPU is available:

```bash
rocm-smi --showmeminfo
```

Expected: `gfx1035` (AMD Radeon 680M)

## Common Build Errors

### Error: `fatal error: 'hip/hip_runtime.h' file not found`

**Fix**: ROCm not installed or not in PATH

```bash
export PATH=/opt/rocm/bin:$PATH
export LD_LIBRARY_PATH=/opt/rocm/lib:$LD_LIBRARY_PATH
```

### Error: `No available targets are compatible with triple "amdgcn-amd-amdhsa"`

**Fix**: Wrong architecture specified

```bash
# List available targets
/opt/rocm/llvm/bin/llc --version | grep gfx

# Use correct target (gfx1035 for Radeon 680M)
hipcc --offload-arch=gfx1035 ...
```

### Error: `unsupported option '-ffast-math' for target`

**Fix**: ROCm version too old (need 5.0+)

```bash
rocm-smi --showversion
# Upgrade if < 5.0
```

## Performance Benchmarking

### 1. Criterion Benchmark (Rust)

```bash
cd /home/samuel/Primitives/kindly-av1
cargo bench --bench gpu_motion_estimation
```

### 2. rocBLAS Timing (C++)

```cpp
#include <hip/hip_runtime.h>
#include <chrono>

hipEvent_t start, stop;
hipEventCreate(&start);
hipEventCreate(&stop);

hipEventRecord(start);
hipLaunchKernelGGL(motion_estimation_sad_kernel, ...);
hipEventRecord(stop);

float ms = 0;
hipEventElapsedTime(&ms, start, stop);
printf("Kernel time: %.3f ms\n", ms);
```

### 3. ROCProfiler (Full Analysis)

```bash
rocprof --stats --hsa-trace \
    ./motion_estimation.out

# View results
cat results.csv
```

## Optimization Flags

| Flag | Impact | Use Case |
|------|--------|----------|
| `-O3` | Max optimization | Production builds |
| `-ffast-math` | Aggressive FP opts | GPU kernels (safe for SAD) |
| `-g` | Debug symbols | Profiling with rocprof |
| `--genco` | Code object only | Runtime loading in Rust |
| `-fno-gpu-rdc` | Disable RDC | Faster compile (single-file) |

## Integration with Rust

### FFI Bindings (hip_sys)

```rust
// Cargo.toml
[dependencies]
hip-sys = "0.3"

// src/gpu/motion_estimation.rs
use hip_sys::*;
use std::ffi::CString;

pub struct GpuMotionEstimator {
    module: hipModule_t,
    kernel: hipFunction_t,
}

impl GpuMotionEstimator {
    pub fn new() -> Result<Self, hipError_t> {
        unsafe {
            let mut module = std::ptr::null_mut();
            let path = CString::new("kernels/motion_estimation.co").unwrap();
            hipModuleLoad(&mut module, path.as_ptr())?;

            let mut kernel = std::ptr::null_mut();
            let name = CString::new("motion_estimation_sad_kernel").unwrap();
            hipModuleGetFunction(&mut kernel, module, name.as_ptr())?;

            Ok(Self { module, kernel })
        }
    }
}
```

## Cross-Compilation

### From x86_64 to gfx1035

```bash
# Native compilation (on kindly-hub)
hipcc --genco -O3 --amdgpu-target=gfx1035 motion_estimation.hip

# Cross-compilation (not recommended for HIP)
# HIP kernels must be compiled on ROCm-enabled system
```

## Continuous Integration

```yaml
# .github/workflows/hip-build.yml
name: HIP Kernel Build

on: [push, pull_request]

jobs:
  build:
    runs-on: [self-hosted, rocm]
    steps:
      - uses: actions/checkout@v3
      - name: Build HIP kernel
        run: |
          cd kernels
          hipcc --genco -O3 --amdgpu-target=gfx1035 \
            -o motion_estimation.co motion_estimation.hip
      - name: Upload artifact
        uses: actions/upload-artifact@v3
        with:
          name: hip-kernels
          path: kernels/*.co
```

## Troubleshooting

### Kernel Launch Failures

```bash
# Enable debug logging
export HIP_VISIBLE_DEVICES=0
export AMD_LOG_LEVEL=4

# Run with debug info
./motion_estimation.out 2>&1 | grep -i error
```

### Memory Errors

```bash
# Compute-sanitizer equivalent for ROCm
/opt/rocm/bin/rocminfo
```

### Performance Issues

```bash
# Check GPU clock throttling
rocm-smi --showclocks

# Set performance mode
rocm-smi --setperflevel high
```

## References

- [ROCm Documentation](https://rocm.docs.amd.com/)
- [HIP Programming Guide](https://rocm.docs.amd.com/projects/HIP/en/latest/)
- [RDNA2 Optimization Guide](https://gpuopen.com/rdna2-performance-guide/)
- [hipcc Compiler Options](https://rocm.docs.amd.com/projects/HIP/en/latest/reference/kernel_language.html)

---

**Last Updated**: 2025-11-26
**ROCm Version**: 6.0.2
**Target GPU**: AMD Radeon 680M (gfx1035)
