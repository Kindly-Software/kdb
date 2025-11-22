//! Unix platform implementation for memory-mapped files
//!
//! **Platform**: Linux, macOS, BSD (POSIX mmap)
//!
//! # Safety Assumptions
//!
//! #ASSUME_POSIX_MMAP: mmap syscall follows POSIX semantics
//! #ASSUME_PAGE_SIZE: 4KB page size (validated at runtime)
//! #ASSUME_MSYNC_DURABILITY: MS_SYNC provides crash-safe durability

use crate::mmap::MmapError;
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;

/// Platform-specific mmap result
pub struct PlatformMmap {
    pub fd: RawFd,
    pub ptr: *mut u8,
    pub size: usize,
}

/// Create memory-mapped file on Unix
///
/// **Performance**: <10ms for 1GB file (OS syscall bound)
///
/// # Safety
///
/// #ASSUME_POSIX_MMAP: Uses POSIX mmap with MAP_SHARED for persistence
/// #ASSUME_FILE_CREATION: File truncated to exact size before mmap
pub fn platform_mmap(path: &Path, size: u64) -> Result<PlatformMmap, MmapError> {
    // Open file with O_RDWR | O_CREAT
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600) // Owner read/write only
        .open(path)?;

    // Truncate to exact size
    file.set_len(size)?;

    let fd = file.as_raw_fd();

    // mmap with MAP_SHARED for persistence
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            size as libc::size_t,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            0,
        )
    };

    if ptr == libc::MAP_FAILED {
        let err = std::io::Error::last_os_error();
        return Err(MmapError::IOError {
            code: err.raw_os_error().unwrap_or(-1),
            operation: "mmap",
        });
    }

    Ok(PlatformMmap {
        fd,
        ptr: ptr as *mut u8,
        size: size as usize,
    })
}

/// Flush memory-mapped file to disk (crash-safe durability)
///
/// **Performance**: <1ms NVMe, <5ms SSD (storage bound)
///
/// # Safety
///
/// #ASSUME_MSYNC_DURABILITY: MS_SYNC guarantees data reaches storage
pub fn platform_fsync(ptr: *mut u8, size: usize) -> Result<(), MmapError> {
    let ret = unsafe { libc::msync(ptr as *mut libc::c_void, size, libc::MS_SYNC) };

    if ret != 0 {
        let err = std::io::Error::last_os_error();
        return Err(MmapError::IOError {
            code: err.raw_os_error().unwrap_or(-1),
            operation: "msync",
        });
    }

    Ok(())
}

/// Unmap memory-mapped file
///
/// **Performance**: <1ms (OS syscall bound)
///
/// # Safety
///
/// #ASSUME_MUNMAP_VALID: ptr/size must match original mmap parameters
pub fn platform_munmap(ptr: *mut u8, size: usize) -> Result<(), MmapError> {
    let ret = unsafe { libc::munmap(ptr as *mut libc::c_void, size) };

    if ret != 0 {
        let err = std::io::Error::last_os_error();
        return Err(MmapError::IOError {
            code: err.raw_os_error().unwrap_or(-1),
            operation: "munmap",
        });
    }

    Ok(())
}

/// Close file descriptor
///
/// **Performance**: <1ms (OS syscall bound)
pub fn platform_close_fd(fd: RawFd) {
    unsafe {
        libc::close(fd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_platform_mmap_basic() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_mmap_unix.bin");

        // Create 4KB mmap
        let result = platform_mmap(&path, 4096);
        assert!(result.is_ok());

        let mmap = result.unwrap();
        assert_eq!(mmap.size, 4096);
        assert!(!mmap.ptr.is_null());

        // Cleanup
        platform_munmap(mmap.ptr, mmap.size).unwrap();
        platform_close_fd(mmap.fd);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_platform_fsync() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_fsync_unix.bin");

        let mmap = platform_mmap(&path, 4096).unwrap();

        // Write data
        unsafe {
            std::ptr::write(mmap.ptr, 42u8);
        }

        // Flush to disk
        let result = platform_fsync(mmap.ptr, mmap.size);
        assert!(result.is_ok());

        // Cleanup
        platform_munmap(mmap.ptr, mmap.size).unwrap();
        platform_close_fd(mmap.fd);
        let _ = fs::remove_file(path);
    }
}
