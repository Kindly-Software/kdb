//! Hardening Module - Security, Safety, Fuzzing, Benchmarking, and Error Recovery Capsules
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! # Overview
//!
//! This module provides security hardening capsules for kindly-av1, implementing
//! defense-in-depth strategies against common vulnerability classes:
//!
//! - **Buffer overflows** (CWE-119, CWE-787, CWE-125)
//! - **Integer overflows** (CWE-190)
//! - **Null pointer dereferences** (CWE-476)
//! - **Memory corruption** (CWE-416, CWE-415)
//! - **Stream corruption** (error recovery and resynchronization)
//! - **Fuzzing** (automated malformed input generation and crash detection)
//! - **Performance regression** (B32-compliant benchmark validation)
//!
//! # Capsule Inventory
//!
//! | Capsule | Tier | Size | Purpose |
//! |---------|------|------|---------|
//! | BoundsCheckerCapsule | T1 | 128B | Memory bounds validation |
//! | ErrorRecoveryCapsule | T1 | 256B | Error tracking, sync point detection, recovery |
//! | FuzzHarnessCapsule | T4 | 256B | Fuzzing infrastructure, mutation, crash tracking |
//! | BenchmarkHarnessCapsule | T1 | 256B | B32-compliant performance validation |
//!
//! # Framework Compliance
//!
//! All hardening capsules comply with:
//!
//! - **UCE34**: Systematic security design (Q10 tier selection)
//! - **Chaos**: 100% lockfree, cache-aligned, generation counters
//! - **ASSUM**: All unsafe operations documented
//! - **T28**: 5-tier testing (unit/property/integration/production/determinism)
//! - **Q34**: Hash-chained audit trails for security events
//! - **B32**: Statistical validation (95% CI, 1000+ iterations)
//!
//! # Usage
//!
//! ## Bounds Checking
//!
//! ```rust,ignore
//! use kindly_av1::hardening::{BoundsCheckerCapsule, BoundsViolation};
//!
//! let checker = BoundsCheckerCapsule::new();
//! checker.set_frame_bounds(1920, 1080);
//!
//! // Inline bounds checking
//! checker.check_read(offset, length, buffer_size)?;
//! checker.check_write(offset, length, buffer_size)?;
//! checker.check_mv_bounds(base_x, base_y, mv_x, mv_y, block_w, block_h, frame_w, frame_h)?;
//!
//! // Get statistics
//! let stats = checker.stats();
//! println!("Violation rate: {:.4}%", stats.violation_rate * 100.0);
//! ```
//!
//! ## Error Recovery
//!
//! ```rust,ignore
//! use kindly_av1::hardening::{ErrorRecoveryCapsule, ErrorCategory, VideoCodec};
//!
//! let recovery = ErrorRecoveryCapsule::new();
//!
//! // Report an error
//! recovery.report_error(ErrorCategory::BitstreamCorruption, 1234, "invalid NAL header");
//!
//! // Get recovery strategy
//! let strategy = recovery.get_recovery_strategy(ErrorCategory::BitstreamCorruption);
//!
//! // Find sync point to resume decoding
//! let data = &corrupted_stream[..];
//! if let Some(offset) = recovery.find_sync_point(data, VideoCodec::H264) {
//!     // Resume decoding from sync point
//! }
//!
//! // Track stream health
//! if !recovery.is_stream_healthy() {
//!     println!("Stream error rate too high: {:.2}/1000", recovery.error_rate());
//! }
//! ```
//!
//! ## Fuzzing
//!
//! ```rust,ignore
//! use kindly_av1::hardening::{FuzzHarnessCapsule, FuzzTarget, MutationStrategy};
//!
//! let harness = FuzzHarnessCapsule::with_seed(42);
//!
//! // Generate random test input for H.264 target
//! let data = harness.generate_random(FuzzTarget::H264Bitstream, 1024, 0);
//!
//! // Fuzz single iteration
//! let result = harness.fuzz_once(FuzzTarget::H264Bitstream, &data);
//! if result.is_crash() {
//!     println!("Found crash: {:?}", result.crash);
//! }
//!
//! // Batch fuzzing
//! let summary = harness.fuzz_iterations(FuzzTarget::H264Bitstream, 1000);
//! println!("Crashes: {}, Coverage: {}", summary.crashes, summary.coverage_increase);
//!
//! // Mutation strategies
//! let mutated = harness.mutate(&data, MutationStrategy::Havoc, 0);
//! ```
//!
//! ## Benchmark Harness
//!
//! ```rust,ignore
//! use kindly_av1::hardening::{BenchmarkHarnessCapsule, BenchmarkTarget, B32Config};
//!
//! let harness = BenchmarkHarnessCapsule::new();
//! harness.set_target(BenchmarkTarget::H264Transform);
//! harness.set_config(B32Config::default());
//!
//! // Run benchmark with timing
//! let result = harness.run(1000, || {
//!     // ... operation to benchmark ...
//! });
//!
//! println!("{}", harness.format_result());
//!
//! // Compare against baseline
//! let comparison = harness.compare(&baseline_result);
//! if comparison.is_regression(0.05) {
//!     println!("Performance regression detected!");
//! }
//! ```

pub mod benchmark_harness;
pub mod bounds_checker;
pub mod error_recovery;
pub mod fuzz_harness;

// Re-export main types from bounds_checker
pub use bounds_checker::{
    // Capsule
    BoundsCheckerCapsule,
    // Statistics
    BoundsCheckerStats,
    // Enums
    BoundsCheckType,
    BoundsViolation,
    // Flags
    bounds_flags,
};

// Re-export main types from error_recovery
pub use error_recovery::{
    // Main capsule
    ErrorRecoveryCapsule,
    ErrorRecoveryStats,
    // Error types
    ErrorCategory,
    RecoveryStrategy,
    ConcealmentStrategy,
    RecoveryState,
    VideoCodec,
    // Constants
    H264_START_CODE,
    H264_START_CODE_3,
    VP9_FRAME_MARKER,
    MP4_MDAT,
    MKV_CLUSTER,
    H264_NAL_TYPE_MASK,
    H264_NAL_IDR_SLICE,
    H264_NAL_SPS,
    VP9_KEYFRAME_BIT,
    DEFAULT_ERROR_RATE_THRESHOLD,
    DEFAULT_MAX_CONSECUTIVE_ERRORS,
    ERROR_WINDOW_SIZE,
};

// Re-export main types from benchmark_harness
pub use benchmark_harness::{
    // Main capsule
    BenchmarkHarnessCapsule,
    // Configuration
    B32Config,
    // Results
    BenchmarkResult,
    BenchmarkStats,
    Comparison,
    // Enums
    BenchmarkTarget,
    MetricType,
};

// Re-export main types from fuzz_harness
pub use fuzz_harness::{
    // Capsule
    FuzzHarnessCapsule,
    // Enums
    FuzzTarget,
    MutationStrategy,
    CrashType,
    // Error
    FuzzError,
    // Results
    FuzzResult,
    FuzzSummary,
    FuzzStats,
    CorpusEntry,
    // Constants
    INTERESTING_U8,
    INTERESTING_U16,
    INTERESTING_U32,
    H264_NAL_TYPES,
    VP9_FRAME_TYPES,
    MAX_CORPUS_ENTRIES,
    MAX_MUTATION_SIZE,
    // State flags
    state_flags,
};

// libFuzzer entry points (only available with fuzzing feature)
#[cfg(fuzzing)]
pub use fuzz_harness::{
    fuzz_target_h264,
    fuzz_target_vp9,
    fuzz_target_mp4,
    fuzz_target_mkv,
};

// Re-export macros
#[cfg(feature = "bounds-checking")]
pub use crate::{bounds_check_index, bounds_check_read, bounds_check_write};

#[cfg(not(feature = "bounds-checking"))]
pub use crate::{bounds_check_index, bounds_check_read, bounds_check_write};
