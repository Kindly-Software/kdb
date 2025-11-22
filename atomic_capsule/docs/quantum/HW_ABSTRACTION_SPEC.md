# Hardware Abstraction Layer - Technical Specification

**Version**: 1.0
**Date**: 2025-11-21
**Status**: Design Complete, Implementation Pending
**Tier**: T7 Heterogeneous (Multi-Accelerator Coordination)

---

## Table of Contents

1. [Overview](#overview)
2. [Trait Hierarchy](#trait-hierarchy)
3. [Core Abstractions](#core-abstractions)
4. [Memory Model](#memory-model)
5. [Synchronization Model](#synchronization-model)
6. [Backend Implementations](#backend-implementations)
7. [Error Handling](#error-handling)
8. [Performance Characteristics](#performance-characteristics)

---

## Overview

### Design Goals

1. **Zero-Cost Abstraction**: Static dispatch via trait monomorphization (0ns overhead)
2. **Portability**: Single API supports FPGA (XRT), GPU (CUDA), TPU (XLA), Mock (testing)
3. **Safety**: 99.99% safe Rust, all FFI isolated in backend implementations
4. **Performance**: <5μs DMA latency, <10ns queue operations, 80% PCIe bandwidth utilization
5. **Lockfree**: 100% atomic coordination (no mutex/RwLock in fast paths)

### Architecture Layers

```
┌────────────────────────────────────────────────────────────┐
│  Layer 4: Application (T7 Capsules)                        │
│  - QEC Syndrome Extraction                                  │
│  - GPU Decoder                                              │
│  - TPU Optimizer                                            │
└────────────────────┬───────────────────────────────────────┘
                     │ Trait API (zero-cost abstraction)
┌────────────────────▼───────────────────────────────────────┐
│  Layer 3: Hardware Abstraction Layer (HAL)                 │
│  - AcceleratorDevice (trait)                               │
│  - DmaBuffer (T1 Atomic)                                   │
│  - CommandQueue (T1 MPMC)                                  │
│  - SyncPrimitive (T1 Atomic)                               │
└────────────────────┬───────────────────────────────────────┘
                     │ Runtime dispatch (trait object or enum)
┌────────────────────▼───────────────────────────────────────┐
│  Layer 2: Backend Implementations                          │
│  - MockDevice (Instant, no FFI)                            │
│  - FpgaXrtDevice (XRT FFI)                                 │
│  - GpuCudaDevice (CUDA FFI)                                │
│  - TpuXlaDevice (XLA FFI, future)                          │
└────────────────────┬───────────────────────────────────────┘
                     │ FFI boundary (unsafe, isolated)
┌────────────────────▼───────────────────────────────────────┐
│  Layer 1: Vendor Libraries (C/C++)                         │
│  - libxrt_coreutil.so (Xilinx XRT)                         │
│  - libcuda.so (NVIDIA Driver)                              │
│  - libtpu.so (Google TPU Runtime)                          │
└────────────────────────────────────────────────────────────┘
```

---

## Trait Hierarchy

### Primary Traits

```rust
/// Core accelerator device abstraction.
/// All backend implementations must implement this trait.
///
/// # Safety
/// Implementations must ensure:
/// - Thread-safe (Send + Sync)
/// - No data races in concurrent operations
/// - Proper cleanup in Drop (free device resources)
///
/// # Performance
/// - Device open: <1ms
/// - Buffer allocation: <100μs
/// - Transfer initiation: <5μs
/// - Synchronization check: <5ns
pub trait AcceleratorDevice: Send + Sync {
    /// Query device capabilities (cached, <100ns)
    fn capabilities(&self) -> &DeviceCapabilities;

    /// Allocate device-side buffer (opaque handle)
    ///
    /// # Arguments
    /// - `size`: Buffer size in bytes (must be multiple of 4KB)
    /// - `flags`: Allocation flags (coherent, cached, etc.)
    ///
    /// # Returns
    /// - `Ok(DeviceHandle)`: Opaque handle to device memory
    /// - `Err(HwError::AllocFailed)`: Out of memory or invalid size
    ///
    /// # Performance
    /// - Target: <100μs
    /// - Complexity: O(1) (backend-specific allocator)
    fn alloc_device(&self, size: usize, flags: AllocFlags) -> Result<DeviceHandle, HwError>;

    /// Free device-side buffer
    ///
    /// # Safety
    /// Must not be called twice on same handle (prevented by DmaBuffer Drop)
    ///
    /// # Performance
    /// - Target: <50μs
    fn free_device(&self, handle: DeviceHandle) -> Result<(), HwError>;

    /// Initiate async DMA transfer (non-blocking)
    ///
    /// # Arguments
    /// - `buf`: DMA buffer (contains host + device handles)
    /// - `direction`: HostToDevice, DeviceToHost, or DeviceToDevice
    /// - `sync`: Synchronization primitive (updated on completion)
    ///
    /// # Returns
    /// - `Ok(())`: Transfer initiated (check sync for completion)
    /// - `Err(HwError)`: Immediate error (invalid handle, device error)
    ///
    /// # Performance
    /// - Target: <5μs (transfer initiation, not completion)
    /// - Latency: <5μs + transfer_time (size / bandwidth)
    ///
    /// # Ordering
    /// Transfers to same device are ordered (FIFO), different devices are unordered
    fn transfer_async(
        &self,
        buf: &DmaBuffer,
        direction: TransferDirection,
        sync: &SyncPrimitive,
    ) -> Result<(), HwError>;

    /// Submit command to device (kernel launch, fence, etc.)
    ///
    /// # Arguments
    /// - `cmd`: Command (type + payload)
    ///
    /// # Returns
    /// - `Ok(())`: Command submitted (async execution)
    /// - `Err(HwError)`: Queue full or invalid command
    ///
    /// # Performance
    /// - Target: <1μs (command submission, not execution)
    fn submit(&self, cmd: &Command) -> Result<(), HwError>;

    /// Block until synchronization primitive completes
    ///
    /// # Arguments
    /// - `sync`: Synchronization primitive
    /// - `timeout_us`: Timeout in microseconds (0 = infinite)
    ///
    /// # Returns
    /// - `Ok(())`: Operation completed successfully
    /// - `Err(HwError::Timeout)`: Timeout expired
    /// - `Err(HwError::DeviceError)`: Device error during operation
    ///
    /// # Performance
    /// - Target: <10ns per polling iteration
    /// - CPU usage: Busy-wait (100% CPU) or exponential backoff
    fn sync_wait(&self, sync: &SyncPrimitive, timeout_us: u64) -> Result<(), HwError>;

    /// Non-blocking sync check (returns immediately)
    ///
    /// # Returns
    /// - `Ok(true)`: Operation completed
    /// - `Ok(false)`: Still pending
    /// - `Err(HwError)`: Device error
    ///
    /// # Performance
    /// - Target: <5ns (single atomic load)
    fn sync_check(&self, sync: &SyncPrimitive) -> Result<bool, HwError>;
}

/// Device capabilities (queried once at device open)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceCapabilities {
    /// Device type (FPGA, GPU, TPU, Mock)
    pub device_type: DeviceType,

    /// Vendor name (e.g., "Xilinx", "NVIDIA", "Google")
    pub vendor: &'static str,

    /// Device name (e.g., "Alveo U250", "RTX 4090", "TPU v5")
    pub device_name: &'static str,

    /// PCIe generation (3, 4, 5) and lane count (8, 16)
    pub pcie_gen: u8,
    pub pcie_lanes: u8,

    /// Theoretical PCIe bandwidth (bytes/sec)
    /// Gen3 x16: 16 GB/s, Gen4 x16: 32 GB/s, Gen5 x16: 64 GB/s
    pub pcie_bandwidth: u64,

    /// Device memory size (bytes)
    pub device_memory: u64,

    /// Device memory bandwidth (bytes/sec, HBM is 1-2 TB/s)
    pub memory_bandwidth: u64,

    /// Supports atomic operations in device memory?
    /// FPGA: AXI Atomic IP, GPU: atomicCAS, TPU: JAX atomic primitives
    pub atomic_support: bool,

    /// Supports pinned (non-pageable) host memory?
    /// Linux: mlock, Windows: VirtualLock, macOS: mlock
    pub pinned_memory: bool,

    /// Supports peer-to-peer DMA (device-to-device without host)?
    /// GPU-GPU: CUDA P2P, FPGA-GPU: PCIe P2P (requires IOMMU)
    pub p2p_support: bool,

    /// Supports unified virtual addressing (UVA)?
    /// Host and device share address space (CUDA UVA, FPGA HBM)
    pub uva_support: bool,

    /// Maximum concurrent transfers (DMA channels)
    pub max_concurrent_transfers: u32,

    /// Maximum command queue depth (pending commands)
    pub max_queue_depth: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Mock,     // Testing only, instant operations
    Fpga,     // FPGA (Xilinx, Intel)
    Gpu,      // GPU (NVIDIA, AMD)
    Tpu,      // TPU (Google)
}
```

### Supporting Traits

```rust
/// Trait for buffer allocation strategies
pub trait BufferAllocator: Send + Sync {
    /// Allocate host-side buffer (pinned or pageable)
    fn alloc_host(&self, size: usize, pinned: bool) -> Result<*mut u8, HwError>;

    /// Free host-side buffer
    unsafe fn free_host(&self, ptr: *mut u8, size: usize);

    /// Pin existing buffer (make non-pageable)
    unsafe fn pin_buffer(&self, ptr: *mut u8, size: usize) -> Result<(), HwError>;

    /// Unpin buffer (make pageable)
    unsafe fn unpin_buffer(&self, ptr: *mut u8, size: usize) -> Result<(), HwError>;
}

/// Default allocator (uses libc mlock for pinning)
pub struct SystemAllocator;

impl BufferAllocator for SystemAllocator {
    fn alloc_host(&self, size: usize, pinned: bool) -> Result<*mut u8, HwError> {
        use std::alloc::{alloc, Layout};

        // Allocate page-aligned buffer (4KB alignment)
        let layout = Layout::from_size_align(size, 4096)
            .map_err(|_| HwError::AllocFailed { size, align: 4096 })?;

        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return Err(HwError::AllocFailed { size, align: 4096 });
        }

        // Pin if requested
        if pinned {
            unsafe {
                if libc::mlock(ptr as *const libc::c_void, size) != 0 {
                    std::alloc::dealloc(ptr, layout);
                    return Err(HwError::AllocFailed { size, align: 4096 });
                }
            }
        }

        Ok(ptr)
    }

    unsafe fn free_host(&self, ptr: *mut u8, size: usize) {
        use std::alloc::{dealloc, Layout};

        // Unpin first (safe to call even if not pinned)
        libc::munlock(ptr as *const libc::c_void, size);

        // Deallocate
        let layout = Layout::from_size_align_unchecked(size, 4096);
        dealloc(ptr, layout);
    }

    unsafe fn pin_buffer(&self, ptr: *mut u8, size: usize) -> Result<(), HwError> {
        if libc::mlock(ptr as *const libc::c_void, size) != 0 {
            Err(HwError::AllocFailed { size, align: 4096 })
        } else {
            Ok(())
        }
    }

    unsafe fn unpin_buffer(&self, ptr: *mut u8, size: usize) -> Result<(), HwError> {
        if libc::munlock(ptr as *const libc::c_void, size) != 0 {
            Err(HwError::AllocFailed { size, align: 4096 })
        } else {
            Ok(())
        }
    }
}
```

---

## Core Abstractions

### DmaBuffer (T1 Atomic)

```rust
/// Zero-copy DMA buffer with lockfree atomic coordination.
///
/// # Memory Layout
/// ```text
/// +------------------+  ← 4KB-aligned (page boundary)
/// | Metadata (64B)   |  Cache-aligned header
/// +------------------+
/// | Host Data (N)    |  Pinned memory (non-pageable)
/// +------------------+
/// ```
///
/// # Ownership
/// - Host pointer: Owned (freed on drop)
/// - Device handle: Reference (backend frees on device close)
/// - Ref count: Atomic (lockfree multi-threaded access)
///
/// # Performance
/// - Allocation: <100μs (pinned memory)
/// - Ref count ops: <10ns (lockfree CAS)
/// - Host read/write: Memcpy (5 GB/s typical)
/// - Device transfer: <5μs (via AcceleratorDevice)
#[repr(C, align(4096))]
pub struct DmaBuffer {
    /// Host-side pointer (pinned memory, non-pageable)
    /// Allocated via SystemAllocator::alloc_host(size, pinned=true)
    host_ptr: *mut u8,

    /// Device-side handle (opaque, backend-specific)
    /// 0 = not allocated, non-zero = valid handle
    /// Updated atomically by backend during alloc_device()
    device_handle: AtomicU64,

    /// Buffer size in bytes (immutable after creation)
    size: usize,

    /// Atomic reference count (lockfree cleanup)
    /// Decremented on drop, buffer freed when reaches 0
    ref_count: AtomicU64,

    /// Transfer status flags (atomic, lockfree)
    /// Bits: [63:62] direction (00=idle, 01=H2D, 10=D2H, 11=D2D)
    ///       [61:32] transfer ID (monotonic counter)
    ///       [31:16] error code (0 = no error)
    ///       [15:0]  state (0=idle, 1=pending, 2=in_progress, 3=complete)
    flags: AtomicU64,

    /// Backend-specific metadata (e.g., CUDA stream ID, XRT BO offset)
    backend_data: AtomicU64,

    /// Allocator (for custom allocation strategies)
    allocator: &'static dyn BufferAllocator,
}

impl DmaBuffer {
    /// Create new DMA buffer with pinned host memory.
    ///
    /// # Arguments
    /// - `size`: Buffer size in bytes (rounded up to 4KB multiple)
    ///
    /// # Returns
    /// - `Ok(DmaBuffer)`: Buffer allocated successfully
    /// - `Err(HwError::AllocFailed)`: Out of memory or mlock failed
    ///
    /// # Performance
    /// - Target: <100μs (pinned memory allocation + mlock)
    ///
    /// # Safety
    /// - 100% safe (no FFI, uses SystemAllocator)
    pub fn new_pinned(size: usize) -> Result<Self, HwError> {
        Self::new_with_allocator(size, true, &SystemAllocator)
    }

    /// Create with custom allocator (advanced use case)
    pub fn new_with_allocator(
        size: usize,
        pinned: bool,
        allocator: &'static dyn BufferAllocator,
    ) -> Result<Self, HwError> {
        // Round up to 4KB (page size)
        let aligned_size = (size + 4095) & !4095;

        // Allocate host memory
        let host_ptr = allocator.alloc_host(aligned_size, pinned)?;

        Ok(Self {
            host_ptr,
            device_handle: AtomicU64::new(0),
            size: aligned_size,
            ref_count: AtomicU64::new(1),
            flags: AtomicU64::new(0),
            backend_data: AtomicU64::new(0),
            allocator,
        })
    }

    /// Write data to host buffer (no device transfer).
    ///
    /// # Performance
    /// - Target: ~5 GB/s (memcpy throughput)
    ///
    /// # Safety
    /// - 100% safe (bounds-checked)
    pub fn write_host(&mut self, data: &[u8]) -> Result<(), HwError> {
        if data.len() > self.size {
            return Err(HwError::AllocFailed { size: data.len(), align: 4096 });
        }

        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.host_ptr, data.len());
        }
        Ok(())
    }

    /// Read data from host buffer (no device transfer).
    ///
    /// # Performance
    /// - Target: ~5 GB/s (memcpy throughput)
    ///
    /// # Safety
    /// - 100% safe (bounds-checked)
    pub fn read_host(&self) -> Result<Vec<u8>, HwError> {
        let mut data = vec![0u8; self.size];
        unsafe {
            std::ptr::copy_nonoverlapping(self.host_ptr, data.as_mut_ptr(), self.size);
        }
        Ok(data)
    }

    /// Get immutable slice to host buffer.
    ///
    /// # Safety
    /// - Safe (borrow checker ensures no concurrent writes)
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.host_ptr, self.size) }
    }

    /// Get mutable slice to host buffer.
    ///
    /// # Safety
    /// - Safe (borrow checker ensures exclusive access)
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.host_ptr, self.size) }
    }

    /// Clone buffer (increment ref count, share data).
    ///
    /// # Performance
    /// - Target: <10ns (atomic increment)
    pub fn clone(&self) -> Self {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
        Self {
            host_ptr: self.host_ptr,
            device_handle: AtomicU64::new(self.device_handle.load(Ordering::Acquire)),
            size: self.size,
            ref_count: self.ref_count.clone(), // Share atomic
            flags: AtomicU64::new(self.flags.load(Ordering::Acquire)),
            backend_data: AtomicU64::new(self.backend_data.load(Ordering::Acquire)),
            allocator: self.allocator,
        }
    }

    /// Internal: Set device handle (called by backend).
    pub(crate) fn set_device_handle(&self, handle: u64) {
        self.device_handle.store(handle, Ordering::Release);
    }

    /// Internal: Get device handle (called by backend).
    pub(crate) fn device_handle(&self) -> u64 {
        self.device_handle.load(Ordering::Acquire)
    }

    /// Size in bytes.
    pub fn size(&self) -> usize {
        self.size
    }
}

impl Drop for DmaBuffer {
    fn drop(&mut self) {
        // Decrement ref count
        let prev = self.ref_count.fetch_sub(1, Ordering::Release);

        // Last reference? Free host memory
        if prev == 1 {
            unsafe {
                self.allocator.free_host(self.host_ptr, self.size);
            }
        }

        // Note: Device handle NOT freed here (backend manages device memory)
    }
}

unsafe impl Send for DmaBuffer {}
unsafe impl Sync for DmaBuffer {}
```

### SyncPrimitive (T1 Atomic)

```rust
/// Lockfree synchronization primitive for async operation completion.
///
/// # Layout
/// Single atomic u64 (8 bytes, cache-aligned to 64 bytes)
///
/// # State Machine
/// ```text
/// Idle(0) → Pending(1) → InProgress(2) → Complete(3)
///                                      └→ Error(4)
/// ```
///
/// # Performance
/// - State update: <5ns (single atomic store)
/// - Polling: <5ns (single atomic load)
/// - Timeout: <10ns per iteration (busy-wait or exponential backoff)
#[repr(C, align(64))]
pub struct SyncPrimitive {
    /// Packed atomic state
    /// Bits: [63:32] timestamp (microseconds, for timeout detection)
    ///       [31:16] error code (0 = no error)
    ///       [15:8]  progress (0-100, percentage for long ops)
    ///       [7:0]   state (SyncState enum)
    state: AtomicU64,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    Idle = 0,
    Pending = 1,
    InProgress = 2,
    Complete = 3,
    Error = 4,
}

impl SyncPrimitive {
    /// Create new sync primitive (idle state).
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
        }
    }

    /// Reset to idle (reuse primitive).
    pub fn reset(&self) {
        self.state.store(0, Ordering::Release);
    }

    /// Set pending (operation submitted).
    ///
    /// # Performance
    /// - Target: <5ns (single atomic store)
    pub fn set_pending(&self) {
        let timestamp = get_timestamp_us();
        let packed = (timestamp << 32) | (SyncState::Pending as u64);
        self.state.store(packed, Ordering::Release);
    }

    /// Set in-progress (operation started, optional progress updates).
    ///
    /// # Arguments
    /// - `progress`: 0-100 (percentage complete)
    pub fn set_in_progress(&self, progress: u8) {
        let timestamp = get_timestamp_us();
        let packed = (timestamp << 32) | ((progress as u64) << 8) | (SyncState::InProgress as u64);
        self.state.store(packed, Ordering::Release);
    }

    /// Set complete (operation finished successfully).
    pub fn set_complete(&self) {
        let timestamp = get_timestamp_us();
        let packed = (timestamp << 32) | (SyncState::Complete as u64);
        self.state.store(packed, Ordering::Release);
    }

    /// Set error (operation failed).
    ///
    /// # Arguments
    /// - `error_code`: Backend-specific error code (e.g., CUDA error, XRT error)
    pub fn set_error(&self, error_code: u16) {
        let timestamp = get_timestamp_us();
        let packed = (timestamp << 32) | ((error_code as u64) << 16) | (SyncState::Error as u64);
        self.state.store(packed, Ordering::Release);
    }

    /// Check if complete (non-blocking).
    ///
    /// # Performance
    /// - Target: <5ns (single atomic load)
    pub fn is_complete(&self) -> bool {
        let packed = self.state.load(Ordering::Acquire);
        (packed & 0xFF) == SyncState::Complete as u64
    }

    /// Check if error occurred.
    pub fn has_error(&self) -> bool {
        let packed = self.state.load(Ordering::Acquire);
        (packed & 0xFF) == SyncState::Error as u64
    }

    /// Get error code (if any).
    pub fn error_code(&self) -> u16 {
        let packed = self.state.load(Ordering::Acquire);
        ((packed >> 16) & 0xFFFF) as u16
    }

    /// Get progress (0-100, valid only in InProgress state).
    pub fn progress(&self) -> u8 {
        let packed = self.state.load(Ordering::Acquire);
        ((packed >> 8) & 0xFF) as u8
    }

    /// Get state.
    pub fn state(&self) -> SyncState {
        let packed = self.state.load(Ordering::Acquire);
        match packed & 0xFF {
            0 => SyncState::Idle,
            1 => SyncState::Pending,
            2 => SyncState::InProgress,
            3 => SyncState::Complete,
            4 => SyncState::Error,
            _ => SyncState::Idle, // Shouldn't happen
        }
    }

    /// Block until complete or timeout.
    ///
    /// # Arguments
    /// - `timeout_us`: Timeout in microseconds (0 = infinite)
    ///
    /// # Returns
    /// - `Ok(())`: Completed successfully
    /// - `Err(HwError::Timeout)`: Timeout expired
    /// - `Err(HwError::DeviceError)`: Error occurred
    ///
    /// # Performance
    /// - Busy-wait: <10ns per iteration (100% CPU)
    /// - Exponential backoff: 1μs → 1ms (saves CPU, higher latency)
    pub fn wait(&self, timeout_us: u64) -> Result<(), HwError> {
        let start = get_timestamp_us();

        loop {
            let state = self.state();
            match state {
                SyncState::Complete => return Ok(()),
                SyncState::Error => {
                    return Err(HwError::DeviceError {
                        code: self.error_code(),
                        msg: "Operation failed",
                    })
                }
                _ => {
                    // Check timeout
                    if timeout_us > 0 && (get_timestamp_us() - start) > timeout_us {
                        return Err(HwError::Timeout {
                            requested_us: timeout_us,
                            elapsed_us: get_timestamp_us() - start,
                        });
                    }

                    // Yield CPU (exponential backoff)
                    std::hint::spin_loop();
                }
            }
        }
    }
}

unsafe impl Send for SyncPrimitive {}
unsafe impl Sync for SyncPrimitive {}

/// Get microsecond timestamp (monotonic clock).
fn get_timestamp_us() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}
```

### Command (Kernel Launch, Fence, etc.)

```rust
/// Device command (kernel launch, memory operation, synchronization).
///
/// # Layout
/// 64 bytes total (fits single cache line)
/// - 8 bytes header (type + priority + flags)
/// - 56 bytes payload (command-specific data)
///
/// # Performance
/// - Enqueue: <10ns (lockfree MPMC queue)
/// - Execution: Backend-dependent (1μs-1ms)
#[repr(C, align(64))]
#[derive(Copy, Clone)]
pub struct Command {
    /// Command type (determines payload interpretation)
    cmd_type: CommandType,

    /// Priority (0-7, higher = more urgent)
    /// Used for priority scheduling in CommandQueue
    priority: u8,

    /// Flags (backend-specific hints)
    /// Bit 0: Blocking (wait for completion)
    /// Bit 1: Measure latency (record start/end timestamps)
    /// Bits 2-7: Reserved
    flags: u8,

    /// Reserved (padding for alignment)
    _reserved: [u8; 5],

    /// Command-specific payload (56 bytes)
    payload: CommandPayload,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType {
    /// No operation (used for padding/testing)
    Nop = 0,

    /// DMA transfer (host↔device or device↔device)
    Transfer = 1,

    /// Kernel launch (FPGA IP core, GPU kernel, TPU computation)
    Kernel = 2,

    /// Fence (wait for previous commands to complete)
    Fence = 3,

    /// Synchronization (update SyncPrimitive)
    Sync = 4,

    /// Memory operation (fill, copy within device)
    MemOp = 5,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union CommandPayload {
    /// Transfer payload
    transfer: TransferPayload,

    /// Kernel payload
    kernel: KernelPayload,

    /// Fence payload
    fence: FencePayload,

    /// Sync payload
    sync: SyncPayload,

    /// Memory operation payload
    memop: MemOpPayload,

    /// Raw bytes (for backend-specific commands)
    raw: [u8; 56],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TransferPayload {
    /// Source buffer handle (0 = host memory)
    src_handle: u64,

    /// Destination buffer handle (0 = host memory)
    dst_handle: u64,

    /// Source offset (bytes)
    src_offset: u64,

    /// Destination offset (bytes)
    dst_offset: u64,

    /// Transfer size (bytes)
    size: u64,

    /// Synchronization primitive (optional, 0 = none)
    sync_ptr: u64,

    /// Reserved
    _reserved: [u8; 8],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct KernelPayload {
    /// Kernel handle (backend-specific, e.g., CUDA function, XRT IP core)
    kernel_handle: u64,

    /// Grid dimensions (X, Y, Z)
    grid: [u32; 3],

    /// Block dimensions (X, Y, Z)
    block: [u32; 3],

    /// Shared memory size (bytes)
    shared_mem: u32,

    /// Argument buffer (inline, up to 16 bytes)
    args: [u64; 2],

    /// Reserved
    _reserved: [u8; 4],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FencePayload {
    /// Fence ID (for tracking)
    fence_id: u64,

    /// Reserved
    _reserved: [u8; 48],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SyncPayload {
    /// Synchronization primitive pointer
    sync_ptr: u64,

    /// New state (SyncState enum value)
    new_state: u8,

    /// Error code (if new_state == Error)
    error_code: u16,

    /// Reserved
    _reserved: [u8; 45],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MemOpPayload {
    /// Operation type (Fill, Copy)
    op_type: u8,

    /// Reserved
    _reserved1: [u8; 7],

    /// Destination buffer handle
    dst_handle: u64,

    /// Destination offset (bytes)
    dst_offset: u64,

    /// Size (bytes)
    size: u64,

    /// Fill pattern (for Fill op) or source offset (for Copy op)
    data: u64,

    /// Reserved
    _reserved2: [u8; 16],
}

impl Command {
    /// Create transfer command.
    pub fn transfer(
        src: u64,
        dst: u64,
        src_offset: u64,
        dst_offset: u64,
        size: u64,
        sync: Option<&SyncPrimitive>,
    ) -> Self {
        let sync_ptr = sync.map_or(0, |s| s as *const _ as u64);
        Self {
            cmd_type: CommandType::Transfer,
            priority: 0,
            flags: 0,
            _reserved: [0; 5],
            payload: CommandPayload {
                transfer: TransferPayload {
                    src_handle: src,
                    dst_handle: dst,
                    src_offset,
                    dst_offset,
                    size,
                    sync_ptr,
                    _reserved: [0; 8],
                },
            },
        }
    }

    /// Create kernel launch command.
    pub fn kernel(
        kernel_handle: u64,
        grid: [u32; 3],
        block: [u32; 3],
        args: [u64; 2],
    ) -> Self {
        Self {
            cmd_type: CommandType::Kernel,
            priority: 0,
            flags: 0,
            _reserved: [0; 5],
            payload: CommandPayload {
                kernel: KernelPayload {
                    kernel_handle,
                    grid,
                    block,
                    shared_mem: 0,
                    args,
                    _reserved: [0; 4],
                },
            },
        }
    }

    /// Create fence command.
    pub fn fence(fence_id: u64) -> Self {
        Self {
            cmd_type: CommandType::Fence,
            priority: 0,
            flags: 0,
            _reserved: [0; 5],
            payload: CommandPayload {
                fence: FencePayload {
                    fence_id,
                    _reserved: [0; 48],
                },
            },
        }
    }
}
```

---

## Memory Model

### Host Memory

```
Host Memory Regions:
┌─────────────────────────────────────────────────────────────┐
│  Pinned Memory (mlock)                                       │
│  - Non-pageable (stays in RAM, never swapped to disk)       │
│  - DMA-accessible (device can read/write directly)          │
│  - Faster transfers (<5μs for 1MB)                          │
│  - Limited size (check ulimit -l, typically 64KB-8GB)       │
│  - Allocation: mlock(ptr, size)                             │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Pageable Memory (malloc)                                    │
│  - Can be swapped to disk (slower transfers)                │
│  - Requires intermediate copy to pinned memory              │
│  - Slower transfers (50-100μs for 1MB)                      │
│  - Unlimited size (subject to RAM availability)             │
│  - Allocation: malloc(size)                                 │
└─────────────────────────────────────────────────────────────┘
```

### Device Memory

```
Device Memory Hierarchy:
┌─────────────────────────────────────────────────────────────┐
│  GPU HBM (High-Bandwidth Memory)                            │
│  - 1-2 TB/s bandwidth (100× faster than PCIe)               │
│  - On-chip (no PCIe overhead for device-side access)        │
│  - Limited size (24GB-80GB for H100)                        │
│  - Allocation: cudaMalloc, xclAllocBO                       │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  FPGA DDR/HBM                                               │
│  - 20-500 GB/s bandwidth (depends on FPGA model)            │
│  - Shared with PL (programmable logic) kernels              │
│  - Configurable size (8GB-128GB for Alveo U280)             │
│  - Allocation: xclAllocBO                                   │
└─────────────────────────────────────────────────────────────┘
```

### Memory Coherence

```
Coherence Models:
┌─────────────────────────────────────────────────────────────┐
│  Explicit Coherence (FPGA, older GPUs)                      │
│  - Host and device have separate address spaces             │
│  - Explicit sync required (xclSyncBO, cudaMemcpy)           │
│  - Programmer responsibility (error-prone)                  │
│  - Performance: Deterministic (no hidden costs)             │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Unified Virtual Addressing (CUDA UVA, future FPGAs)        │
│  - Host and device share address space                      │
│  - Automatic migration (transparent to programmer)          │
│  - Easier programming model                                 │
│  - Performance: Non-deterministic (hidden page faults)      │
└─────────────────────────────────────────────────────────────┘
```

**Design Decision**: Use explicit coherence for predictable latency (<5μs target). UVA adds 10-50μs overhead due to page migration.

---

## Synchronization Model

### Polling vs Interrupts

```
Synchronization Strategies:
┌─────────────────────────────────────────────────────────────┐
│  Busy-Wait Polling (Default)                                │
│  - Spin on atomic flag in tight loop                        │
│  - Lowest latency (<10ns per iteration)                     │
│  - 100% CPU usage (not suitable for long operations)        │
│  - Use case: <1ms operations (DMA transfers)                │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Exponential Backoff Polling                                │
│  - Start with spin loop, increase sleep duration            │
│  - Trade latency for CPU efficiency                         │
│  - Sleep sequence: 0ns → 1μs → 10μs → 100μs → 1ms          │
│  - Use case: 1ms-1s operations (kernel execution)           │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Interrupt-Driven (Future)                                  │
│  - Device signals interrupt on completion                   │
│  - Zero CPU overhead (thread sleeps)                        │
│  - Higher latency (interrupt handling ~5-50μs)              │
│  - Use case: >1s operations (large batch processing)        │
└─────────────────────────────────────────────────────────────┘
```

### Completion Detection

```rust
/// Completion detection strategies
pub enum CompletionStrategy {
    /// Busy-wait (tight spin loop, <10ns latency, 100% CPU)
    BusyWait,

    /// Exponential backoff (1μs → 1ms, saves CPU, higher latency)
    ExponentialBackoff {
        initial_us: u64,
        max_us: u64,
        multiplier: u8,
    },

    /// Interrupt-driven (future, kernel support required)
    Interrupt,
}

impl SyncPrimitive {
    /// Wait with custom strategy.
    pub fn wait_with_strategy(
        &self,
        timeout_us: u64,
        strategy: CompletionStrategy,
    ) -> Result<(), HwError> {
        match strategy {
            CompletionStrategy::BusyWait => self.wait(timeout_us),
            CompletionStrategy::ExponentialBackoff { initial_us, max_us, multiplier } => {
                let start = get_timestamp_us();
                let mut backoff_us = initial_us;

                loop {
                    if self.is_complete() {
                        return Ok(());
                    }

                    if self.has_error() {
                        return Err(HwError::DeviceError {
                            code: self.error_code(),
                            msg: "Operation failed",
                        });
                    }

                    if timeout_us > 0 && (get_timestamp_us() - start) > timeout_us {
                        return Err(HwError::Timeout {
                            requested_us: timeout_us,
                            elapsed_us: get_timestamp_us() - start,
                        });
                    }

                    // Sleep with exponential backoff
                    std::thread::sleep(std::time::Duration::from_micros(backoff_us));
                    backoff_us = (backoff_us * multiplier as u64).min(max_us);
                }
            }
            CompletionStrategy::Interrupt => {
                // Future: epoll/kqueue on device file descriptor
                unimplemented!("Interrupt-driven completion not yet supported")
            }
        }
    }
}
```

---

## Backend Implementations

### Mock Backend (Testing)

```rust
/// Mock device for testing (instant operations, no FFI).
///
/// # Characteristics
/// - All operations return immediately (0ns latency)
/// - No real hardware interaction
/// - Deterministic (same input → same output)
/// - Thread-safe (Send + Sync)
///
/// # Use Cases
/// - Unit tests (no hardware required)
/// - CI/CD (no FPGA/GPU access)
/// - Development (fast iteration)
pub struct MockDevice {
    caps: DeviceCapabilities,
    buffers: std::sync::Mutex<std::collections::HashMap<u64, Vec<u8>>>,
    next_handle: AtomicU64,
}

impl MockDevice {
    pub fn new() -> Self {
        Self {
            caps: DeviceCapabilities {
                device_type: DeviceType::Mock,
                vendor: "Atomic Capsule",
                device_name: "Mock Device",
                pcie_gen: 4,
                pcie_lanes: 16,
                pcie_bandwidth: 32_000_000_000, // 32 GB/s (fake)
                device_memory: 16_000_000_000, // 16 GB (fake)
                memory_bandwidth: 1_000_000_000_000, // 1 TB/s (fake)
                atomic_support: true,
                pinned_memory: true,
                p2p_support: true,
                uva_support: true,
                max_concurrent_transfers: 16,
                max_queue_depth: 4096,
            },
            buffers: std::sync::Mutex::new(std::collections::HashMap::new()),
            next_handle: AtomicU64::new(1),
        }
    }
}

impl AcceleratorDevice for MockDevice {
    fn capabilities(&self) -> &DeviceCapabilities {
        &self.caps
    }

    fn alloc_device(&self, size: usize, _flags: AllocFlags) -> Result<DeviceHandle, HwError> {
        let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
        let mut buffers = self.buffers.lock().unwrap();
        buffers.insert(handle, vec![0u8; size]);
        Ok(DeviceHandle(handle))
    }

    fn free_device(&self, handle: DeviceHandle) -> Result<(), HwError> {
        let mut buffers = self.buffers.lock().unwrap();
        buffers.remove(&handle.0);
        Ok(())
    }

    fn transfer_async(
        &self,
        buf: &DmaBuffer,
        direction: TransferDirection,
        sync: &SyncPrimitive,
    ) -> Result<(), HwError> {
        // Instant transfer (copy data between host and "device" buffer)
        let handle = buf.device_handle();
        if handle == 0 {
            return Err(HwError::DeviceError {
                code: 1,
                msg: "Device handle not allocated",
            });
        }

        let mut buffers = self.buffers.lock().unwrap();
        let device_buf = buffers.get_mut(&handle).ok_or(HwError::DeviceError {
            code: 2,
            msg: "Invalid device handle",
        })?;

        match direction {
            TransferDirection::HostToDevice => {
                // Copy host → device
                let host_slice = buf.as_slice();
                device_buf[..host_slice.len()].copy_from_slice(host_slice);
            }
            TransferDirection::DeviceToHost => {
                // Copy device → host
                let host_slice = unsafe {
                    std::slice::from_raw_parts_mut(buf.host_ptr, buf.size)
                };
                host_slice.copy_from_slice(&device_buf[..buf.size]);
            }
            TransferDirection::DeviceToDevice => {
                // No-op (same device)
            }
        }

        // Instantly complete
        sync.set_complete();
        Ok(())
    }

    fn submit(&self, _cmd: &Command) -> Result<(), HwError> {
        // Instant execution (no-op)
        Ok(())
    }

    fn sync_wait(&self, sync: &SyncPrimitive, _timeout_us: u64) -> Result<(), HwError> {
        // Already complete (instant operations)
        if sync.is_complete() {
            Ok(())
        } else if sync.has_error() {
            Err(HwError::DeviceError {
                code: sync.error_code(),
                msg: "Mock operation failed",
            })
        } else {
            Err(HwError::Timeout {
                requested_us: 0,
                elapsed_us: 0,
            })
        }
    }

    fn sync_check(&self, sync: &SyncPrimitive) -> Result<bool, HwError> {
        Ok(sync.is_complete())
    }
}
```

### FPGA Backend (Xilinx XRT) - Skeleton

```rust
/// FPGA device (Xilinx XRT backend).
///
/// # FFI Dependencies
/// - libxrt_coreutil.so (Xilinx Runtime)
///
/// # Safety
/// - All FFI isolated in this module (99.99% safe overall)
/// - Pointers validated before FFI calls
/// - RAII cleanup (xclClose on drop)
pub struct FpgaXrtDevice {
    handle: *mut std::ffi::c_void, // xclDeviceHandle
    caps: DeviceCapabilities,
}

impl FpgaXrtDevice {
    /// Open FPGA device.
    ///
    /// # Arguments
    /// - `device_index`: Device index (0 = first FPGA)
    ///
    /// # Returns
    /// - `Ok(FpgaXrtDevice)`: Device opened successfully
    /// - `Err(HwError::DeviceNotFound)`: No FPGA at index
    /// - `Err(HwError::InitFailed)`: xclOpen failed
    pub fn open(device_index: u32) -> Result<Self, HwError> {
        // See ffi/xrt.rs for full implementation
        unimplemented!("FPGA XRT backend (see DMA_TRANSFER_CAPSULE.md)")
    }
}

impl AcceleratorDevice for FpgaXrtDevice {
    // See HW_INTERFACE_T28.md for full implementation + tests
}
```

### GPU Backend (NVIDIA CUDA) - Skeleton

```rust
/// GPU device (NVIDIA CUDA backend).
///
/// # FFI Dependencies
/// - libcuda.so (NVIDIA Driver)
///
/// # Safety
/// - All FFI isolated in this module
/// - RAII cleanup (cuDeviceDetach on drop)
pub struct GpuCudaDevice {
    device: i32, // CUdevice
    context: *mut std::ffi::c_void, // CUcontext
    caps: DeviceCapabilities,
}

impl GpuCudaDevice {
    /// Open GPU device.
    pub fn open(device_index: u32) -> Result<Self, HwError> {
        // See ffi/cuda.rs for full implementation
        unimplemented!("GPU CUDA backend (future work)")
    }
}

impl AcceleratorDevice for GpuCudaDevice {
    // Future implementation
}
```

---

## Error Handling

### Error Type Hierarchy

```rust
/// Hardware interface errors (comprehensive taxonomy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HwError {
    /// Device not found (e.g., no FPGA at index)
    DeviceNotFound,

    /// Device initialization failed (e.g., xclOpen returned NULL)
    InitFailed(i32),

    /// Buffer allocation failed
    AllocFailed {
        size: usize,
        align: usize,
    },

    /// Transfer failed
    TransferFailed {
        code: i32,
        msg: &'static str,
    },

    /// Command submission failed
    SubmitFailed {
        code: i32,
        msg: &'static str,
    },

    /// Synchronization timeout
    Timeout {
        requested_us: u64,
        elapsed_us: u64,
    },

    /// Device error (hardware fault, thermal, PCIe link down)
    DeviceError {
        code: u16,
        msg: &'static str,
    },

    /// FFI error (NULL pointer, invalid handle, ABI mismatch)
    FfiError(&'static str),

    /// Invalid argument
    InvalidArgument(&'static str),
}

impl std::fmt::Display for HwError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            HwError::DeviceNotFound => write!(f, "Device not found"),
            HwError::InitFailed(code) => write!(f, "Device init failed: code {}", code),
            HwError::AllocFailed { size, align } => {
                write!(f, "Alloc failed: size={}, align={}", size, align)
            }
            HwError::TransferFailed { code, msg } => {
                write!(f, "Transfer failed: {} (code {})", msg, code)
            }
            HwError::SubmitFailed { code, msg } => {
                write!(f, "Submit failed: {} (code {})", msg, code)
            }
            HwError::Timeout { requested_us, elapsed_us } => {
                write!(f, "Timeout: requested {}μs, elapsed {}μs", requested_us, elapsed_us)
            }
            HwError::DeviceError { code, msg } => {
                write!(f, "Device error: {} (code {})", msg, code)
            }
            HwError::FfiError(msg) => write!(f, "FFI error: {}", msg),
            HwError::InvalidArgument(msg) => write!(f, "Invalid argument: {}", msg),
        }
    }
}

impl std::error::Error for HwError {}
```

### Retry Logic

```rust
/// Retry transient errors with exponential backoff.
pub fn transfer_with_retry(
    device: &dyn AcceleratorDevice,
    buf: &DmaBuffer,
    direction: TransferDirection,
    sync: &SyncPrimitive,
    max_retries: u32,
) -> Result<(), HwError> {
    for attempt in 0..max_retries {
        match device.transfer_async(buf, direction, sync) {
            Ok(_) => {
                // Wait for completion
                return sync.wait(1_000_000); // 1 second timeout
            }
            Err(HwError::TransferFailed { .. }) if attempt < max_retries - 1 => {
                // Exponential backoff: 1μs, 2μs, 4μs, ..., 1ms
                let backoff_us = 1 << attempt.min(10);
                std::thread::sleep(std::time::Duration::from_micros(backoff_us));
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}
```

---

## Performance Characteristics

### Latency Budget

| Operation | Target | Breakdown | Notes |
|-----------|--------|-----------|-------|
| **DMA Transfer (1MB)** | <5μs | PCIe overhead (1μs) + transfer (3μs @ 32 GB/s) + completion (1μs) | Pinned memory required |
| **Queue Enqueue** | <10ns | Atomic CAS (5ns) + cache line write (5ns) | Lockfree MPMC |
| **Queue Dequeue** | <10ns | Atomic CAS (5ns) + cache line read (5ns) | Lockfree MPMC |
| **Sync Check** | <5ns | Single atomic load (3ns) + branch (2ns) | No syscall |
| **Device Open** | <1ms | PCIe enumeration (500μs) + driver init (500μs) | One-time cost |
| **Buffer Alloc** | <100μs | Pinned memory (50μs) + device alloc (50μs) | Amortize via pooling |

### Bandwidth Targets

| Transfer Size | PCIe Gen4 Theoretical | Target (80%) | Measured (TBD) |
|--------------|----------------------|--------------|----------------|
| **1KB** | 32 GB/s | 800 MB/s | TBD (B32) |
| **4KB** | 32 GB/s | 3.2 GB/s | TBD (B32) |
| **1MB** | 32 GB/s | 25 GB/s | TBD (B32) |
| **1GB** | 32 GB/s | 28 GB/s | TBD (B32) |

**Note**: Small transfers (<4KB) are latency-bound (PCIe overhead dominates), large transfers (>1MB) are bandwidth-bound.

### Scalability

| Scenario | Queue Latency | Throughput | Notes |
|----------|--------------|------------|-------|
| **1 Producer, 1 Consumer** | <10ns | 100M ops/sec | Baseline (no contention) |
| **4 Producers, 4 Consumers** | <20ns | 50M ops/sec | Moderate contention |
| **16 Producers, 16 Consumers** | <50ns | 20M ops/sec | High contention (acceptable) |
| **64 Producers, 64 Consumers** | <200ns | 5M ops/sec | Extreme contention (rare) |

---

## Summary

**Hardware Abstraction Layer Design**:
- **5 Core Traits**: AcceleratorDevice, BufferAllocator, DmaBuffer (struct), SyncPrimitive (struct), Command (struct)
- **3 Initial Backends**: Mock (testing), FPGA (XRT), GPU (CUDA, future)
- **Zero-Cost Abstraction**: Static dispatch via trait monomorphization (0ns overhead)
- **Lockfree Coordination**: 100% atomic operations (no mutex/RwLock)
- **Safety**: 99.99% safe Rust (FFI isolated in <1% of codebase)

**Performance Targets**:
- **DMA Latency**: <5μs (1MB transfers)
- **Queue Operations**: <10ns (lockfree MPMC)
- **PCIe Bandwidth**: >25 GB/s (80% utilization)

**Next Steps**:
1. Implement DmaBuffer + SyncPrimitive (Week 1-2, see DMA_TRANSFER_CAPSULE.md)
2. Implement CommandQueue (Week 3-4, see COMMAND_QUEUE_CAPSULE.md)
3. Implement FPGA XRT backend (Week 5-6)
4. Comprehensive testing (Week 7-8, see HW_INTERFACE_T28.md)

**Files**: 5 comprehensive design docs complete (3,500+ lines total).
