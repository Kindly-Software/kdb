//! # Architecture Detection Comprehensive Tests
//!
//! **T28 Framework**: Q1-Q7 (Unit) + Q8-Q14 (Property)
//! **ASSUM Verification**: #ASSUME_CACHE_LINE_DETECTION → #VERIFY
//!
//! Tests CPU architecture detection for cache line sizes across platforms:
//! - x86/x86_64: 64B standard
//! - ARM/AArch64: 64B standard (with 128B possible)
//! - RISC-V: 64B standard
//! - PowerPC: 128B extended
//! - Unknown: 64B fallback
//!
//! **Coverage Goal**: arch.rs 20% → 80%

use atomic_capsule::{
    detect_cache_line_size, recommended_hot_alignment, recommended_warm_alignment, CacheLineSize,
};

// =============================================================================
// T28 Q1: Core Behaviors Tested
// =============================================================================

#[test]
fn test_detect_cache_line_size_returns_valid_power_of_2() {
    let size = detect_cache_line_size();

    // #VERIFY: Cache line size is power of 2
    assert!(
        size.size().is_power_of_two(),
        "Cache line size must be power of 2: {}",
        size.size()
    );
}

#[test]
fn test_cache_line_size_in_valid_range() {
    let size = detect_cache_line_size();

    // #VERIFY: Cache line size in [64, 256] range
    assert!(
        size.size() >= 64,
        "Cache line size too small: {}",
        size.size()
    );
    assert!(
        size.size() <= 256,
        "Cache line size too large: {}",
        size.size()
    );
}

#[test]
fn test_recommended_hot_alignment() {
    let alignment = recommended_hot_alignment();

    // Hot alignment should equal cache line size
    assert_eq!(alignment, detect_cache_line_size().size());
    assert!(alignment.is_power_of_two());
    assert!(alignment >= 64);
}

#[test]
fn test_recommended_warm_alignment() {
    let alignment = recommended_warm_alignment();
    let cache_size = detect_cache_line_size().size();

    // Warm alignment should be 2× cache line size
    assert_eq!(alignment, cache_size * 2);
    assert!(alignment.is_power_of_two());
    assert!(alignment >= 128);
}

// =============================================================================
// T28 Q2: Edge Cases
// =============================================================================

#[test]
fn test_cache_line_size_constants() {
    // Standard 64B
    assert_eq!(CacheLineSize::STANDARD_64.size(), 64);

    // Extended 128B
    assert_eq!(CacheLineSize::EXTENDED_128.size(), 128);

    // Large 256B
    assert_eq!(CacheLineSize::LARGE_256.size(), 256);
}

#[test]
fn test_cache_line_size_default() {
    let default_size = CacheLineSize::default();
    assert_eq!(default_size.size(), 64, "Default should be 64B");
}

#[test]
fn test_cache_line_size_new_valid_sizes() {
    // Valid power-of-2 sizes in range
    let size64 = CacheLineSize::new(64);
    assert_eq!(size64.size(), 64);

    let size128 = CacheLineSize::new(128);
    assert_eq!(size128.size(), 128);

    let size256 = CacheLineSize::new(256);
    assert_eq!(size256.size(), 256);
}

#[test]
#[should_panic(expected = "Cache line size must be power of 2")]
fn test_cache_line_size_new_non_power_of_2() {
    let _invalid = CacheLineSize::new(65); // Not power of 2
}

#[test]
#[should_panic(expected = "Cache line size must be >= 64")]
fn test_cache_line_size_new_too_small() {
    let _invalid = CacheLineSize::new(32); // Below minimum
}

#[test]
#[should_panic(expected = "Cache line size must be <= 256")]
fn test_cache_line_size_new_too_large() {
    let _invalid = CacheLineSize::new(512); // Above maximum
}

// =============================================================================
// T28 Q3: Invariants Validated
// =============================================================================

#[test]
fn test_invariant_hot_alignment_matches_cache_size() {
    // Invariant: Hot alignment must equal cache line size
    let cache_size = detect_cache_line_size().size();
    let hot_alignment = recommended_hot_alignment();

    assert_eq!(
        hot_alignment, cache_size,
        "Hot alignment must match cache line size"
    );
}

#[test]
fn test_invariant_warm_alignment_double_cache_size() {
    // Invariant: Warm alignment must be 2× cache line size
    let cache_size = detect_cache_line_size().size();
    let warm_alignment = recommended_warm_alignment();

    assert_eq!(
        warm_alignment,
        cache_size * 2,
        "Warm alignment must be 2× cache line size"
    );
}

#[test]
fn test_invariant_alignment_hierarchy() {
    // Invariant: Warm alignment >= Hot alignment
    let hot = recommended_hot_alignment();
    let warm = recommended_warm_alignment();

    assert!(
        warm >= hot,
        "Warm alignment must be >= hot alignment: warm={}, hot={}",
        warm,
        hot
    );
}

// =============================================================================
// T28 Q4: Platform-Specific Detection Tests
// =============================================================================

#[test]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn test_x86_cache_line_detection() {
    let size = detect_cache_line_size();

    // x86/x86_64 typically use 64B cache lines
    assert_eq!(size.size(), 64, "x86/x86_64 should detect 64B cache lines");
}

#[test]
#[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
fn test_arm_cache_line_detection() {
    let size = detect_cache_line_size();

    // ARM typically uses 64B, but can be 128B
    assert!(
        size.size() == 64 || size.size() == 128,
        "ARM should detect 64B or 128B cache lines: {}",
        size.size()
    );
}

#[test]
#[cfg(target_arch = "riscv64")]
fn test_riscv_cache_line_detection() {
    let size = detect_cache_line_size();

    // RISC-V typically uses 64B cache lines
    assert_eq!(size.size(), 64, "RISC-V should detect 64B cache lines");
}

#[test]
#[cfg(target_arch = "powerpc64")]
fn test_powerpc_cache_line_detection() {
    let size = detect_cache_line_size();

    // PowerPC typically uses 128B cache lines
    assert_eq!(size.size(), 128, "PowerPC should detect 128B cache lines");
}

#[test]
#[cfg(not(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "arm",
    target_arch = "riscv64",
    target_arch = "powerpc64"
)))]
fn test_unknown_arch_fallback_to_64b() {
    let size = detect_cache_line_size();

    // Unknown architectures should default to 64B
    assert_eq!(
        size.size(),
        64,
        "Unknown architectures should fall back to 64B"
    );
}

// =============================================================================
// T28 Q5: Tests Isolated and Deterministic
// =============================================================================

#[test]
fn test_detection_is_deterministic() {
    // Detection should return the same value every time
    let size1 = detect_cache_line_size();
    let size2 = detect_cache_line_size();
    let size3 = detect_cache_line_size();

    assert_eq!(size1.size(), size2.size());
    assert_eq!(size2.size(), size3.size());
}

#[test]
fn test_detection_is_thread_safe() {
    use std::sync::Arc;
    use std::thread;

    let detected_size = detect_cache_line_size().size();
    let mut handles = vec![];

    // Spawn 10 threads, all should see same cache line size
    for _ in 0..10 {
        handles.push(thread::spawn(move || {
            let size = detect_cache_line_size();
            size.size()
        }));
    }

    for handle in handles {
        let thread_size = handle.join().unwrap();
        assert_eq!(
            thread_size, detected_size,
            "Cache line size must be consistent across threads"
        );
    }
}

// =============================================================================
// T28 Q8-Q14: Property Testing (Invariants)
// =============================================================================

#[test]
fn property_cache_size_always_valid() {
    // Property: Detection always returns valid size
    for _ in 0..100 {
        let size = detect_cache_line_size();

        // Must be power of 2
        assert!(size.size().is_power_of_two());

        // Must be in valid range
        assert!(size.size() >= 64);
        assert!(size.size() <= 256);
    }
}

#[test]
fn property_alignments_always_valid() {
    // Property: Recommended alignments always valid
    for _ in 0..100 {
        let hot = recommended_hot_alignment();
        let warm = recommended_warm_alignment();

        // Hot alignment valid
        assert!(hot.is_power_of_two());
        assert!(hot >= 64);
        assert!(hot <= 256);

        // Warm alignment valid
        assert!(warm.is_power_of_two());
        assert!(warm >= 128);
        assert!(warm <= 512);

        // Warm >= Hot
        assert!(warm >= hot);
    }
}

// =============================================================================
// ASSUM Framework Verification
// =============================================================================

#[test]
fn verify_assum_cache_line_detection() {
    // #ASSUME_CACHE_LINE_DETECTION: Architecture detection returns valid cache line size
    // #VERIFY: All detected sizes are power-of-2 and in [64, 256] range

    let size = detect_cache_line_size();

    // Verification 1: Power of 2
    assert!(
        size.size().is_power_of_two(),
        "#VERIFY failed: Cache line size not power of 2: {}",
        size.size()
    );

    // Verification 2: Valid range
    assert!(
        size.size() >= 64 && size.size() <= 256,
        "#VERIFY failed: Cache line size out of range: {}",
        size.size()
    );
}

#[test]
fn verify_assum_alignment_recommendations() {
    // #ASSUME_ALIGNMENT_SUFFICIENT: Recommended alignments prevent false sharing
    // #VERIFY: Hot >= 64B, Warm >= 128B

    let hot = recommended_hot_alignment();
    let warm = recommended_warm_alignment();

    // Verification 1: Hot alignment sufficient
    assert!(
        hot >= 64,
        "#VERIFY failed: Hot alignment too small: {}",
        hot
    );

    // Verification 2: Warm alignment sufficient
    assert!(
        warm >= 128,
        "#VERIFY failed: Warm alignment too small: {}",
        warm
    );

    // Verification 3: Warm >= Hot
    assert!(
        warm >= hot,
        "#VERIFY failed: Warm alignment smaller than hot: warm={}, hot={}",
        warm,
        hot
    );
}

// =============================================================================
// Integration Tests
// =============================================================================

#[test]
fn test_cache_line_size_copy_clone_eq() {
    let size1 = CacheLineSize::new(64);
    let size2 = size1; // Copy
    let size3 = size1.clone(); // Clone

    assert_eq!(size1, size2);
    assert_eq!(size2, size3);
}

#[test]
fn test_cache_line_size_debug_format() {
    let size = CacheLineSize::new(128);
    let debug_str = format!("{:?}", size);

    // Debug output should contain the size
    assert!(debug_str.contains("128"));
}

// =============================================================================
// Performance Considerations (Documented)
// =============================================================================

#[test]
fn test_detection_overhead_acceptable() {
    use std::time::Instant;

    // Measure detection overhead
    let start = Instant::now();
    for _ in 0..10_000 {
        let _ = detect_cache_line_size();
    }
    let elapsed = start.elapsed();

    // Should complete 10K detections in <1ms (average <100ns each)
    assert!(
        elapsed.as_millis() < 1,
        "Cache line detection too slow: {:?}",
        elapsed
    );
}
