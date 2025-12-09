// GPU Reduction Kernels - CUDA Implementation
// State-of-the-Art Techniques (2024-2025)
//
// Based on:
// - NVIDIA Mark Harris "Optimizing Parallel Reduction in CUDA" (2007)
// - NVIDIA CUB library (2024)
// - CUDA 9+ warp-level primitives (__shfl_down_sync)
//
// UCE34 Compliance: T7 Heterogeneous Tier
// Performance Target: 10-50× vs CPU sequential (20-100× exceptional)
// Memory Bandwidth: 80-90% of theoretical peak
//
// ASSUM Framework:
// - #ASSUME_WARP_SIZE_32: NVIDIA GPUs have warpSize=32 (Kepler+)
// - #ASSUME_SHUFFLE_AVAILABLE: Compute Capability ≥ 3.0 (Kepler+)
// - #ASSUME_CUDA_9_PLUS: __shfl_down_sync available (deprecated __shfl_down)
// - #ASSUME_COALESCED_ACCESS: 128-byte cache lines (32×float32 or 16×float64)
// - #ASSUME_FLOAT_ASSOCIATIVE: FP addition is associative (epsilon error tolerated)

#include <cuda_runtime.h>
#include <stdint.h>

// =============================================================================
// WARP-LEVEL SHUFFLE REDUCTION (Tier 1: <10ns, No Shared Memory)
// =============================================================================
// Based on: https://developer.nvidia.com/blog/faster-parallel-reductions-kepler/
//
// Key Advantages:
// - No shared memory (higher occupancy)
// - Zero latency (register-to-register communication)
// - Implicit synchronization (no __syncthreads)
// - 5-10× faster than shared memory reduction
//
// ASSUM: #ASSUME_WARP_SYNCHRONIZED: All threads in mask participate

// Warp shuffle reduction: Sum
template<typename T>
__inline__ __device__ T warpReduceSum(T val) {
    #pragma unroll
    for (int offset = warpSize/2; offset > 0; offset /= 2) {
        // CUDA 9+ synchronized shuffle (mask 0xffffffff = all 32 threads)
        // Replaces deprecated __shfl_down (CUDA 8 and earlier)
        val += __shfl_down_sync(0xffffffff, val, offset);
    }
    return val; // Lane 0 holds final sum
}

// Warp shuffle reduction: Max
template<typename T>
__inline__ __device__ T warpReduceMax(T val) {
    #pragma unroll
    for (int offset = warpSize/2; offset > 0; offset /= 2) {
        T other = __shfl_down_sync(0xffffffff, val, offset);
        val = (val > other) ? val : other;
    }
    return val;
}

// Warp shuffle reduction: Min
template<typename T>
__inline__ __device__ T warpReduceMin(T val) {
    #pragma unroll
    for (int offset = warpSize/2; offset > 0; offset /= 2) {
        T other = __shfl_down_sync(0xffffffff, val, offset);
        val = (val < other) ? val : other;
    }
    return val;
}

// Warp shuffle reduction: ArgMax (pack index+value in uint64_t)
__inline__ __device__ uint64_t warpReduceArgMax(uint64_t val) {
    #pragma unroll
    for (int offset = warpSize/2; offset > 0; offset /= 2) {
        uint64_t other = __shfl_down_sync(0xffffffff, val, offset);
        // Compare values (upper 32 bits = float as uint32)
        float val_f = __uint_as_float(val >> 32);
        float other_f = __uint_as_float(other >> 32);
        if (other_f > val_f) {
            val = other; // Take index from higher value
        }
    }
    return val;
}

// Warp shuffle reduction: ArgMin
__inline__ __device__ uint64_t warpReduceArgMin(uint64_t val) {
    #pragma unroll
    for (int offset = warpSize/2; offset > 0; offset /= 2) {
        uint64_t other = __shfl_down_sync(0xffffffff, val, offset);
        float val_f = __uint_as_float(val >> 32);
        float other_f = __uint_as_float(other >> 32);
        if (other_f < val_f) {
            val = other;
        }
    }
    return val;
}

// =============================================================================
// BLOCK-LEVEL SHARED MEMORY REDUCTION (Tier 2: <100ns, 512 Threads)
// =============================================================================
// Based on: CUB library BlockReduce
//
// Algorithm:
// 1. Each warp performs warp shuffle reduction (16 warps × 32 threads = 512 threads)
// 2. First thread of each warp writes partial result to shared memory (16 values)
// 3. First warp reduces 16 partial results using shuffle
// 4. Thread 0 of first warp holds final result
//
// Optimization: Only 2 barriers needed (__syncthreads before and after shared write)
//
// ASSUM: #ASSUME_BLOCK_SIZE_512: blockDim.x = 512 (16 warps)

template<typename T>
__device__ T blockReduceSum(T val) {
    // Shared memory for warp partial sums (16 warps for 512 threads)
    __shared__ T shared[32]; // Over-allocate for safety (16 actually needed)

    int lane = threadIdx.x % warpSize;       // Lane ID within warp (0-31)
    int wid = threadIdx.x / warpSize;        // Warp ID within block (0-15)

    // Stage 1: Warp-level reduction (no shared memory)
    val = warpReduceSum(val);

    // Stage 2: Write warp results to shared memory
    if (lane == 0) {
        shared[wid] = val;
    }
    __syncthreads(); // Barrier 1: Wait for all warps to write

    // Stage 3: Final reduction by first warp
    if (wid == 0) {
        // Load partial sums (or 0 if wid >= num_warps)
        val = (threadIdx.x < blockDim.x / warpSize) ? shared[lane] : T(0);
        val = warpReduceSum(val);
    }

    return val; // Thread 0 holds final sum
}

template<typename T>
__device__ T blockReduceMax(T val) {
    __shared__ T shared[32];
    int lane = threadIdx.x % warpSize;
    int wid = threadIdx.x / warpSize;

    val = warpReduceMax(val);

    if (lane == 0) {
        shared[wid] = val;
    }
    __syncthreads();

    if (wid == 0) {
        // Initialize to -infinity for Max
        val = (threadIdx.x < blockDim.x / warpSize) ? shared[lane] : -INFINITY;
        val = warpReduceMax(val);
    }

    return val;
}

template<typename T>
__device__ T blockReduceMin(T val) {
    __shared__ T shared[32];
    int lane = threadIdx.x % warpSize;
    int wid = threadIdx.x / warpSize;

    val = warpReduceMin(val);

    if (lane == 0) {
        shared[wid] = val;
    }
    __syncthreads();

    if (wid == 0) {
        // Initialize to +infinity for Min
        val = (threadIdx.x < blockDim.x / warpSize) ? shared[lane] : INFINITY;
        val = warpReduceMin(val);
    }

    return val;
}

__device__ uint64_t blockReduceArgMax(uint64_t val) {
    __shared__ uint64_t shared[32];
    int lane = threadIdx.x % warpSize;
    int wid = threadIdx.x / warpSize;

    val = warpReduceArgMax(val);

    if (lane == 0) {
        shared[wid] = val;
    }
    __syncthreads();

    if (wid == 0) {
        // Initialize to -infinity + invalid index
        uint64_t init = ((uint64_t)__float_as_uint(-INFINITY) << 32) | 0xFFFFFFFF;
        val = (threadIdx.x < blockDim.x / warpSize) ? shared[lane] : init;
        val = warpReduceArgMax(val);
    }

    return val;
}

__device__ uint64_t blockReduceArgMin(uint64_t val) {
    __shared__ uint64_t shared[32];
    int lane = threadIdx.x % warpSize;
    int wid = threadIdx.x / warpSize;

    val = warpReduceArgMin(val);

    if (lane == 0) {
        shared[wid] = val;
    }
    __syncthreads();

    if (wid == 0) {
        uint64_t init = ((uint64_t)__float_as_uint(INFINITY) << 32) | 0xFFFFFFFF;
        val = (threadIdx.x < blockDim.x / warpSize) ? shared[lane] : init;
        val = warpReduceArgMin(val);
    }

    return val;
}

// =============================================================================
// GRID-LEVEL REDUCTION KERNELS (Tier 3: Multi-Block, Multi-Stage)
// =============================================================================
// Based on: Mark Harris "Optimizing Parallel Reduction in CUDA"
//
// Stage 1: Each block reduces N/num_blocks elements → partial sum (global memory)
// Stage 2: Recursive kernel launch on partial sums (if >512 partials)
// Stage 3: Final reduction (single block, <512 partials)
//
// Memory Pattern:
// - Coalesced reads: 128-byte cache lines (32×float32)
// - Register blocking: ItemsPerThread = 4-8 (amortize overhead)
//
// ASSUM: #ASSUME_ITEMSPERTHREAD_4: Each thread processes 4 elements

// Kernel 1: Block-level reduction with register blocking
template<typename T, int ItemsPerThread = 4>
__global__ void reduceBlockLevel(
    const T* __restrict__ input,
    T* __restrict__ output,
    int n
) {
    // Grid-stride loop with register blocking
    int tid = threadIdx.x + blockIdx.x * blockDim.x;
    int grid_size = blockDim.x * gridDim.x;

    T sum = T(0);

    // Each thread processes ItemsPerThread elements per iteration
    for (int i = tid; i < n; i += grid_size * ItemsPerThread) {
        #pragma unroll
        for (int j = 0; j < ItemsPerThread && (i + j * grid_size) < n; ++j) {
            sum += input[i + j * grid_size];
        }
    }

    // Block-level reduction
    sum = blockReduceSum(sum);

    // Thread 0 writes partial sum to output
    if (threadIdx.x == 0) {
        output[blockIdx.x] = sum;
    }
}

// Kernel 2: ArgMax with index tracking
__global__ void reduceArgMax(
    const float* __restrict__ input,
    uint32_t* __restrict__ output, // Output index
    int n
) {
    int tid = threadIdx.x + blockIdx.x * blockDim.x;
    int grid_size = blockDim.x * gridDim.x;

    // Pack value (upper 32 bits) + index (lower 32 bits)
    uint64_t max_val = ((uint64_t)__float_as_uint(-INFINITY) << 32) | 0xFFFFFFFF;

    for (int i = tid; i < n; i += grid_size) {
        float val = input[i];
        uint64_t packed = ((uint64_t)__float_as_uint(val) << 32) | i;
        if (val > __uint_as_float(max_val >> 32)) {
            max_val = packed;
        }
    }

    // Block-level ArgMax reduction
    max_val = blockReduceArgMax(max_val);

    // Thread 0 writes index to output
    if (threadIdx.x == 0) {
        output[blockIdx.x] = (uint32_t)(max_val & 0xFFFFFFFF);
    }
}

// Kernel 3: ArgMin with index tracking
__global__ void reduceArgMin(
    const float* __restrict__ input,
    uint32_t* __restrict__ output,
    int n
) {
    int tid = threadIdx.x + blockIdx.x * blockDim.x;
    int grid_size = blockDim.x * gridDim.x;

    uint64_t min_val = ((uint64_t)__float_as_uint(INFINITY) << 32) | 0xFFFFFFFF;

    for (int i = tid; i < n; i += grid_size) {
        float val = input[i];
        uint64_t packed = ((uint64_t)__float_as_uint(val) << 32) | i;
        if (val < __uint_as_float(min_val >> 32)) {
            min_val = packed;
        }
    }

    min_val = blockReduceArgMin(min_val);

    if (threadIdx.x == 0) {
        output[blockIdx.x] = (uint32_t)(min_val & 0xFFFFFFFF);
    }
}

// =============================================================================
// SEGMENTED REDUCTION KERNELS (Tier 4: Per-Row/Per-Column)
// =============================================================================
// Based on: Modern GPU segmented reduction
//
// Use Case: Reduce 2D tensor [M, N] along axis
// - Axis 0: Reduce rows → [N] (sum each column)
// - Axis 1: Reduce columns → [M] (sum each row)
//
// Algorithm: Each block reduces one segment (row or column)

// Axis 1 reduction: [M, N] → [M] (reduce each row)
template<typename T>
__global__ void reduceAxis1(
    const T* __restrict__ input,
    T* __restrict__ output,
    int M, // Number of rows
    int N  // Number of columns
) {
    int row = blockIdx.x;
    if (row >= M) return;

    // Each thread processes N/blockDim.x elements
    T sum = T(0);
    for (int col = threadIdx.x; col < N; col += blockDim.x) {
        sum += input[row * N + col];
    }

    // Block-level reduction
    sum = blockReduceSum(sum);

    if (threadIdx.x == 0) {
        output[row] = sum;
    }
}

// Axis 0 reduction: [M, N] → [N] (reduce each column)
template<typename T>
__global__ void reduceAxis0(
    const T* __restrict__ input,
    T* __restrict__ output,
    int M, // Number of rows
    int N  // Number of columns
) {
    int col = blockIdx.x;
    if (col >= N) return;

    T sum = T(0);
    for (int row = threadIdx.x; row < M; row += blockDim.x) {
        sum += input[row * N + col];
    }

    sum = blockReduceSum(sum);

    if (threadIdx.x == 0) {
        output[col] = sum;
    }
}

// =============================================================================
// KERNEL LAUNCH HELPER FUNCTIONS (Extern "C" for Rust FFI)
// =============================================================================

extern "C" {

// Launch block-level reduction kernel (float32)
void launch_reduce_block_sum_f32(
    const float* input,
    float* output,
    int n,
    int num_blocks,
    int threads_per_block,
    cudaStream_t stream
) {
    reduceBlockLevel<float, 4><<<num_blocks, threads_per_block, 0, stream>>>(
        input, output, n
    );
}

// Launch block-level reduction kernel (float64)
void launch_reduce_block_sum_f64(
    const double* input,
    double* output,
    int n,
    int num_blocks,
    int threads_per_block,
    cudaStream_t stream
) {
    reduceBlockLevel<double, 4><<<num_blocks, threads_per_block, 0, stream>>>(
        input, output, n
    );
}

// Launch ArgMax kernel (float32)
void launch_reduce_argmax_f32(
    const float* input,
    uint32_t* output,
    int n,
    int num_blocks,
    int threads_per_block,
    cudaStream_t stream
) {
    reduceArgMax<<<num_blocks, threads_per_block, 0, stream>>>(
        input, output, n
    );
}

// Launch ArgMin kernel (float32)
void launch_reduce_argmin_f32(
    const float* input,
    uint32_t* output,
    int n,
    int num_blocks,
    int threads_per_block,
    cudaStream_t stream
) {
    reduceArgMin<<<num_blocks, threads_per_block, 0, stream>>>(
        input, output, n
    );
}

// Launch axis 1 reduction kernel (float32)
void launch_reduce_axis1_f32(
    const float* input,
    float* output,
    int M,
    int N,
    int threads_per_block,
    cudaStream_t stream
) {
    reduceAxis1<float><<<M, threads_per_block, 0, stream>>>(
        input, output, M, N
    );
}

// Launch axis 0 reduction kernel (float32)
void launch_reduce_axis0_f32(
    const float* input,
    float* output,
    int M,
    int N,
    int threads_per_block,
    cudaStream_t stream
) {
    reduceAxis0<float><<<N, threads_per_block, 0, stream>>>(
        input, output, M, N
    );
}

} // extern "C"
