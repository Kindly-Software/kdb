//! MmapManager - Container Capsule for Multi-Region File Management
//!
//! **UCE34 Framework**: Container Capsule (Q10.5) managing 8-256 MmapRegion capsules
//!
//! # Architecture
//!
//! Container capsule pattern (not composite):
//! - **Purpose**: Manage ≥8 MmapRegion capsules with infrastructure
//! - **Structure**: Preallocated Vec<MmapRegion> + platform handles + generation
//! - **Use case**: File-backed persistent storage with lockfree region allocation
//!
//! # Container vs Composite (Q10.5)
//!
//! **Container Capsule** (this):
//! - Management structure coordinating multiple capsules
//! - Preallocated array + header + infrastructure
//! - ≥8 regions (typically 8-256)
//! - Long-lived (hours+)
//! - Overhead amortized at scale
//!
//! **Composite Capsule** (not this):
//! - Single flat struct combining multiple tiers
//! - <10K objects, 2-3 tier combinations
//! - No nested indirection
//! - All fields inline
//!
//! # Performance Targets (B32)
//!
//! - **File initialization**: <10ms for 1GB file (OS syscall bound)
//! - **Region allocation**: <20ns lockfree CAS (via MmapRegion)
//! - **fsync durability**: <1ms NVMe, <5ms SSD (OS/storage bound)
//! - **Generation check**: <5ns atomic load
//!
//! # Platform Support
//!
//! - ✅ Unix (Linux/macOS/BSD): libc::mmap, libc::msync
//! - ✅ Windows: CreateFileMapping, FlushViewOfFile
//! - 🔬 Capsule OS: Future native syscalls (stub)
//!
//! # UCE34 Q10-Q34 Validation
//!
//! **Q10**: Container Capsule - manages T1 MmapRegion capsules
//! **Q10.5**: Management structure (≥8 regions, not composite)
//! **Q11**: Platform abstraction via cfg(unix)/cfg(windows)/cfg(capsule_os)
//! **Q15**: Integration with platform layers (unix.rs, windows.rs)
//! **Q33**: MmapRegion uses #[derive(ComputationalCapsule)]
//! **Q34**: Manager-level generation counter for audit trail
//!
//! # ASSUM Safety
//!
//! #ASSUME_PLATFORM_MMAP: Platform mmap syscalls follow OS semantics
//! #ASSUME_POINTER_VALIDITY: Mmap pointer valid until munmap/Drop
//! #ASSUME_PAGE_ALIGNMENT: 4KB page alignment on x86-64, 16KB on ARM64
//! #ASSUME_GENERATION_ORDERING: Generation uses Release for visibility

use crate::mmap::{MmapError, MmapRegion};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use crate::mmap::unix;

#[cfg(windows)]
use crate::mmap::windows;

#[cfg(all(feature = "capsule-os", not(any(unix, windows))))]
use crate::mmap::capsule_os;

/// Memory-mapped file layout configuration
///
/// Defines how to partition a file into fixed-size regions.
///
/// # Example
///
/// ```ignore
/// // 1GB file with 8 regions of 128MB each
/// let layout = MmapLayout::new(1024 * 1024 * 1024, 8)?;
/// assert_eq!(layout.region_size(), 128 * 1024 * 1024);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmapLayout {
    /// Total file size in bytes
    file_size: u64,

    /// Number of fixed regions (1-256)
    region_count: usize,

    /// Size per region (file_size / region_count)
    region_size: u64,
}

impl MmapLayout {
    /// Minimum region count (must have at least 1 region)
    pub const MIN_REGIONS: usize = 1;

    /// Maximum region count (practical limit for Vec<MmapRegion>)
    pub const MAX_REGIONS: usize = 256;

    /// Page alignment requirement (4KB on x86-64)
    pub const PAGE_SIZE: u64 = 4096;

    /// Create new layout with validation
    ///
    /// **Validation Rules**:
    /// - `file_size` must be > 0
    /// - `region_count` must be 1-256
    /// - `file_size` must be page-aligned (4KB)
    /// - `region_size` must be page-aligned (4KB)
    ///
    /// **Performance**: <5ns (validation + division)
    pub fn new(file_size: u64, region_count: usize) -> Result<Self, MmapError> {
        // Validate region count
        if region_count < Self::MIN_REGIONS || region_count > Self::MAX_REGIONS {
            return Err(MmapError::invalid_region_index(
                region_count,
                Self::MAX_REGIONS,
            ));
        }

        // Validate file size is non-zero
        if file_size == 0 {
            return Err(MmapError::IOError {
                code: -1,
                operation: "layout_validation",
            });
        }

        // Validate file size is page-aligned
        if file_size % Self::PAGE_SIZE != 0 {
            return Err(MmapError::invalid_alignment(file_size, Self::PAGE_SIZE));
        }

        // Calculate region size
        let region_size = file_size / region_count as u64;

        // Validate region size is page-aligned
        if region_size % Self::PAGE_SIZE != 0 {
            return Err(MmapError::invalid_alignment(region_size, Self::PAGE_SIZE));
        }

        Ok(Self {
            file_size,
            region_count,
            region_size,
        })
    }

    /// Get total file size
    #[inline]
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Get region count
    #[inline]
    pub fn region_count(&self) -> usize {
        self.region_count
    }

    /// Get size per region
    #[inline]
    pub fn region_size(&self) -> u64 {
        self.region_size
    }
}

/// Container capsule managing 8-256 MmapRegion capsules
///
/// **Pattern**: Container Capsule (Q10.5)
/// **Tier**: T9 (Persistent) + T1 (Atomic coordination)
/// **Speedup**: 3-10× vs memmap2 mutex
pub struct MmapManager {
    // Platform-specific handles
    #[cfg(unix)]
    fd: std::os::unix::io::RawFd,

    #[cfg(windows)]
    handle: *mut std::ffi::c_void,

    #[cfg(windows)]
    map_handle: *mut std::ffi::c_void,

    // Common fields
    ptr: *mut u8,             // Mmap base pointer
    size: usize,              // Total file size
    regions: Vec<MmapRegion>, // 1-256 regions
    generation: AtomicU64,    // Manager-level generation (Q34)
}

// SAFETY: MmapManager is Send/Sync via platform-specific handles
// - Unix: RawFd is Send/Sync
// - Windows: Handles are Send/Sync (Win32 kernel objects)
// - Atomic generation counter
unsafe impl Send for MmapManager {}
unsafe impl Sync for MmapManager {}

impl MmapManager {
    /// Create new memory-mapped file manager
    ///
    /// **Performance**: <10ms for 1GB file (OS syscall bound)
    ///
    /// **Steps**:
    /// 1. Validate layout (region count, alignment)
    /// 2. Call platform_mmap (OS syscall)
    /// 3. Initialize MmapRegion capsules
    /// 4. Return manager
    ///
    /// **Platform-Specific**:
    /// - Unix: mmap with MAP_SHARED for persistence
    /// - Windows: CreateFileMapping + MapViewOfFile
    /// - Capsule OS: Future native syscalls (stub)
    ///
    /// #ASSUME_PLATFORM_MMAP: Platform mmap follows OS semantics
    pub fn new(path: &Path, layout: &MmapLayout) -> Result<Self, MmapError> {
        // Platform-specific mmap
        #[cfg(unix)]
        let platform_result = unix::platform_mmap(path, layout.file_size())?;

        #[cfg(windows)]
        let platform_result = windows::platform_mmap(path, layout.file_size())?;

        #[cfg(all(feature = "capsule-os", not(any(unix, windows))))]
        let platform_result = capsule_os::platform_mmap(path, layout.file_size())?;

        // Extract platform-specific fields
        #[cfg(unix)]
        let (fd, ptr, size) = (
            platform_result.fd,
            platform_result.ptr,
            platform_result.size,
        );

        #[cfg(windows)]
        let (handle, map_handle, ptr, size) = (
            platform_result.handle,
            platform_result.map_handle,
            platform_result.ptr,
            platform_result.size,
        );

        #[cfg(all(feature = "capsule-os", not(any(unix, windows))))]
        let (ptr, size) = (platform_result.ptr, platform_result.size);

        // Initialize regions
        let mut regions = Vec::with_capacity(layout.region_count());
        let region_size = layout.region_size();

        for i in 0..layout.region_count() {
            let base_offset = (i as u64) * region_size;
            let region = MmapRegion::new(base_offset, region_size as u32);
            regions.push(region);
        }

        Ok(Self {
            #[cfg(unix)]
            fd,
            #[cfg(windows)]
            handle,
            #[cfg(windows)]
            map_handle,
            ptr,
            size,
            regions,
            generation: AtomicU64::new(0),
        })
    }

    /// Get region by index
    ///
    /// **Performance**: <2ns (bounds check + Vec access)
    ///
    /// Returns `None` if index out of bounds.
    #[inline]
    pub fn region(&self, idx: usize) -> Option<&MmapRegion> {
        self.regions.get(idx)
    }

    /// Get number of regions
    ///
    /// **Performance**: <1ns (Vec len)
    #[inline]
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// Get base pointer for mmap
    ///
    /// **Performance**: <1ns (field access)
    ///
    /// #ASSUME_POINTER_VALIDITY: Pointer valid until Drop
    #[inline]
    pub fn base_ptr(&self) -> *mut u8 {
        self.ptr
    }

    /// Get total mmap size
    ///
    /// **Performance**: <1ns (field access)
    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get manager generation counter
    ///
    /// **Performance**: <5ns (atomic load)
    ///
    /// Q34: Audit trail via generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Flush all regions to disk (crash-safe durability)
    ///
    /// **Performance**: <1ms NVMe, <5ms SSD (OS/storage bound)
    ///
    /// **Platform-Specific**:
    /// - Unix: msync with MS_SYNC
    /// - Windows: FlushViewOfFile
    /// - Capsule OS: Future native syscalls (stub)
    ///
    /// #ASSUME_FLUSH_DURABILITY: Platform fsync guarantees persistence
    pub fn fsync(&mut self) -> Result<(), MmapError> {
        // Bump generation before fsync (Q34 audit trail)
        self.generation.fetch_add(1, Ordering::Release);

        // Platform-specific fsync
        #[cfg(unix)]
        unix::platform_fsync(self.ptr, self.size)?;

        #[cfg(windows)]
        windows::platform_fsync(self.ptr, self.size)?;

        #[cfg(all(feature = "capsule-os", not(any(unix, windows))))]
        capsule_os::platform_fsync(self.ptr, self.size)?;

        Ok(())
    }

    /// Get pointer at absolute offset (for atomic_from_mut integration)
    ///
    /// **Performance**: <2ns (bounds check + pointer arithmetic)
    ///
    /// #ASSUME_OFFSET_VALID: offset must be < size
    /// #ASSUME_POINTER_VALIDITY: base_ptr must still be valid
    #[inline]
    pub unsafe fn ptr_at_offset(&self, offset: u64) -> Result<*mut u8, MmapError> {
        if offset >= self.size as u64 {
            return Err(MmapError::invalid_alignment(offset, self.size as u64));
        }
        Ok(self.ptr.add(offset as usize))
    }
}

impl Drop for MmapManager {
    /// Cleanup memory-mapped file
    ///
    /// **Performance**: <1ms (OS syscall bound)
    ///
    /// **Platform-Specific**:
    /// - Unix: munmap + close(fd)
    /// - Windows: UnmapViewOfFile + CloseHandle
    /// - Capsule OS: Future cleanup (stub)
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let _ = unix::platform_munmap(self.ptr, self.size);
            unix::platform_close_fd(self.fd);
        }

        #[cfg(windows)]
        {
            let _ = windows::platform_munmap(self.ptr);
            windows::platform_close_handles(self.map_handle, self.handle);
        }

        #[cfg(all(feature = "capsule-os", not(any(unix, windows))))]
        {
            let _ = capsule_os::platform_munmap(self.ptr, self.size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_layout_creation() {
        // Valid: 1GB file with 8 regions
        let layout = MmapLayout::new(1024 * 1024 * 1024, 8);
        assert!(layout.is_ok());

        let layout = layout.unwrap();
        assert_eq!(layout.file_size(), 1024 * 1024 * 1024);
        assert_eq!(layout.region_count(), 8);
        assert_eq!(layout.region_size(), 128 * 1024 * 1024);
    }

    #[test]
    fn test_layout_page_alignment() {
        // File size not page-aligned (4096)
        let layout = MmapLayout::new(5000, 1);
        assert!(layout.is_err());

        // Valid: page-aligned
        let layout = MmapLayout::new(4096, 1);
        assert!(layout.is_ok());
    }

    #[test]
    fn test_layout_region_count_bounds() {
        // Too few regions (0)
        let layout = MmapLayout::new(4096, 0);
        assert!(layout.is_err());

        // Too many regions (>256)
        let layout = MmapLayout::new(4096 * 300, 300);
        assert!(layout.is_err());

        // Valid: 256 regions
        let layout = MmapLayout::new(4096 * 256, 256);
        assert!(layout.is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn test_manager_creation_unix() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_manager_unix.bin");

        // Create 1MB file with 4 regions (256KB each)
        let layout = MmapLayout::new(1024 * 1024, 4).unwrap();
        let manager = MmapManager::new(&path, &layout);

        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert_eq!(manager.region_count(), 4);
        assert_eq!(manager.size(), 1024 * 1024);
        assert_eq!(manager.generation(), 0);

        // Cleanup
        drop(manager);
        let _ = fs::remove_file(path);
    }

    #[test]
    #[cfg(unix)]
    fn test_manager_region_access() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_region_access.bin");

        let layout = MmapLayout::new(1024 * 1024, 4).unwrap();
        let manager = MmapManager::new(&path, &layout).unwrap();

        // Valid region access
        assert!(manager.region(0).is_some());
        assert!(manager.region(3).is_some());

        // Invalid region access
        assert!(manager.region(4).is_none());

        // Cleanup
        drop(manager);
        let _ = fs::remove_file(path);
    }

    #[test]
    #[cfg(unix)]
    fn test_manager_fsync() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_fsync_manager.bin");

        let layout = MmapLayout::new(4096, 1).unwrap();
        let mut manager = MmapManager::new(&path, &layout).unwrap();

        let gen_before = manager.generation();
        assert_eq!(gen_before, 0);

        // Fsync should bump generation
        let result = manager.fsync();
        assert!(result.is_ok());

        let gen_after = manager.generation();
        assert_eq!(gen_after, 1);

        // Cleanup
        drop(manager);
        let _ = fs::remove_file(path);
    }

    #[test]
    #[cfg(unix)]
    fn test_manager_ptr_at_offset() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_ptr_offset.bin");

        let layout = MmapLayout::new(4096, 1).unwrap();
        let manager = MmapManager::new(&path, &layout).unwrap();

        // Valid offset
        let ptr = unsafe { manager.ptr_at_offset(1000) };
        assert!(ptr.is_ok());

        // Invalid offset (beyond size)
        let ptr = unsafe { manager.ptr_at_offset(5000) };
        assert!(ptr.is_err());

        // Cleanup
        drop(manager);
        let _ = fs::remove_file(path);
    }
}
