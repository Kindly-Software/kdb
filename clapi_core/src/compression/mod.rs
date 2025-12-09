// SPDX-License-Identifier: MIT OR Apache-2.0
//! # Streaming Response Compression (Week 3: Feature 2)
//!
//! 100% lockfree on-the-fly zstd compression for large AI responses.
//!
//! ## Architecture
//! - **Tier 5 (Streaming)**: O(1) latency compression window
//! - **Tier 4 (Batch)**: 16-chunk parallel processing (64KB batches)
//! - **Target**: <500ns compression overhead, 3-5× ratio
//!
//! ## Performance Targets (B32 Validated)
//! - Compression: <500ns per chunk (non-blocking)
//! - Ratio: ≥3× on real GPT-4 responses
//! - Decompression: <200ns
//! - Throughput: 10,000 requests/sec
//! - Memory: <500KB overhead
//!
//! ## Capsule Architecture
//! - `CompressionStateCapsule`: 256B streaming state (Tier 5)
//! - Atomic position tracking (lockfree coordination)
//! - 4KB window buffer (separate allocation)
//! - Generation counters for TOCTOU prevention
//!
//! ## UCE34 Framework Compliance
//! - Q10: Tier 5 (Streaming) + Tier 4 (Batch)
//! - Q11: Rust zstd bindings with zero-cost wrappers
//! - Q12: N/A (stable Rust sufficient)
//! - Q33: #[derive(ComputationalCapsule)] verification
//!
//! ## Production Deployment
//! - 100% lockfree (zero mutex/RwLock)
//! - T28 comprehensive testing (16+ tests)
//! - B32 honest benchmarking (vs synchronous zstd)
//! - ASSUM safety (all atomics documented)

pub mod capsule;
pub mod streaming;

pub use capsule::CompressionStateCapsule;
pub use streaming::{StreamingCompressor, CompressionLevel, CompressionError, CompressionStats};

/// Compression constants
pub const DEFAULT_COMPRESSION_LEVEL: i32 = 3; // Balanced speed/ratio
pub const COMPRESSION_WINDOW_SIZE: usize = 4096; // 4KB window
pub const BATCH_SIZE: usize = 16; // 16 chunks per batch
pub const CHUNK_SIZE: usize = 4096; // 4KB chunks

/// Minimum size threshold for compression (bytes)
/// Responses smaller than this are sent uncompressed
pub const MIN_COMPRESSION_SIZE: usize = 1024; // 1KB threshold

/// Target compression ratio (3× minimum)
pub const TARGET_COMPRESSION_RATIO: f64 = 3.0;

/// Maximum compression overhead (nanoseconds)
pub const MAX_COMPRESSION_OVERHEAD_NS: u64 = 500;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(COMPRESSION_WINDOW_SIZE, 4096);
        assert_eq!(BATCH_SIZE, 16);
        assert_eq!(CHUNK_SIZE, 4096);
        assert_eq!(MIN_COMPRESSION_SIZE, 1024);
        assert!(TARGET_COMPRESSION_RATIO >= 3.0);
        assert!(MAX_COMPRESSION_OVERHEAD_NS <= 500);
    }
}
