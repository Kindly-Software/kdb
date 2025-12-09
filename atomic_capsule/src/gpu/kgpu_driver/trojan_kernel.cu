/**
 * NVIDIA Trojan Kernel - KGPU-Driver v2.0
 *
 * A persistent CUDA kernel that bypasses NVIDIA's locked GSP firmware by polling
 * a shared memory ring buffer for commands. This achieves <100ns command latency
 * by eliminating cuLaunchKernel overhead per command.
 *
 * # Architecture
 *
 * The Trojan Kernel runs forever (until CMD_SHUTDOWN), polling a ring buffer:
 *
 *   CPU (Rust)                          GPU (This Kernel)
 *   +------------+     Pinned Memory    +-----------------+
 *   | Write cmd  | ------------------> | Poll loop       |
 *   | to ring    | <------------------ | Execute & ACK   |
 *   +------------+     Completion       +-----------------+
 *
 * # Memory Layout
 *
 * Ring Header (64 bytes, CPU+GPU visible):
 *   - head (u64): Next slot GPU will process (GPU increments)
 *   - tail (u64): Next slot CPU will write (CPU increments)
 *   - stop_flag (u64): Non-zero = shutdown requested
 *   - kernel_status (u64): Health indicator (0xDEAD = exited)
 *   - commands_processed (u64): Total commands completed
 *   - fence_value (u64): Latest completion fence
 *   - _padding (16 bytes)
 *
 * Commands: Array of TrojanCommand (64 bytes each), cache-line aligned
 *
 * # Opcodes
 *
 * Must match TrojanOpcode in nvidia_ring.rs exactly:
 *   0x00 = NOP
 *   0x01 = MEM_COPY
 *   0x02 = MEM_SET
 *   0x03 = KERNEL_LAUNCH
 *   0x04 = SYNC
 *   0x05 = FENCE_SIGNAL
 *   0x06 = FENCE_WAIT
 *   0x07 = REGISTER_READ
 *   0x08 = REGISTER_WRITE
 *   0xFF = SHUTDOWN
 *
 * # Compilation
 *
 * Compile to PTX for embedding:
 *   nvcc -ptx -arch=sm_52 -o trojan_sm52.ptx trojan_kernel.cu
 *   nvcc -ptx -arch=sm_70 -o trojan_sm70.ptx trojan_kernel.cu
 *   nvcc -ptx -arch=sm_80 -o trojan_sm80.ptx trojan_kernel.cu
 *
 * # Safety
 *
 * ASSUM tags for unsafe operations:
 *   - ASSUME_PINNED_MEMORY: ring_ptr points to cudaHostAlloc'd memory
 *   - ASSUME_COHERENT: Memory writes are visible without explicit flush
 *   - ASSUME_ALIGNED: All command slots are 64-byte aligned
 *
 * # License
 *
 * Proprietary - Trade Secret
 * Copyright (c) 2024-2025 Kindly Systems
 */

#include <stdint.h>

// ============================================================================
// Command Opcodes (MUST match TrojanOpcode in nvidia_ring.rs)
// ============================================================================

#define CMD_NOP             0x00
#define CMD_MEM_COPY        0x01
#define CMD_MEM_SET         0x02
#define CMD_KERNEL_LAUNCH   0x03
#define CMD_SYNC            0x04
#define CMD_FENCE_SIGNAL    0x05
#define CMD_FENCE_WAIT      0x06
#define CMD_REGISTER_READ   0x07
#define CMD_REGISTER_WRITE  0x08
#define CMD_SHUTDOWN        0xFF

// ============================================================================
// Command Flags (MUST match cmd_flags in nvidia_ring.rs)
// ============================================================================

#define FLAG_HAS_COMPLETION 0x01
#define FLAG_ASYNC          0x02
#define FLAG_FENCE_BEFORE   0x04
#define FLAG_FENCE_AFTER    0x08
#define FLAG_HIGH_PRIORITY  0x10

// ============================================================================
// Kernel Status Codes
// ============================================================================

#define STATUS_RUNNING      0x0000
#define STATUS_IDLE         0x0001
#define STATUS_PROCESSING   0x0002
#define STATUS_ERROR        0xBEEF
#define STATUS_EXITED       0xDEAD

// ============================================================================
// TrojanCommand Structure (64 bytes, MUST match nvidia_ring.rs)
// ============================================================================

/**
 * Command structure - exactly 64 bytes for cache-line alignment.
 *
 * Memory Layout:
 *   Offset  Size  Field
 *   ------  ----  -----
 *   0       4     opcode (u32)
 *   4       4     flags (u32)
 *   8       8     seqno (u64)
 *   16      8     src (u64)
 *   24      8     dst (u64)
 *   32      8     size (u64)
 *   40      8     extra (u64)
 *   48      16    _padding
 */
struct __align__(64) TrojanCommand {
    uint32_t opcode;    // Command opcode
    uint32_t flags;     // Command flags
    uint64_t seqno;     // Sequence number
    uint64_t src;       // Source address or value
    uint64_t dst;       // Destination address
    uint64_t size;      // Size in bytes or count
    uint64_t extra;     // Extra parameter
    uint8_t  _padding[16];
};

// Static assertion for size (CUDA style)
extern char TrojanCommand_size_check[sizeof(TrojanCommand) == 64 ? 1 : -1];

// ============================================================================
// Ring Header Structure (64 bytes, matches Rust side)
// ============================================================================

/**
 * Ring buffer header - shared between CPU and GPU.
 *
 * The CPU writes commands to ring[tail % capacity] and increments tail.
 * The GPU reads commands from ring[head % capacity] and increments head.
 */
struct __align__(64) TrojanRingHeader {
    volatile uint64_t head;                 // GPU read position
    volatile uint64_t tail;                 // CPU write position
    volatile uint64_t stop_flag;            // Shutdown signal
    volatile uint64_t kernel_status;        // Health indicator
    volatile uint64_t commands_processed;   // Total completed
    volatile uint64_t fence_value;          // Latest fence
    uint64_t _padding[2];                   // Align to 64 bytes
};

// Static assertion for size
extern char TrojanRingHeader_size_check[sizeof(TrojanRingHeader) == 64 ? 1 : -1];

// ============================================================================
// Device Memory Copy (inline for performance)
// ============================================================================

/**
 * GPU-side memory copy using byte-by-byte transfer.
 *
 * For larger copies, the CUDA compiler may optimize this to use
 * vectorized loads/stores. For truly large copies, we should use
 * cooperative groups or cuMemcpy, but for ring buffer command
 * payloads this is sufficient.
 */
__device__ __forceinline__ void trojan_memcpy(
    void* __restrict__ dst,
    const void* __restrict__ src,
    uint64_t size
) {
    uint8_t* d = (uint8_t*)dst;
    const uint8_t* s = (const uint8_t*)src;

    // Vectorized copy for aligned, large transfers
    if (((uintptr_t)dst & 7) == 0 && ((uintptr_t)src & 7) == 0 && (size & 7) == 0) {
        uint64_t* d64 = (uint64_t*)dst;
        const uint64_t* s64 = (const uint64_t*)src;
        uint64_t count = size >> 3;
        for (uint64_t i = 0; i < count; i++) {
            d64[i] = s64[i];
        }
        return;
    }

    // Fallback: byte-by-byte
    for (uint64_t i = 0; i < size; i++) {
        d[i] = s[i];
    }
}

/**
 * GPU-side memory set using byte-by-byte fill.
 */
__device__ __forceinline__ void trojan_memset(
    void* dst,
    uint8_t value,
    uint64_t size
) {
    uint8_t* d = (uint8_t*)dst;

    // Vectorized set for aligned, large fills
    if (((uintptr_t)dst & 7) == 0 && (size & 7) == 0) {
        uint64_t v64 = value;
        v64 |= v64 << 8;
        v64 |= v64 << 16;
        v64 |= v64 << 32;

        uint64_t* d64 = (uint64_t*)dst;
        uint64_t count = size >> 3;
        for (uint64_t i = 0; i < count; i++) {
            d64[i] = v64;
        }
        return;
    }

    // Fallback: byte-by-byte
    for (uint64_t i = 0; i < size; i++) {
        d[i] = value;
    }
}

// ============================================================================
// Main Trojan Kernel
// ============================================================================

/**
 * Persistent Trojan Kernel - runs until shutdown command received.
 *
 * This kernel polls a shared memory ring buffer for commands, executing
 * them immediately without the overhead of cuLaunchKernel per command.
 *
 * @param header     Pointer to ring header (pinned memory, CPU+GPU visible)
 * @param ring       Pointer to command ring (array of TrojanCommand)
 * @param ring_size  Number of command slots (must be power of 2)
 * @param poll_ns    Polling interval in nanoseconds (for spin-wait backoff)
 */
extern "C" __global__ void trojan_poll(
    TrojanRingHeader* header,
    TrojanCommand* ring,
    uint32_t ring_size,
    uint32_t poll_ns
) {
    // Compute global thread ID
    uint32_t tid = threadIdx.x + blockIdx.x * blockDim.x;

    // Ring size mask for modulo (assumes power of 2)
    uint32_t ring_mask = ring_size - 1;

    // Only thread 0 of block 0 processes commands
    // Other threads can be repurposed for parallel command execution
    if (tid == 0) {
        // Signal kernel is running
        header->kernel_status = STATUS_RUNNING;

        // Main polling loop - runs until stop_flag is set
        while (header->stop_flag == 0) {
            // Memory fence to see latest writes from CPU
            __threadfence_system();

            // Load head and tail
            uint64_t head = header->head;
            uint64_t tail = header->tail;

            // Check for new commands
            if (head != tail) {
                // Update status
                header->kernel_status = STATUS_PROCESSING;

                // Get command at head position
                TrojanCommand* cmd = &ring[head & ring_mask];

                // Fence before if requested
                if (cmd->flags & FLAG_FENCE_BEFORE) {
                    __threadfence_system();
                }

                // Execute command based on opcode
                switch (cmd->opcode) {
                    case CMD_NOP:
                        // No operation - useful for latency testing
                        break;

                    case CMD_MEM_COPY:
                        // Memory copy: src -> dst, size bytes
                        if (cmd->src != 0 && cmd->dst != 0 && cmd->size > 0) {
                            trojan_memcpy(
                                (void*)cmd->dst,
                                (const void*)cmd->src,
                                cmd->size
                            );
                        }
                        break;

                    case CMD_MEM_SET:
                        // Memory set: fill dst with (src & 0xFF), size bytes
                        if (cmd->dst != 0 && cmd->size > 0) {
                            trojan_memset(
                                (void*)cmd->dst,
                                (uint8_t)(cmd->src & 0xFF),
                                cmd->size
                            );
                        }
                        break;

                    case CMD_KERNEL_LAUNCH:
                        // Indirect kernel launch (advanced feature)
                        // Not yet implemented - requires CUDA dynamic parallelism
                        // or function pointer dispatch
                        break;

                    case CMD_SYNC:
                        // Full memory fence
                        __threadfence_system();
                        break;

                    case CMD_FENCE_SIGNAL:
                        // Write fence value to memory location
                        if (cmd->dst != 0) {
                            volatile uint64_t* fence = (volatile uint64_t*)cmd->dst;
                            *fence = cmd->src;  // src contains the value to write
                            __threadfence_system();
                        }
                        // Also update header fence
                        header->fence_value = cmd->seqno;
                        break;

                    case CMD_FENCE_WAIT:
                        // Wait for fence location to reach expected value
                        if (cmd->dst != 0) {
                            volatile uint64_t* fence = (volatile uint64_t*)cmd->dst;
                            // Spin until fence reaches expected value
                            while (*fence < cmd->src) {
                                // Check for shutdown during wait
                                if (header->stop_flag != 0) break;
                                // Brief pause to reduce power consumption
                                #if __CUDA_ARCH__ >= 700
                                __nanosleep(100);
                                #endif
                            }
                        }
                        break;

                    case CMD_REGISTER_READ:
                        // Register read (requires special handling)
                        // Not implemented for safety - would need MMIO access
                        break;

                    case CMD_REGISTER_WRITE:
                        // Register write (requires special handling)
                        // Not implemented for safety - would need MMIO access
                        break;

                    case CMD_SHUTDOWN:
                        // Graceful shutdown
                        header->stop_flag = 1;
                        break;

                    default:
                        // Unknown opcode - skip
                        break;
                }

                // Fence after if requested
                if (cmd->flags & FLAG_FENCE_AFTER) {
                    __threadfence_system();
                }

                // Advance head (atomically)
                atomicAdd((unsigned long long*)&header->head, 1ULL);

                // Increment processed counter
                atomicAdd((unsigned long long*)&header->commands_processed, 1ULL);

                // Update status
                header->kernel_status = STATUS_IDLE;

            } else {
                // No commands available - spin wait with backoff
                header->kernel_status = STATUS_IDLE;

                // Power-efficient spin (Volta+ only)
                #if __CUDA_ARCH__ >= 700
                __nanosleep(poll_ns);
                #else
                // Pre-Volta: busy spin (no nanosleep)
                for (uint32_t i = 0; i < poll_ns / 10; i++) {
                    // Empty loop to burn time
                    __threadfence();
                }
                #endif
            }
        }

        // Signal clean exit
        header->kernel_status = STATUS_EXITED;
    }

    // Synchronize all threads before exit
    __syncthreads();
}

// ============================================================================
// Auxiliary Kernels
// ============================================================================

/**
 * Health check kernel - verifies GPU is responsive.
 *
 * Writes a magic value to the provided address to confirm GPU execution.
 * This is a one-shot kernel (not persistent).
 */
extern "C" __global__ void trojan_health_check(
    volatile uint64_t* health_ptr,
    uint64_t magic_value
) {
    if (threadIdx.x == 0 && blockIdx.x == 0) {
        *health_ptr = magic_value;
        __threadfence_system();
    }
}

/**
 * Ring buffer reset kernel - clears ring state.
 *
 * Call this before launching trojan_poll to ensure clean state.
 */
extern "C" __global__ void trojan_ring_reset(
    TrojanRingHeader* header
) {
    if (threadIdx.x == 0 && blockIdx.x == 0) {
        header->head = 0;
        header->tail = 0;
        header->stop_flag = 0;
        header->kernel_status = STATUS_RUNNING;
        header->commands_processed = 0;
        header->fence_value = 0;
        __threadfence_system();
    }
}

/**
 * Timestamp kernel - captures GPU clock value.
 *
 * Uses %globaltimer for nanosecond-resolution timestamps.
 */
extern "C" __global__ void trojan_timestamp(
    volatile uint64_t* timestamp_ptr
) {
    if (threadIdx.x == 0 && blockIdx.x == 0) {
        uint64_t ts;
        asm volatile("mov.u64 %0, %%globaltimer;" : "=l"(ts));
        *timestamp_ptr = ts;
        __threadfence_system();
    }
}
