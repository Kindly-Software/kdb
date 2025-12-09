//! Integration tests for architecture detection.
//!
//! Validates CPU cache line size detection works correctly.

use atomic_capsule::{
    detect_cache_line_size, recommended_hot_alignment, recommended_warm_alignment, CacheLineSize,
};

#[test]
fn test_cache_line_size_constants() {
    assert_eq!(CacheLineSize::STANDARD_64.size(), 64);
    assert_eq!(CacheLineSize::EXTENDED_128.size(), 128);
    assert_eq!(CacheLineSize::LARGE_256.size(), 256);
}

#[test]
fn test_cache_line_size_new() {
    let size64 = CacheLineSize::new(64);
    assert_eq!(size64.size(), 64);

    let size128 = CacheLineSize::new(128);
    assert_eq!(size128.size(), 128);

    let size256 = CacheLineSize::new(256);
    assert_eq!(size256.size(), 256);
}

#[test]
#[should_panic(expected = "power of 2")]
fn test_cache_line_size_invalid_not_pow2() {
    let _ = CacheLineSize::new(100);
}

#[test]
#[should_panic(expected = "must be >= 64")]
fn test_cache_line_size_too_small() {
    let _ = CacheLineSize::new(32);
}

#[test]
#[should_panic(expected = "must be <= 256")]
fn test_cache_line_size_too_large() {
    let _ = CacheLineSize::new(512);
}

#[test]
fn test_default_cache_line_size() {
    let default = CacheLineSize::default();
    assert_eq!(default.size(), 64);
}

#[test]
fn test_detect_cache_line_size() {
    let detected = detect_cache_line_size();

    // Should return a valid cache line size
    assert!(detected.size() >= 64);
    assert!(detected.size() <= 256);

    // Should be power of 2
    assert_eq!(detected.size().count_ones(), 1);
}

#[test]
fn test_detect_returns_known_values() {
    let detected = detect_cache_line_size();

    // Should be one of the known cache line sizes
    let is_known = detected.size() == 64 || detected.size() == 128 || detected.size() == 256;

    assert!(
        is_known,
        "Detected cache line size should be 64, 128, or 256, got {}",
        detected.size()
    );
}

#[test]
fn test_recommended_hot_alignment() {
    let alignment = recommended_hot_alignment();

    // Should match detected cache line size
    assert_eq!(alignment, detect_cache_line_size().size());

    // Should be valid alignment
    assert!(alignment >= 64);
    assert!(alignment <= 256);
    assert_eq!(alignment.count_ones(), 1);
}

#[test]
fn test_recommended_warm_alignment() {
    let alignment = recommended_warm_alignment();

    // Should be 2x hot alignment
    assert_eq!(alignment, recommended_hot_alignment() * 2);

    // Should be valid alignment
    assert!(alignment >= 128);
    assert!(alignment <= 512);
    assert_eq!(alignment.count_ones(), 1);
}

/// Test architecture-specific detection on known platforms
#[test]
fn test_architecture_specific_detection() {
    let detected = detect_cache_line_size();

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // Intel/AMD should return 64 bytes
        assert_eq!(
            detected.size(),
            64,
            "x86/x86_64 should have 64-byte cache lines"
        );
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    {
        // ARM should return 64 bytes
        assert_eq!(
            detected.size(),
            64,
            "ARM/AArch64 should have 64-byte cache lines"
        );
    }

    #[cfg(target_arch = "riscv64")]
    {
        // RISC-V should return 64 bytes
        assert_eq!(
            detected.size(),
            64,
            "RISC-V should have 64-byte cache lines"
        );
    }

    #[cfg(target_arch = "powerpc64")]
    {
        // PowerPC should return 128 bytes
        assert_eq!(
            detected.size(),
            128,
            "PowerPC should have 128-byte cache lines"
        );
    }
}

/// Test that detection is consistent across multiple calls
#[test]
fn test_detection_consistency() {
    let first = detect_cache_line_size();
    let second = detect_cache_line_size();
    let third = detect_cache_line_size();

    assert_eq!(first.size(), second.size());
    assert_eq!(second.size(), third.size());
}

/// Test cache line size equality
#[test]
fn test_cache_line_size_equality() {
    let size1 = CacheLineSize::new(64);
    let size2 = CacheLineSize::new(64);
    let size3 = CacheLineSize::new(128);

    assert_eq!(size1, size2);
    assert_ne!(size1, size3);
}

/// Test that recommended alignments are suitable for repr(align)
#[test]
fn test_recommended_alignments_valid_for_repr() {
    let hot = recommended_hot_alignment();
    let warm = recommended_warm_alignment();

    // Both should be valid repr(align) values (powers of 2)
    assert_eq!(hot.count_ones(), 1);
    assert_eq!(warm.count_ones(), 1);

    // Both should be within reasonable bounds
    assert!((64..=256).contains(&hot));
    assert!((128..=512).contains(&warm));
}

/// Property test: All valid cache line sizes are powers of 2 in [64, 256]
#[test]
fn test_valid_cache_line_sizes_property() {
    for size in [64, 128, 256] {
        let cache_line = CacheLineSize::new(size);

        assert_eq!(cache_line.size(), size);
        assert_eq!(cache_line.size().count_ones(), 1);
        assert!(cache_line.size() >= 64);
        assert!(cache_line.size() <= 256);
    }
}

/// Test that detect works in const context (compile-time)
#[test]
fn test_const_evaluation() {
    const DEFAULT: CacheLineSize = CacheLineSize::STANDARD_64;
    assert_eq!(DEFAULT.size(), 64);

    const SIZE_128: CacheLineSize = CacheLineSize::EXTENDED_128;
    assert_eq!(SIZE_128.size(), 128);
}
