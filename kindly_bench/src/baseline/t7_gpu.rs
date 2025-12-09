//! T7 Heterogeneous (GPU) baseline generation
//!
//! # Baseline Strategy
//!
//! **Optimized**: GPU kernel execution (CUDA, Vulkan, compute shaders)
//! **Baseline**: CPU-only version (manual implementation required)
//!
//! # Why Manual?
//!
//! GPU kernels are highly specialized:
//! - Parallel algorithms (thread blocks, warps)
//! - Memory hierarchies (shared memory, registers)
//! - Hardware-specific optimizations
//!
//! Automatic CPU baseline would be strawman (naive single-threaded loop).
//! Fair baseline requires well-optimized CPU library (OpenBLAS, MKL, etc.).
//!
//! # Manual Baseline Guide
//!
//! ## Step 1: Identify GPU Kernel Operations
//!
//! ```cuda
//! __global__ void vec_add(float* a, float* b, float* c, int n) {
//!     int i = blockIdx.x * blockDim.x + threadIdx.x;
//!     if (i < n) c[i] = a[i] + b[i];
//! }
//! ```
//!
//! ## Step 2: Write Equivalent CPU Code (Optimized!)
//!
//! ```rust
//! fn vec_add_cpu(a: &[f32], b: &[f32]) -> Vec<f32> {
//!     // GOOD: SIMD-optimized CPU code
//!     a.iter().zip(b).map(|(x, y)| x + y).collect()
//!
//!     // BAD (strawman): Naive loop
//!     // let mut c = vec![0.0; a.len()];
//!     // for i in 0..a.len() { c[i] = a[i] + b[i]; }
//! }
//! ```
//!
//! ## Step 3: Use Well-Optimized Libraries
//!
//! - **Matrix multiplication**: Use OpenBLAS, Intel MKL, Eigen (NOT naive O(n³))
//! - **Convolutions**: Use optimized CPU libraries (NNPACK, OneDNN)
//! - **Reductions**: Use CPU SIMD intrinsics
//!
//! ## Step 4: Benchmark Both Implementations
//!
//! ```rust
//! use kindly_bench::*;
//!
//! let config = BenchmarkConfig::builder()
//!     .tier(Tier::T7Heterogeneous)
//!     .gpu_timer(GpuTimer::cuda())
//!     .baseline_manual(Box::new(|| vec_add_cpu(&a, &b)))
//!     .build();
//! ```
//!
//! # Expected Results
//!
//! - **EXCEPTIONAL**: 15-20× speedup (large datasets, e.g., 4096×4096 matrices)
//! - **BREAKTHROUGH**: 20-100× speedup (massive parallelism)
//! - **SUSPICIOUS**: >100× speedup (validate baseline isn't strawman)
//!
//! # Fair Baseline Checklist
//!
//! - ✓ Uses well-optimized CPU library (OpenBLAS, MKL, etc.)
//! - ✓ Same algorithm as GPU version (not naive implementation)
//! - ✓ CPU SIMD intrinsics where applicable
//! - ✓ Multi-threaded CPU code (if GPU uses massive parallelism)
//! - ✗ Naive single-threaded loop (STRAWMAN!)

use super::{BaselineGenerator, ManualBaselineFn};

/// T7 Heterogeneous baseline generator (GPU → CPU)
pub struct T7GpuBaseline;

impl<T> BaselineGenerator<T> for T7GpuBaseline {
    fn generate_baseline(&self) -> Option<ManualBaselineFn<T>> {
        // Manual baseline required - cannot auto-generate
        None
    }

    fn is_auto_generated(&self) -> bool {
        false
    }

    fn manual_guide(&self) -> &'static str {
        r#"
# T7 Heterogeneous (GPU) - Manual Baseline Guide

## Baseline Strategy
**Optimized**: GPU kernel execution
**Baseline**: CPU-only version (YOU provide this)

## How to Write Fair CPU Baseline

1. **Identify GPU kernel operations**
2. **Write equivalent CPU code** (use optimized libraries!)
3. **Benchmark both implementations**

## Example: Matrix Multiplication

```rust
// GPU kernel (optimized)
launch_cublas_gemm(a, b, c);  // cuBLAS library

// CPU baseline (YOU write this)
fn matmul_cpu(a: &[f32], b: &[f32]) -> Vec<f32> {
    // Use OpenBLAS or Intel MKL (NOT naive loops!)
    openblas::sgemm(a, b)
}

// Benchmark
let config = BenchmarkConfig::builder()
    .tier(Tier::T7Heterogeneous)
    .gpu_timer(GpuTimer::cuda())
    .baseline_manual(Box::new(|| matmul_cpu(&a, &b)))
    .build();
```

## Expected Results
- **EXCEPTIONAL**: 15-20× speedup
- **BREAKTHROUGH**: 20-100× speedup (large datasets)
- **SUSPICIOUS**: >100× (validate baseline isn't strawman)

## Fair Baseline Checklist
✓ Uses well-optimized CPU library (OpenBLAS, MKL, etc.)
✓ Same algorithm as GPU version
✓ CPU SIMD intrinsics where applicable
✓ Multi-threaded CPU code (if applicable)
✗ Naive single-threaded loop (STRAWMAN!)
        "#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_t7_gpu_baseline_is_manual() {
        let baseline = T7GpuBaseline;
        assert!(!baseline.is_auto_generated());
        assert!(baseline.generate_baseline::<()>().is_none());
    }

    #[test]
    fn test_t7_gpu_baseline_has_guide() {
        let baseline = T7GpuBaseline;
        let guide = baseline.manual_guide();
        assert!(guide.contains("GPU kernel"));
        assert!(guide.contains("CPU baseline"));
        assert!(guide.contains("OpenBLAS"));
    }
}
