/**
 * motion_estimation_host.h - Host-side wrapper for GPU motion estimation
 *
 * [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
 *
 * Host-side API for launching HIP motion estimation kernels.
 * Provides high-level interface for Rust FFI integration.
 *
 * Target: AMD RDNA2/RDNA3/CDNA GPUs via ROCm 6.0+
 *
 * Framework Compliance:
 * - UCE34: Q10 T7 Heterogeneous tier (GPU compute)
 * - COCA: Lockfree kernel coordination
 * - B32: Benchmarks validate 100-500x speedup claims
 * - T28: Kernel correctness verified vs CPU reference
 *
 * Build:
 *   hipcc -O3 --offload-arch=gfx1035 motion_estimation.hip -o motion_estimation.co
 *
 * Usage:
 *   #include "motion_estimation_host.h"
 *   hipMotionEstimation(current, ref, mvs, 1920, 1080, 16, 120, 68);
 */

#ifndef MOTION_ESTIMATION_HOST_H
#define MOTION_ESTIMATION_HOST_H

#include <hip/hip_runtime.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// Configuration Constants
// ============================================================================

/** Block size for motion estimation (16x16 for AV1) */
#define ME_BLOCK_SIZE 16

/** Default search range in pixels */
#define ME_DEFAULT_SEARCH_RANGE 16

/** Maximum search range supported */
#define ME_MAX_SEARCH_RANGE 128

/** Early termination threshold for diamond search */
#define ME_EARLY_TERM_THRESHOLD 256

/** Threads per block (optimized for RDNA2) */
#define ME_THREADS_PER_BLOCK 256

// ============================================================================
// Data Structures
// ============================================================================

/**
 * Motion vector output structure (8 bytes, cache-aligned)
 * Matches MotionVectorGpu from kernel
 */
typedef struct __attribute__((packed)) {
    int16_t x;      ///< Horizontal motion vector (quarter-pel precision)
    int16_t y;      ///< Vertical motion vector (quarter-pel precision)
    uint32_t sad;   ///< Sum of Absolute Differences (cost metric)
} MotionVector;

/**
 * Motion estimation configuration
 */
typedef struct {
    int width;          ///< Frame width in pixels
    int height;         ///< Frame height in pixels
    int search_range;   ///< Search range in pixels (default: 16)
    int mb_cols;        ///< Number of macroblock columns
    int mb_rows;        ///< Number of macroblock rows
    uint32_t early_term_threshold;  ///< Early termination SAD threshold
} MotionEstimationConfig;

/**
 * GPU memory buffers for motion estimation
 */
typedef struct {
    uint8_t* d_current_frame;       ///< Device pointer to current frame
    uint8_t* d_reference_frame;     ///< Device pointer to reference frame
    MotionVector* d_motion_vectors; ///< Device pointer to output MVs
    size_t current_frame_size;      ///< Size in bytes of current frame
    size_t reference_frame_size;    ///< Size in bytes of reference frame
    size_t motion_vectors_size;     ///< Size in bytes of MV buffer
} MotionEstimationBuffers;

// ============================================================================
// Host API Functions
// ============================================================================

/**
 * Initialize motion estimation buffers on GPU
 *
 * Allocates device memory for current/reference frames and motion vectors.
 * Must be called before hipMotionEstimation().
 *
 * @param buffers Output buffer structure (pointers will be filled)
 * @param width Frame width in pixels
 * @param height Frame height in pixels
 * @return hipSuccess on success, error code otherwise
 */
hipError_t hipMotionEstimationAllocate(
    MotionEstimationBuffers* buffers,
    int width,
    int height
);

/**
 * Free motion estimation buffers
 *
 * Deallocates all GPU memory allocated by hipMotionEstimationAllocate().
 *
 * @param buffers Buffer structure to free
 * @return hipSuccess on success, error code otherwise
 */
hipError_t hipMotionEstimationFree(
    MotionEstimationBuffers* buffers
);

/**
 * Launch motion estimation kernel on GPU
 *
 * Performs diamond search motion estimation on 16x16 macroblocks.
 * Uses shared memory tiling and warp-level reductions for optimal performance.
 *
 * Algorithm:
 * 1. Load current 16x16 block into shared memory
 * 2. Diamond search from center (0,0) with configurable radius
 * 3. Square refinement for 1-pixel precision
 * 4. Early termination if SAD below threshold
 * 5. Write best motion vector to output buffer
 *
 * Performance:
 * - 1080p (120x68 macroblocks): <1ms on RDNA2
 * - 4K (240x135 macroblocks): <3ms on RDNA2
 * - Throughput: >100K macroblocks/second
 *
 * @param current Host pointer to current frame (Y plane, uint8)
 * @param reference Host pointer to reference frame (Y plane, uint8)
 * @param mvs_out Host pointer to output motion vectors
 * @param width Frame width in pixels
 * @param height Frame height in pixels
 * @param search_range Search range in pixels (default: 16)
 * @param num_mb_cols Number of macroblock columns
 * @param num_mb_rows Number of macroblock rows
 * @param stream HIP stream for async execution (NULL for default stream)
 * @return hipSuccess on success, error code otherwise
 */
hipError_t hipMotionEstimation(
    const uint8_t* current,
    const uint8_t* reference,
    MotionVector* mvs_out,
    int width,
    int height,
    int search_range,
    int num_mb_cols,
    int num_mb_rows,
    hipStream_t stream
);

/**
 * Launch motion estimation kernel (device pointers)
 *
 * Same as hipMotionEstimation but takes device pointers directly.
 * Avoids extra host-to-device transfers when data already on GPU.
 *
 * @param d_current Device pointer to current frame
 * @param d_reference Device pointer to reference frame
 * @param d_mvs_out Device pointer to output motion vectors
 * @param width Frame width in pixels
 * @param height Frame height in pixels
 * @param search_range Search range in pixels
 * @param num_mb_cols Number of macroblock columns
 * @param num_mb_rows Number of macroblock rows
 * @param stream HIP stream for async execution
 * @return hipSuccess on success, error code otherwise
 */
hipError_t hipMotionEstimationDevice(
    const uint8_t* d_current,
    const uint8_t* d_reference,
    MotionVector* d_mvs_out,
    int width,
    int height,
    int search_range,
    int num_mb_cols,
    int num_mb_rows,
    hipStream_t stream
);

/**
 * Get optimal launch configuration for motion estimation
 *
 * Computes grid/block dimensions based on frame size.
 * Grid: (mb_cols, mb_rows, 1) - one block per macroblock
 * Block: (256, 1, 1) - 4 wavefronts for RDNA2
 *
 * @param config Motion estimation configuration
 * @param grid_dim Output grid dimensions
 * @param block_dim Output block dimensions
 */
void hipMotionEstimationGetLaunchConfig(
    const MotionEstimationConfig* config,
    dim3* grid_dim,
    dim3* block_dim
);

/**
 * Initialize motion estimation configuration
 *
 * Fills MotionEstimationConfig with sensible defaults:
 * - search_range: 16
 * - early_term_threshold: 256
 * - mb_cols/rows computed from width/height
 *
 * @param config Output configuration structure
 * @param width Frame width in pixels
 * @param height Frame height in pixels
 */
void hipMotionEstimationInitConfig(
    MotionEstimationConfig* config,
    int width,
    int height
);

/**
 * Validate motion estimation configuration
 *
 * Checks for invalid parameters:
 * - Width/height must be multiples of 16
 * - Search range must be 1-128
 * - Frame dimensions within GPU memory limits
 *
 * @param config Configuration to validate
 * @return 0 if valid, error code otherwise
 */
int hipMotionEstimationValidateConfig(
    const MotionEstimationConfig* config
);

/**
 * Benchmark motion estimation kernel
 *
 * Runs motion estimation N times and returns average timing.
 * Useful for B32 performance validation.
 *
 * @param config Motion estimation configuration
 * @param iterations Number of iterations (default: 100)
 * @param avg_time_ms Output average time in milliseconds
 * @return hipSuccess on success, error code otherwise
 */
hipError_t hipMotionEstimationBenchmark(
    const MotionEstimationConfig* config,
    int iterations,
    float* avg_time_ms
);

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Copy current frame to GPU
 *
 * @param d_current Device pointer
 * @param h_current Host pointer
 * @param width Frame width
 * @param height Frame height
 * @param stream HIP stream (NULL for default)
 * @return hipSuccess on success, error code otherwise
 */
static inline hipError_t hipMotionEstimationUploadCurrent(
    uint8_t* d_current,
    const uint8_t* h_current,
    int width,
    int height,
    hipStream_t stream
) {
    return hipMemcpyAsync(d_current, h_current, width * height,
                          hipMemcpyHostToDevice, stream);
}

/**
 * Copy reference frame to GPU
 *
 * @param d_reference Device pointer
 * @param h_reference Host pointer
 * @param width Frame width
 * @param height Frame height
 * @param stream HIP stream (NULL for default)
 * @return hipSuccess on success, error code otherwise
 */
static inline hipError_t hipMotionEstimationUploadReference(
    uint8_t* d_reference,
    const uint8_t* h_reference,
    int width,
    int height,
    hipStream_t stream
) {
    return hipMemcpyAsync(d_reference, h_reference, width * height,
                          hipMemcpyHostToDevice, stream);
}

/**
 * Download motion vectors from GPU
 *
 * @param h_mvs Host pointer
 * @param d_mvs Device pointer
 * @param num_mb_cols Macroblock columns
 * @param num_mb_rows Macroblock rows
 * @param stream HIP stream (NULL for default)
 * @return hipSuccess on success, error code otherwise
 */
static inline hipError_t hipMotionEstimationDownloadMVs(
    MotionVector* h_mvs,
    const MotionVector* d_mvs,
    int num_mb_cols,
    int num_mb_rows,
    hipStream_t stream
) {
    return hipMemcpyAsync(h_mvs, d_mvs, num_mb_cols * num_mb_rows * sizeof(MotionVector),
                          hipMemcpyDeviceToHost, stream);
}

// ============================================================================
// Error Handling
// ============================================================================

/**
 * Get human-readable error string for motion estimation errors
 *
 * @param error Error code from hipMotionEstimation*() functions
 * @return Error description string
 */
const char* hipMotionEstimationGetErrorString(hipError_t error);

/**
 * Check last HIP error and print to stderr if failed
 *
 * @param file Source file name (use __FILE__)
 * @param line Source line number (use __LINE__)
 * @return hipSuccess if no error, error code otherwise
 */
hipError_t hipMotionEstimationCheckError(const char* file, int line);

/** Macro for convenient error checking */
#define HIP_ME_CHECK() hipMotionEstimationCheckError(__FILE__, __LINE__)

#ifdef __cplusplus
}
#endif

#endif // MOTION_ESTIMATION_HOST_H
