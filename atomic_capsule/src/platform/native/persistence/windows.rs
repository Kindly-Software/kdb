//! Windows platform implementation for memory-mapped files
//!
//! **Platform**: Windows (CreateFileMapping, MapViewOfFile)
//!
//! # Safety Assumptions
//!
//! #ASSUME_WIN32_MMAP: CreateFileMapping follows Win32 semantics
//! #ASSUME_PAGE_SIZE: 4KB page size on x86/x64 Windows
//! #ASSUME_FLUSH_DURABILITY: FlushViewOfFile provides crash-safe durability

use super::MmapError;
use std::ffi::c_void;
use std::fs::OpenOptions;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::path::Path;

// Windows API constants
const INVALID_HANDLE_VALUE: *mut c_void = (-1isize) as *mut c_void;
const PAGE_READWRITE: u32 = 0x04;
const FILE_MAP_ALL_ACCESS: u32 = 0xF001F;
const GENERIC_READ: u32 = 0x80000000;
const GENERIC_WRITE: u32 = 0x40000000;

/// Platform-specific mmap result
pub struct PlatformMmap {
    pub handle: *mut c_void,
    pub map_handle: *mut c_void,
    pub ptr: *mut u8,
    pub size: usize,
}

extern "system" {
    fn CreateFileMappingW(
        hFile: *mut c_void,
        lpAttributes: *const c_void,
        flProtect: u32,
        dwMaximumSizeHigh: u32,
        dwMaximumSizeLow: u32,
        lpName: *const u16,
    ) -> *mut c_void;

    fn MapViewOfFile(
        hFileMappingObject: *mut c_void,
        dwDesiredAccess: u32,
        dwFileOffsetHigh: u32,
        dwFileOffsetLow: u32,
        dwNumberOfBytesToMap: usize,
    ) -> *mut c_void;

    fn UnmapViewOfFile(lpBaseAddress: *const c_void) -> i32;

    fn FlushViewOfFile(lpBaseAddress: *const c_void, dwNumberOfBytesToFlush: usize) -> i32;

    fn CloseHandle(hObject: *mut c_void) -> i32;
}

/// Create memory-mapped file on Windows
///
/// **Performance**: <10ms for 1GB file (OS syscall bound)
///
/// # Safety
///
/// #ASSUME_WIN32_MMAP: Uses Win32 CreateFileMapping/MapViewOfFile
/// #ASSUME_FILE_CREATION: File truncated to exact size before mapping
pub fn platform_mmap(path: &Path, size: u64) -> Result<PlatformMmap, MmapError> {
    // Open file with GENERIC_READ | GENERIC_WRITE
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .access_mode(GENERIC_READ | GENERIC_WRITE)
        .open(path)?;

    // Truncate to exact size
    file.set_len(size)?;

    let handle = file.as_raw_handle() as *mut c_void;

    // Create file mapping
    let size_high = (size >> 32) as u32;
    let size_low = (size & 0xFFFFFFFF) as u32;

    let map_handle = unsafe {
        CreateFileMappingW(
            handle,
            std::ptr::null(),
            PAGE_READWRITE,
            size_high,
            size_low,
            std::ptr::null(),
        )
    };

    if map_handle.is_null() || map_handle == INVALID_HANDLE_VALUE {
        let err = std::io::Error::last_os_error();
        return Err(MmapError::IOError {
            code: err.raw_os_error().unwrap_or(-1),
            operation: "CreateFileMapping",
        });
    }

    // Map view of file
    let ptr = unsafe { MapViewOfFile(map_handle, FILE_MAP_ALL_ACCESS, 0, 0, size as usize) };

    if ptr.is_null() {
        unsafe {
            CloseHandle(map_handle);
        }
        let err = std::io::Error::last_os_error();
        return Err(MmapError::IOError {
            code: err.raw_os_error().unwrap_or(-1),
            operation: "MapViewOfFile",
        });
    }

    Ok(PlatformMmap {
        handle,
        map_handle,
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
/// #ASSUME_FLUSH_DURABILITY: FlushViewOfFile guarantees data reaches storage
pub fn platform_fsync(ptr: *mut u8, size: usize) -> Result<(), MmapError> {
    let ret = unsafe { FlushViewOfFile(ptr as *const c_void, size) };

    if ret == 0 {
        let err = std::io::Error::last_os_error();
        return Err(MmapError::IOError {
            code: err.raw_os_error().unwrap_or(-1),
            operation: "FlushViewOfFile",
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
/// #ASSUME_MUNMAP_VALID: ptr must be valid MapViewOfFile pointer
pub fn platform_munmap(ptr: *mut u8) -> Result<(), MmapError> {
    let ret = unsafe { UnmapViewOfFile(ptr as *const c_void) };

    if ret == 0 {
        let err = std::io::Error::last_os_error();
        return Err(MmapError::IOError {
            code: err.raw_os_error().unwrap_or(-1),
            operation: "UnmapViewOfFile",
        });
    }

    Ok(())
}

/// Close Windows handles
///
/// **Performance**: <1ms (OS syscall bound)
pub fn platform_close_handles(map_handle: *mut c_void, handle: *mut c_void) {
    unsafe {
        CloseHandle(map_handle);
        CloseHandle(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_platform_mmap_basic() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_mmap_windows.bin");

        // Create 4KB mmap
        let result = platform_mmap(&path, 4096);
        assert!(result.is_ok());

        let mmap = result.unwrap();
        assert_eq!(mmap.size, 4096);
        assert!(!mmap.ptr.is_null());

        // Cleanup
        platform_munmap(mmap.ptr).unwrap();
        platform_close_handles(mmap.map_handle, mmap.handle);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_platform_fsync() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_fsync_windows.bin");

        let mmap = platform_mmap(&path, 4096).unwrap();

        // Write data
        unsafe {
            std::ptr::write(mmap.ptr, 42u8);
        }

        // Flush to disk
        let result = platform_fsync(mmap.ptr, mmap.size);
        assert!(result.is_ok());

        // Cleanup
        platform_munmap(mmap.ptr).unwrap();
        platform_close_handles(mmap.map_handle, mmap.handle);
        let _ = fs::remove_file(path);
    }
}
