//! # Architecture Detection Optimization Tests
//!
//! Validates Phase 1 architecture detection optimizations maintain correctness.
//!
//! ## Test Goals
//!
//! 1. **Caching Works**: OnceLock initialization is thread-safe
//! 2. **Const Eval Correct**: Compile-time constants match runtime detection
//! 3. **Backward Compatible**: Existing code continues to work
//! 4. **Architecture-Specific**: Correct values for each platform

use atomic_capsule::{
    detect_cache_line_size, recommended_cold_alignment, recommended_hot_alignment,
    recommended_warm_alignment, CacheLineSize,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ============================================================================
// CACHE LINE SIZE CONSTANTS
// ============================================================================

#[test]
fn test_cache_line_size_constants_valid() {
    assert_eq!(CacheLineSize::STANDARD_64.size(), 64);
    assert_eq!(CacheLineSize::EXTENDED_128.size(), 128);
    assert_eq!(CacheLineSize::LARGE_256.size(), 256);
}

#[test]
fn test_cache_line_size_default() {
    assert_eq!(CacheLineSize::default(), CacheLineSize::STANDARD_64);
}

// ============================================================================
// RUNTIME DETECTION TESTS (Cached with OnceLock)
// ============================================================================

#[test]
fn test_detect_cache_line_size_returns_valid() {
    let size = detect_cache_line_size();
    let size_bytes = size.size();

    // Should be power of 2
    assert!(
        size_bytes.count_ones() == 1,
        "Size {} is not power of 2",
        size_bytes
    );

    // Should be in valid range
    assert!(size_bytes >= 64, "Size {} too small", size_bytes);
    assert!(size_bytes <= 256, "Size {} too large", size_bytes);
}

#[test]
fn test_detect_cache_line_size_consistent() {
    // Multiple calls should return same value (cached)
    let first = detect_cache_line_size();
    let second = detect_cache_line_size();
    let third = detect_cache_line_size();

    assert_eq!(first, second);
    assert_eq!(second, third);
}

#[test]
fn test_detect_cache_line_size_thread_safe() {
    // Concurrent initialization should be safe (OnceLock guarantee)
    let results = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let results = results.clone();

            std::thread::spawn(move || {
                let size = detect_cache_line_size().size();
                results.fetch_add(size, Ordering::Relaxed);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // All threads should have seen same size
    let total = results.load(Ordering::Relaxed);
    let expected_size = detect_cache_line_size().size();
    assert_eq!(total, expected_size * 10);
}

// ============================================================================
// CONST EVALUATION TESTS
// ============================================================================

#[test]
fn test_recommended_hot_alignment_valid() {
    let alignment = recommended_hot_alignment();

    // Should be power of 2
    assert!(
        alignment.count_ones() == 1,
        "Alignment {} is not power of 2",
        alignment
    );

    // Should be reasonable cache line size
    assert!(alignment >= 64, "Alignment {} too small", alignment);
    assert!(alignment <= 256, "Alignment {} too large", alignment);
}

#[test]
fn test_recommended_warm_alignment_valid() {
    let alignment = recommended_warm_alignment();
    let hot = recommended_hot_alignment();

    // Should be 2× hot tier
    assert_eq!(alignment, hot * 2);

    // Should be power of 2
    assert!(alignment.count_ones() == 1);
}

#[test]
fn test_recommended_cold_alignment_valid() {
    let alignment = recommended_cold_alignment();
    let hot = recommended_hot_alignment();

    // Should be 4× hot tier
    assert_eq!(alignment, hot * 4);

    // Should be power of 2
    assert!(alignment.count_ones() == 1);
}

#[test]
fn test_const_functions_are_const() {
    // These should compile as const functions
    const HOT: usize = recommended_hot_alignment();
    const WARM: usize = recommended_warm_alignment();
    const COLD: usize = recommended_cold_alignment();

    // Runtime checks
    assert_eq!(HOT, recommended_hot_alignment());
    assert_eq!(WARM, recommended_warm_alignment());
    assert_eq!(COLD, recommended_cold_alignment());
}

// ============================================================================
// ARCHITECTURE-SPECIFIC TESTS
// ============================================================================

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[test]
fn test_x86_cache_line_size() {
    assert_eq!(detect_cache_line_size(), CacheLineSize::STANDARD_64);
    assert_eq!(recommended_hot_alignment(), 64);
}

#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
#[test]
fn test_arm_cache_line_size() {
    assert_eq!(detect_cache_line_size(), CacheLineSize::STANDARD_64);
    assert_eq!(recommended_hot_alignment(), 64);
}

#[cfg(target_arch = "riscv64")]
#[test]
fn test_riscv_cache_line_size() {
    assert_eq!(detect_cache_line_size(), CacheLineSize::STANDARD_64);
    assert_eq!(recommended_hot_alignment(), 64);
}

#[cfg(target_arch = "powerpc64")]
#[test]
fn test_powerpc_cache_line_size() {
    assert_eq!(detect_cache_line_size(), CacheLineSize::EXTENDED_128);
    assert_eq!(recommended_hot_alignment(), 128);
}

// ============================================================================
// CONST VS RUNTIME CONSISTENCY TESTS
// ============================================================================

#[test]
fn test_const_matches_runtime_common_arch() {
    // On common architectures (x86/ARM/RISC-V), const should match runtime
    #[cfg(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64"
    ))]
    {
        let runtime = detect_cache_line_size().size();
        let const_eval = recommended_hot_alignment();

        assert_eq!(
            const_eval, runtime,
            "Const eval ({}) doesn't match runtime ({})",
            const_eval, runtime
        );
    }
}

#[test]
fn test_warm_alignment_relationship() {
    // Warm should always be 2× hot
    let hot = recommended_hot_alignment();
    let warm = recommended_warm_alignment();

    assert_eq!(warm, hot * 2);
}

#[test]
fn test_cold_alignment_relationship() {
    // Cold should always be 4× hot
    let hot = recommended_hot_alignment();
    let cold = recommended_cold_alignment();

    assert_eq!(cold, hot * 4);
}

// ============================================================================
// BACKWARD COMPATIBILITY TESTS
// ============================================================================

#[test]
fn test_detect_cache_line_size_api_unchanged() {
    // Old API should still work
    let size: CacheLineSize = detect_cache_line_size();
    let size_bytes: usize = size.size();

    assert!(size_bytes > 0);
}

#[test]
fn test_cache_line_size_new_validation() {
    // Should validate power of 2
    let size = CacheLineSize::new(64);
    assert_eq!(size.size(), 64);

    let size = CacheLineSize::new(128);
    assert_eq!(size.size(), 128);
}

#[test]
#[should_panic(expected = "power of 2")]
fn test_cache_line_size_new_rejects_invalid() {
    // Should panic on non-power-of-2
    let _ = CacheLineSize::new(65);
}

#[test]
#[should_panic(expected = ">= 64")]
fn test_cache_line_size_new_rejects_too_small() {
    let _ = CacheLineSize::new(32);
}

#[test]
#[should_panic(expected = "<= 256")]
fn test_cache_line_size_new_rejects_too_large() {
    let _ = CacheLineSize::new(512);
}

// ============================================================================
// PROPERTY-BASED TESTS
// ============================================================================

#[test]
fn test_all_alignments_powers_of_two() {
    let alignments = [
        recommended_hot_alignment(),
        recommended_warm_alignment(),
        recommended_cold_alignment(),
    ];

    for alignment in alignments {
        assert_eq!(
            alignment.count_ones(),
            1,
            "Alignment {} is not power of 2",
            alignment
        );
    }
}

#[test]
fn test_alignment_ordering() {
    let hot = recommended_hot_alignment();
    let warm = recommended_warm_alignment();
    let cold = recommended_cold_alignment();

    // Should be strictly increasing
    assert!(hot < warm);
    assert!(warm < cold);
}

#[test]
fn test_repeated_calls_identical() {
    // Const functions should be deterministic
    for _ in 0..100 {
        assert_eq!(recommended_hot_alignment(), recommended_hot_alignment());
        assert_eq!(recommended_warm_alignment(), recommended_warm_alignment());
        assert_eq!(recommended_cold_alignment(), recommended_cold_alignment());
    }
}

#[test]
fn test_detect_never_panics() {
    // Detection should be robust
    for _ in 0..100 {
        let _ = detect_cache_line_size();
    }
}

// ============================================================================
// PERFORMANCE CHARACTERISTICS TESTS
// ============================================================================

#[test]
fn test_const_eval_compiles_to_constant() {
    // This test verifies that const functions truly evaluate at compile-time
    // by using them in const context
    const _: [u8; recommended_hot_alignment()] = [0; recommended_hot_alignment()];
    const _: [u8; recommended_warm_alignment()] = [0; recommended_warm_alignment()];
    const _: [u8; recommended_cold_alignment()] = [0; recommended_cold_alignment()];

    // If this compiles, const eval is working
}

#[test]
fn test_cache_initialization_idempotent() {
    // Multiple initializations should be safe (OnceLock property)
    let first = detect_cache_line_size();

    // Force potential re-initialization attempts
    for _ in 0..10 {
        let current = detect_cache_line_size();
        assert_eq!(current, first);
    }
}
