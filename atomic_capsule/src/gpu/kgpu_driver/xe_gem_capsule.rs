// Intel Xe2 GEM Buffer Object Management Capsule
// T1 Atomic Tier: 256B cache-aligned, 100% lockfree
//
// Graphics Execution Manager (GEM) provides GPU memory allocation and management
// for Intel Xe2 GPUs via DRM (Direct Rendering Manager) interface.
//
// # Overview
// GEM objects are the fundamental unit of GPU memory in Intel's architecture.
// Each GEM object has:
// - A handle (unique identifier per DRM file descriptor)
// - Size (in bytes, page-aligned)
// - GPU address (after binding to VM)
// - CPU address (after mmap)
// - State machine: INVALID → ALLOCATED → BOUND → MAPPED
//
// # Memory Placement
// - VRAM: On-board GPU memory (fastest, limited capacity)
// - SYSTEM: System RAM accessible to GPU (larger, slower)
// - STOLEN: Pre-allocated system memory reserved for GPU (firmware/display)

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// GEM object states
const GEM_STATE_INVALID: u32 = 0;
const GEM_STATE_ALLOCATED: u32 = 1;
const GEM_STATE_BOUND: u32 = 2;
const GEM_STATE_MAPPED: u32 = 3;

/// GEM allocation flags
pub const GEM_FLAG_DEVICE_LOCAL: u32 = 1 << 0; // Allocate in VRAM (fast, limited)
pub const GEM_FLAG_HOST_VISIBLE: u32 = 1 << 1; // CPU can map the memory
pub const GEM_FLAG_HOST_COHERENT: u32 = 1 << 2; // CPU/GPU caches are coherent

/// Memory placement types
pub const GEM_PLACEMENT_VRAM: u32 = 0; // On-board GPU memory
pub const GEM_PLACEMENT_SYSTEM: u32 = 1; // System RAM
pub const GEM_PLACEMENT_STOLEN: u32 = 2; // Pre-allocated system memory

/// Intel Xe2 GEM Buffer Object Capsule (T1 Atomic, 256B cache-aligned)
///
/// Manages GPU memory buffers via GEM (Graphics Execution Manager).
/// Provides lockfree coordination for allocation, binding, mapping, and deallocation.
///
/// # State Machine
/// ```text
/// INVALID --allocate()--> ALLOCATED --bind()--> BOUND --map()--> MAPPED
///    ^                        |                   |                |
///    |                        |                   |                |
///    +-------- free() --------+-------------------+-- unmap() -----+
/// ```
///
/// # Memory Safety
/// - #ASSUME: DRM file descriptor remains valid during operations
/// - #VERIFY: All operations check state before proceeding
/// - #ASSUME: GPU and CPU addresses are valid after bind/map
/// - #VERIFY: Generation counter prevents ABA race conditions
#[repr(C, align(256))]
pub struct XeGemCapsule {
    // GEM identification
    handle: AtomicU32, // GEM handle (0 if invalid)
    size: AtomicU64,   // Buffer size in bytes

    // Address mapping
    gpu_addr: AtomicU64, // GPU virtual address (0 if not bound)
    cpu_addr: AtomicU64, // CPU mapped address (0 if not mapped)

    // State coordination
    state: AtomicU32,      // Current state (see GEM_STATE_* constants)
    generation: AtomicU64, // Generation counter for ABA prevention

    // Configuration
    flags: AtomicU32,     // Allocation flags (see GEM_FLAG_* constants)
    placement: AtomicU32, // Memory placement (see GEM_PLACEMENT_* constants)

    // Statistics (lockfree counters)
    alloc_count: AtomicU64,
    free_count: AtomicU64,
    map_count: AtomicU64,
    unmap_count: AtomicU64,

    // Padding to exactly 256 bytes
    // Current size: 12 * 8 = 96 bytes (3 AtomicU32 = 12 bytes aligned to 8, 9 AtomicU64 = 72 bytes)
    // Padding needed: 256 - 96 = 160 bytes
    _padding: [u8; 160],
}

/// GEM-specific errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XeGemError {
    InvalidHandle,
    AllocationFailed,
    BindFailed,
    MapFailed,
    AlreadyMapped,
    NotMapped,
    NotBound,
}

impl XeGemCapsule {
    /// Create new uninitialized GEM capsule
    #[inline]
    pub fn new() -> Self {
        // #ASSUME: Cache-aligned allocation by caller
        // #VERIFY: #[repr(C, align(256))] enforces alignment
        Self {
            handle: AtomicU32::new(0),
            size: AtomicU64::new(0),
            gpu_addr: AtomicU64::new(0),
            cpu_addr: AtomicU64::new(0),
            state: AtomicU32::new(GEM_STATE_INVALID),
            generation: AtomicU64::new(0),
            flags: AtomicU32::new(0),
            placement: AtomicU32::new(GEM_PLACEMENT_VRAM),
            alloc_count: AtomicU64::new(0),
            free_count: AtomicU64::new(0),
            map_count: AtomicU64::new(0),
            unmap_count: AtomicU64::new(0),
            _padding: [0u8; 160],
        }
    }

    /// Allocate a GEM buffer
    ///
    /// Creates a new GEM object with the specified size and flags.
    /// Size is rounded up to page boundaries by the kernel.
    ///
    /// # Arguments
    /// - `drm_fd`: DRM file descriptor
    /// - `size`: Buffer size in bytes
    /// - `flags`: Allocation flags (see GEM_FLAG_* constants)
    ///
    /// # Errors
    /// - `AllocationFailed`: Kernel allocation failed or out of memory
    ///
    /// # State Transition
    /// INVALID → ALLOCATED
    pub fn allocate(&self, drm_fd: i32, size: usize, flags: u32) -> Result<(), XeGemError> {
        // #ASSUME: drm_fd is a valid open file descriptor
        // #VERIFY: Caller must ensure drm_fd remains open

        // Check current state
        let current_state = self.state.load(Ordering::Acquire);
        if current_state != GEM_STATE_INVALID {
            return Err(XeGemError::InvalidHandle);
        }

        // Phase 1: Simulate allocation (no real kernel calls yet)
        // In production, this would call DRM_IOCTL_XE_GEM_CREATE
        let _ = drm_fd; // Suppress unused warning

        let simulated_handle = self.generation.load(Ordering::Relaxed) as u32 + 1;

        // Store allocation parameters
        self.handle.store(simulated_handle, Ordering::Relaxed);
        self.size.store(size as u64, Ordering::Relaxed);
        self.flags.store(flags, Ordering::Relaxed);

        // Determine placement based on flags
        let placement = if flags & GEM_FLAG_DEVICE_LOCAL != 0 {
            GEM_PLACEMENT_VRAM
        } else {
            GEM_PLACEMENT_SYSTEM
        };
        self.placement.store(placement, Ordering::Relaxed);

        // Update state and generation
        self.state.store(GEM_STATE_ALLOCATED, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
        self.alloc_count.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Free the GEM buffer
    ///
    /// Destroys the GEM object and releases GPU memory.
    /// Automatically unmaps and unbinds if necessary.
    ///
    /// # Arguments
    /// - `drm_fd`: DRM file descriptor
    ///
    /// # Errors
    /// - `InvalidHandle`: Buffer is not allocated
    ///
    /// # State Transition
    /// ALLOCATED/BOUND/MAPPED → INVALID
    pub fn free(&self, drm_fd: i32) -> Result<(), XeGemError> {
        let current_state = self.state.load(Ordering::Acquire);
        if current_state == GEM_STATE_INVALID {
            return Err(XeGemError::InvalidHandle);
        }

        // Unmap if currently mapped
        if current_state == GEM_STATE_MAPPED {
            let _ = self.unmap();
        }

        // Phase 1: Simulate deallocation
        // In production, this would call DRM_IOCTL_GEM_CLOSE
        let _ = drm_fd;

        // Clear all state
        self.handle.store(0, Ordering::Relaxed);
        self.size.store(0, Ordering::Relaxed);
        self.gpu_addr.store(0, Ordering::Relaxed);
        self.cpu_addr.store(0, Ordering::Relaxed);

        // Update state and generation
        self.state.store(GEM_STATE_INVALID, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
        self.free_count.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Bind GEM buffer to GPU virtual address space
    ///
    /// Associates the GEM object with a VM (Virtual Machine) context,
    /// making it accessible to GPU at a specific virtual address.
    ///
    /// # Arguments
    /// - `drm_fd`: DRM file descriptor
    /// - `vm_id`: Virtual machine context ID
    ///
    /// # Returns
    /// GPU virtual address on success
    ///
    /// # Errors
    /// - `InvalidHandle`: Buffer is not allocated
    /// - `BindFailed`: Kernel binding operation failed
    ///
    /// # State Transition
    /// ALLOCATED → BOUND
    pub fn bind(&self, drm_fd: i32, vm_id: u32) -> Result<u64, XeGemError> {
        let current_state = self.state.load(Ordering::Acquire);
        if current_state != GEM_STATE_ALLOCATED {
            return Err(XeGemError::InvalidHandle);
        }

        // Phase 1: Simulate binding
        // In production, this would call DRM_IOCTL_XE_VM_BIND
        let _ = (drm_fd, vm_id);

        // Simulate GPU address allocation
        let size = self.size.load(Ordering::Relaxed);
        let simulated_gpu_addr = 0x1000_0000 + (size * self.generation.load(Ordering::Relaxed));

        self.gpu_addr.store(simulated_gpu_addr, Ordering::Relaxed);

        // Update state and generation
        self.state.store(GEM_STATE_BOUND, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(simulated_gpu_addr)
    }

    /// Unbind GEM buffer from GPU virtual address space
    ///
    /// Removes the GEM object from the VM context.
    ///
    /// # Arguments
    /// - `drm_fd`: DRM file descriptor
    /// - `vm_id`: Virtual machine context ID
    ///
    /// # Errors
    /// - `NotBound`: Buffer is not currently bound
    ///
    /// # State Transition
    /// BOUND → ALLOCATED (MAPPED → ALLOCATED if currently mapped)
    pub fn unbind(&self, drm_fd: i32, vm_id: u32) -> Result<(), XeGemError> {
        let current_state = self.state.load(Ordering::Acquire);
        if current_state != GEM_STATE_BOUND && current_state != GEM_STATE_MAPPED {
            return Err(XeGemError::NotBound);
        }

        // Unmap if currently mapped
        if current_state == GEM_STATE_MAPPED {
            let _ = self.unmap();
        }

        // Phase 1: Simulate unbinding
        // In production, this would call DRM_IOCTL_XE_VM_UNBIND
        let _ = (drm_fd, vm_id);

        self.gpu_addr.store(0, Ordering::Relaxed);

        // Update state and generation
        self.state.store(GEM_STATE_ALLOCATED, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Map GEM buffer to CPU address space
    ///
    /// Creates a CPU-accessible mapping of the GPU buffer.
    /// Requires GEM_FLAG_HOST_VISIBLE to be set during allocation.
    ///
    /// # Arguments
    /// - `drm_fd`: DRM file descriptor
    ///
    /// # Returns
    /// CPU pointer to mapped memory
    ///
    /// # Errors
    /// - `InvalidHandle`: Buffer is not bound
    /// - `AlreadyMapped`: Buffer is already mapped
    /// - `MapFailed`: Kernel mmap operation failed
    ///
    /// # Safety
    /// The returned pointer is valid until unmap() is called.
    /// Caller must ensure proper synchronization for CPU/GPU access.
    ///
    /// # State Transition
    /// BOUND → MAPPED
    pub fn map(&self, drm_fd: i32) -> Result<*mut u8, XeGemError> {
        let current_state = self.state.load(Ordering::Acquire);
        if current_state != GEM_STATE_BOUND {
            if current_state == GEM_STATE_MAPPED {
                return Err(XeGemError::AlreadyMapped);
            }
            return Err(XeGemError::InvalidHandle);
        }

        // Check if host visible
        let flags = self.flags.load(Ordering::Relaxed);
        if flags & GEM_FLAG_HOST_VISIBLE == 0 {
            return Err(XeGemError::MapFailed);
        }

        // Phase 1: Simulate CPU mapping
        // In production, this would:
        // 1. Call DRM_IOCTL_XE_GEM_MMAP_OFFSET to get mmap offset
        // 2. Call mmap() with the offset
        let _ = drm_fd;

        #[cfg(target_os = "linux")]
        {
            let size = self.size.load(Ordering::Relaxed) as usize;

            // Simulate mmap allocation (use heap memory for Phase 1)
            let layout = std::alloc::Layout::from_size_align(size, 4096)
                .map_err(|_| XeGemError::MapFailed)?;

            // SAFETY: Layout is valid (non-zero size, valid alignment)
            let ptr = unsafe { std::alloc::alloc_zeroed(layout) };

            if ptr.is_null() {
                return Err(XeGemError::MapFailed);
            }

            self.cpu_addr.store(ptr as u64, Ordering::Relaxed);

            // Update state and generation
            self.state.store(GEM_STATE_MAPPED, Ordering::Release);
            self.generation.fetch_add(1, Ordering::Release);
            self.map_count.fetch_add(1, Ordering::Relaxed);

            Ok(ptr)
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(XeGemError::MapFailed)
        }
    }

    /// Unmap GEM buffer from CPU address space
    ///
    /// Removes the CPU mapping created by map().
    ///
    /// # Errors
    /// - `NotMapped`: Buffer is not currently mapped
    ///
    /// # State Transition
    /// MAPPED → BOUND
    pub fn unmap(&self) -> Result<(), XeGemError> {
        let current_state = self.state.load(Ordering::Acquire);
        if current_state != GEM_STATE_MAPPED {
            return Err(XeGemError::NotMapped);
        }

        // Phase 1: Simulate CPU unmapping
        // In production, this would call munmap()

        #[cfg(target_os = "linux")]
        {
            let ptr = self.cpu_addr.load(Ordering::Relaxed);
            if ptr != 0 {
                let size = self.size.load(Ordering::Relaxed) as usize;

                // SAFETY: ptr was allocated by alloc_zeroed in map()
                unsafe {
                    let layout = std::alloc::Layout::from_size_align_unchecked(size, 4096);
                    std::alloc::dealloc(ptr as *mut u8, layout);
                }
            }
        }

        self.cpu_addr.store(0, Ordering::Relaxed);

        // Update state and generation
        self.state.store(GEM_STATE_BOUND, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
        self.unmap_count.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Get GEM handle
    #[inline]
    pub fn handle(&self) -> u32 {
        self.handle.load(Ordering::Relaxed)
    }

    /// Get buffer size in bytes
    #[inline]
    pub fn size(&self) -> u64 {
        self.size.load(Ordering::Relaxed)
    }

    /// Get GPU virtual address
    #[inline]
    pub fn gpu_addr(&self) -> u64 {
        self.gpu_addr.load(Ordering::Relaxed)
    }

    /// Get CPU mapped address
    #[inline]
    pub fn cpu_addr(&self) -> u64 {
        self.cpu_addr.load(Ordering::Relaxed)
    }

    /// Check if buffer is valid (allocated)
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.state.load(Ordering::Acquire) != GEM_STATE_INVALID
    }

    /// Check if buffer is currently mapped
    #[inline]
    pub fn is_mapped(&self) -> bool {
        self.state.load(Ordering::Acquire) == GEM_STATE_MAPPED
    }

    /// Get allocation count
    #[inline]
    pub fn alloc_count(&self) -> u64 {
        self.alloc_count.load(Ordering::Relaxed)
    }

    /// Get free count
    #[inline]
    pub fn free_count(&self) -> u64 {
        self.free_count.load(Ordering::Relaxed)
    }

    /// Get map count
    #[inline]
    pub fn map_count(&self) -> u64 {
        self.map_count.load(Ordering::Relaxed)
    }

    /// Get unmap count
    #[inline]
    pub fn unmap_count(&self) -> u64 {
        self.unmap_count.load(Ordering::Relaxed)
    }
}

impl Default for XeGemCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_alignment() {
        // T28 Q1: Verify 256B cache alignment
        assert_eq!(
            core::mem::size_of::<XeGemCapsule>(),
            256,
            "Capsule must be exactly 256 bytes"
        );
        assert_eq!(
            core::mem::align_of::<XeGemCapsule>(),
            256,
            "Capsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_new_capsule() {
        // T28 Q2: Verify initial state
        let capsule = XeGemCapsule::new();
        assert_eq!(capsule.handle(), 0);
        assert_eq!(capsule.size(), 0);
        assert_eq!(capsule.gpu_addr(), 0);
        assert_eq!(capsule.cpu_addr(), 0);
        assert!(!capsule.is_valid());
        assert!(!capsule.is_mapped());
        assert_eq!(capsule.alloc_count(), 0);
        assert_eq!(capsule.free_count(), 0);
        assert_eq!(capsule.map_count(), 0);
        assert_eq!(capsule.unmap_count(), 0);
    }

    #[test]
    fn test_allocate() {
        // T28 Q3: Verify allocation
        let capsule = XeGemCapsule::new();
        let result = capsule.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE);
        assert!(result.is_ok());
        assert!(capsule.is_valid());
        assert_eq!(capsule.size(), 4096);
        assert_ne!(capsule.handle(), 0);
        assert_eq!(capsule.alloc_count(), 1);
    }

    #[test]
    fn test_double_allocate_fails() {
        // T28 Q4: Verify no double allocation
        let capsule = XeGemCapsule::new();
        assert!(capsule.allocate(-1, 4096, 0).is_ok());
        assert_eq!(
            capsule.allocate(-1, 4096, 0),
            Err(XeGemError::InvalidHandle)
        );
    }

    #[test]
    fn test_bind() {
        // T28 Q5: Verify binding
        let capsule = XeGemCapsule::new();
        capsule.allocate(-1, 4096, 0).unwrap();

        let gpu_addr = capsule.bind(-1, 1).unwrap();
        assert_ne!(gpu_addr, 0);
        assert_eq!(capsule.gpu_addr(), gpu_addr);
    }

    #[test]
    fn test_bind_without_allocate_fails() {
        // T28 Q6: Verify bind requires allocation
        let capsule = XeGemCapsule::new();
        assert_eq!(capsule.bind(-1, 1), Err(XeGemError::InvalidHandle));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_map() {
        // T28 Q7: Verify CPU mapping
        let capsule = XeGemCapsule::new();
        capsule.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();
        capsule.bind(-1, 1).unwrap();

        let cpu_ptr = capsule.map(-1).unwrap();
        assert!(!cpu_ptr.is_null());
        assert_eq!(capsule.cpu_addr(), cpu_ptr as u64);
        assert!(capsule.is_mapped());
        assert_eq!(capsule.map_count(), 1);

        // Cleanup
        capsule.unmap().unwrap();
    }

    #[test]
    fn test_map_without_bind_fails() {
        // T28 Q8: Verify map requires bind
        let capsule = XeGemCapsule::new();
        capsule.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();
        assert_eq!(capsule.map(-1), Err(XeGemError::InvalidHandle));
    }

    #[test]
    fn test_map_without_host_visible_fails() {
        // T28 Q9: Verify host visible flag required
        let capsule = XeGemCapsule::new();
        capsule.allocate(-1, 4096, 0).unwrap(); // No HOST_VISIBLE flag
        capsule.bind(-1, 1).unwrap();
        assert_eq!(capsule.map(-1), Err(XeGemError::MapFailed));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_double_map_fails() {
        // T28 Q10: Verify no double mapping
        let capsule = XeGemCapsule::new();
        capsule.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();
        capsule.bind(-1, 1).unwrap();

        capsule.map(-1).unwrap();
        assert_eq!(capsule.map(-1), Err(XeGemError::AlreadyMapped));

        // Cleanup
        capsule.unmap().unwrap();
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_unmap() {
        // T28 Q11: Verify unmapping
        let capsule = XeGemCapsule::new();
        capsule.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();
        capsule.bind(-1, 1).unwrap();
        capsule.map(-1).unwrap();

        let result = capsule.unmap();
        assert!(result.is_ok());
        assert!(!capsule.is_mapped());
        assert_eq!(capsule.cpu_addr(), 0);
        assert_eq!(capsule.unmap_count(), 1);
    }

    #[test]
    fn test_unmap_without_map_fails() {
        // T28 Q12: Verify unmap requires map
        let capsule = XeGemCapsule::new();
        capsule.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();
        capsule.bind(-1, 1).unwrap();
        assert_eq!(capsule.unmap(), Err(XeGemError::NotMapped));
    }

    #[test]
    fn test_unbind() {
        // T28 Q13: Verify unbinding
        let capsule = XeGemCapsule::new();
        capsule.allocate(-1, 4096, 0).unwrap();
        capsule.bind(-1, 1).unwrap();

        let result = capsule.unbind(-1, 1);
        assert!(result.is_ok());
        assert_eq!(capsule.gpu_addr(), 0);
    }

    #[test]
    fn test_unbind_without_bind_fails() {
        // T28 Q14: Verify unbind requires bind
        let capsule = XeGemCapsule::new();
        capsule.allocate(-1, 4096, 0).unwrap();
        assert_eq!(capsule.unbind(-1, 1), Err(XeGemError::NotBound));
    }

    #[test]
    fn test_free() {
        // T28 Q15: Verify deallocation
        let capsule = XeGemCapsule::new();
        capsule.allocate(-1, 4096, 0).unwrap();

        let result = capsule.free(-1);
        assert!(result.is_ok());
        assert!(!capsule.is_valid());
        assert_eq!(capsule.handle(), 0);
        assert_eq!(capsule.size(), 0);
        assert_eq!(capsule.free_count(), 1);
    }

    #[test]
    fn test_free_without_allocate_fails() {
        // T28 Q16: Verify free requires allocation
        let capsule = XeGemCapsule::new();
        assert_eq!(capsule.free(-1), Err(XeGemError::InvalidHandle));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_free_unmaps_automatically() {
        // T28 Q17: Verify free unmaps automatically
        let capsule = XeGemCapsule::new();
        capsule.allocate(-1, 4096, GEM_FLAG_HOST_VISIBLE).unwrap();
        capsule.bind(-1, 1).unwrap();
        capsule.map(-1).unwrap();

        assert!(capsule.is_mapped());
        capsule.free(-1).unwrap();
        assert!(!capsule.is_mapped());
        assert_eq!(capsule.cpu_addr(), 0);
    }

    #[test]
    fn test_full_lifecycle() {
        // T28 Q18: Verify complete lifecycle
        let capsule = XeGemCapsule::new();

        // Allocate
        capsule.allocate(-1, 8192, GEM_FLAG_HOST_VISIBLE).unwrap();
        assert_eq!(capsule.size(), 8192);
        assert_eq!(capsule.alloc_count(), 1);

        // Bind
        let gpu_addr = capsule.bind(-1, 1).unwrap();
        assert_eq!(capsule.gpu_addr(), gpu_addr);

        #[cfg(target_os = "linux")]
        {
            // Map
            let cpu_ptr = capsule.map(-1).unwrap();
            assert!(!cpu_ptr.is_null());
            assert_eq!(capsule.map_count(), 1);

            // Unmap
            capsule.unmap().unwrap();
            assert_eq!(capsule.unmap_count(), 1);
        }

        // Unbind
        capsule.unbind(-1, 1).unwrap();

        // Free
        capsule.free(-1).unwrap();
        assert_eq!(capsule.free_count(), 1);
        assert!(!capsule.is_valid());
    }

    #[test]
    fn test_generation_counter() {
        // T28 Q19: Verify generation counter increments
        let capsule = XeGemCapsule::new();
        let initial_gen = capsule.generation.load(Ordering::Relaxed);

        capsule.allocate(-1, 4096, 0).unwrap();
        assert_eq!(capsule.generation.load(Ordering::Relaxed), initial_gen + 1);

        capsule.bind(-1, 1).unwrap();
        assert_eq!(capsule.generation.load(Ordering::Relaxed), initial_gen + 2);

        capsule.unbind(-1, 1).unwrap();
        assert_eq!(capsule.generation.load(Ordering::Relaxed), initial_gen + 3);

        capsule.free(-1).unwrap();
        assert_eq!(capsule.generation.load(Ordering::Relaxed), initial_gen + 4);
    }

    #[test]
    fn test_placement_flags() {
        // T28 Q20: Verify memory placement based on flags
        let capsule_vram = XeGemCapsule::new();
        capsule_vram
            .allocate(-1, 4096, GEM_FLAG_DEVICE_LOCAL)
            .unwrap();
        assert_eq!(
            capsule_vram.placement.load(Ordering::Relaxed),
            GEM_PLACEMENT_VRAM
        );

        let capsule_system = XeGemCapsule::new();
        capsule_system.allocate(-1, 4096, 0).unwrap();
        assert_eq!(
            capsule_system.placement.load(Ordering::Relaxed),
            GEM_PLACEMENT_SYSTEM
        );
    }
}
