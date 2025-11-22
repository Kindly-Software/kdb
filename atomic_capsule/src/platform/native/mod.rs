//! # Native Platform Capsule Implementations
//!
//! This module provides OS-specific computational capsules for standard platforms
//! (Linux, macOS, Windows). These capsules leverage native OS features:
//!
//! ## Modules
//!
//! - `persistence`: Memory-mapped file capsules (T9 Persistent)
//!   - Zero-copy atomic access to disk-backed memory
//!   - Crash-safe atomic writes with fsync coordination
//!   - Cross-platform: Linux (mmap), macOS (mmap), Windows (CreateFileMapping)
//!
//! - `async_log`: Async logging capsules (T5 Streaming)
//!   - Lockfree async append with tokio integration
//!   - CAS-protected state coordination
//!   - 20-100× speedup vs Mutex<File>
//!
//! - `network`: Network capsules (T8 Network)
//!   - Distributed cache with HTTP/2 and consistent hashing
//!   - Quorum reads with 2/3 majority voting
//!   - Real-time metrics and monitoring dashboards
//!
//! ## UCE34 Tier Mapping
//!
//! - **T5 (Streaming)**: async_log module
//! - **T8 (Network)**: network module
//! - **T9 (Persistent)**: persistence module
//!
//! ## ASSUM Safety
//!
//! All platform-specific unsafe code documents assumptions:
//! - Memory-mapped files: alignment, validity, lifetime invariants
//! - Async runtime: Send/Sync bounds, cancellation safety
//! - Network sockets: buffer ownership, error handling
//!
//! ## Feature Flags
//!
//! Individual features can be enabled separately:
//! - `capsule-mmap`: Enable T9 persistence module
//! - `async-log`: Enable T5 async logging (requires tokio)
//! - `network`: Enable T8 network module (requires std)
//!
//! Or use the preset:
//! - `preset-native`: Enable all native platform features

// T9 Persistent: Memory-mapped file capsules
#[cfg(any(feature = "preset-native", feature = "capsule-mmap"))]
pub mod persistence;

// T5 Streaming: Async logging capsules
#[cfg(any(feature = "preset-native", feature = "async-log"))]
pub mod async_log;

// T8 Network: Distributed network capsules
#[cfg(any(feature = "preset-native", feature = "network"))]
pub mod network;
