//! Capsule OS platform implementation for memory-mapped files
//!
//! **Platform**: Capsule OS (future native syscalls)
//!
//! # Status: Stub Implementation
//!
//! This is a placeholder for future Capsule OS native mmap support.
//! Currently returns stub errors to enable compilation.

use crate::mmap::MmapError;
use std::path::Path;

/// Platform-specific mmap result (stub)
pub struct PlatformMmap {
    pub ptr: *mut u8,
    pub size: usize,
}

/// Create memory-mapped file on Capsule OS (stub)
pub fn platform_mmap(_path: &Path, _size: u64) -> Result<PlatformMmap, MmapError> {
    Err(MmapError::PlatformUnsupported)
}

/// Flush memory-mapped file to disk (stub)
pub fn platform_fsync(_ptr: *mut u8, _size: usize) -> Result<(), MmapError> {
    Err(MmapError::PlatformUnsupported)
}

/// Unmap memory-mapped file (stub)
pub fn platform_munmap(_ptr: *mut u8, _size: usize) -> Result<(), MmapError> {
    Err(MmapError::PlatformUnsupported)
}
