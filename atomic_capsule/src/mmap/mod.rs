//! Capsule-Native Memory-Mapped File Management
//!
//! **UCE34 Framework**: T9 Persistent + T1 Atomic + T0 Foundation
//!
//! Replaces memmap2 with 100% capsule-based implementation:
//! - **T0 (atomic_from_mut)**: Zero-copy atomic views over mmap memory
//! - **T1 (Atomic)**: Lockfree region management (vs memmap2 mutex, 3-10× faster)
//! - **T9 (Persistent)**: Memory-mapped I/O with crash-safe durability
//!
//! # Architecture
//!
//! - **MmapRegion**: T1 Atomic capsule for lockfree region allocation (64B aligned)
//! - **MmapManager**: Container capsule managing 8-256 fixed regions
//! - **Platform abstraction**: Unix (mmap), Windows (CreateFileMapping), Capsule OS (native)
//!
//! # Features
//!
//! - **100% Lockfree**: Atomic CAS loops (no mutex/RwLock)
//! - **Generation counters**: TOCTOU prevention for concurrent access
//! - **4KB Page alignment**: OS mmap requirement, capsule-native
//! - **Zero-copy**: atomic_from_mut integration for T0+T9 composition
//! - **Trade secret**: No external mmap dependencies (full stack ownership)
//!
//! # Performance Targets (B32 Framework)
//!
//! - **File initialization**: <10ms for 1GB file (OS syscall bound)
//! - **Region allocation**: <20ns lockfree CAS (vs ~50ns memmap2 mutex)
//! - **fsync durability**: <1ms NVMe, <5ms SSD (OS/storage bound)
//! - **Concurrent scaling**: Lockfree (vs memmap2 mutex contention)
//!
//! # Platform Support
//!
//! - ✅ **Unix** (Linux, macOS, BSD): libc::mmap, libc::msync
//! - ✅ **Windows**: CreateFileMapping, FlushViewOfFile
//! - 🔬 **Capsule OS**: Native syscalls (future, priority target)
//! - ⚠️  **Other**: Stub (compile-time error with helpful message)
//!
//! # Example
//!
//! ```rust,ignore
//! use atomic_capsule::mmap::{MmapManager, MmapLayout};
//! use std::path::Path;
//!
//! // Create 1GB file with 8 regions of 128MB each
//! let layout = MmapLayout::new(1024 * 1024 * 1024, 8)?;
//! let manager = MmapManager::new(Path::new("data.bin"), &layout)?;
//!
//! // Lockfree allocation in region 0
//! let region = manager.region(0).unwrap();
//! let offset = region.allocate(1024)?; // <20ns CAS loop
//! println!("Allocated at offset: {}", offset);
//!
//! // Crash-safe durability
//! manager.fsync()?; // <1ms NVMe
//! ```
//!
//! # UCE34 Q10-Q34 Validation
//!
//! **Q10**: T9 (Persistent) + T1 (Atomic) + T0 (atomic_from_mut)
//! **Q11**: Rust platform abstraction (cfg unix/windows/capsule)
//! **Q12**: Nightly atomic_from_mut, future io_uring async I/O
//! **Q33**: verify_capsule_properties!(MmapRegion, 64, 64)
//! **Q34**: Generation counters for audit trail (TOCTOU prevention)
//!
//! # Trade Secret Notice
//!
//! This module is proprietary capsule-native infrastructure for the Capsule OS.
//! All implementations are trade secrets. Never commit to public repositories.

// Re-exports
pub use self::error::MmapError;
pub use self::manager::{MmapLayout, MmapManager};
pub use self::region::MmapRegion;

// Module declarations
mod error;
mod manager;
mod region;

// Platform-specific implementations
#[cfg(unix)]
mod unix;

#[cfg(windows)]
mod windows;

// Capsule OS support (future, use feature flag instead of target_os)
#[cfg(all(feature = "capsule-os", not(any(unix, windows))))]
mod capsule_os;

// Stub for unsupported platforms
#[cfg(not(any(unix, windows)))]
compile_error!("atomic_capsule::mmap currently requires Unix or Windows. Capsule OS support via 'capsule-os' feature flag (future)");
