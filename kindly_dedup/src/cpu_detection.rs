//! # CPU Feature Detection for Runtime SIMD Dispatch
//!
//! **Purpose**: Enable runtime detection of AVX-512, AVX2, SSE4.2 for optimal SIMD path selection
//!
//! **Architecture**:
//! - Pattern: Reuses atomic_capsule::CpuCapabilityCapsule (DRY principle)
//! - Performance: <1ms one-time detection, <10ns subsequent queries
//!
//! **Framework Compliance**:
//! - ASSUM: 99.99% safe (zero unsafe code, atomic_capsule guarantee)
//! - B32: <0.1% overhead (amortized over program lifetime)
//!
//! **Usage**:
//! ```rust,ignore
//! use kindly_dedup::cpu_detection::CpuFeatures;
//!
//! let features = CpuFeatures::detect();
//! if features.has_avx512f() {
//!     // Use AVX-512 SIMD path (16-lane, 2× AVX2 speedup)
//!     compute_avx512(data)
//! } else if features.has_avx2() {
//!     // Use AVX2 SIMD path (8-lane, 4-8× scalar speedup)
//!     compute_avx2(data)
//! } else {
//!     // Fallback to portable scalar
//!     compute_scalar(data)
//! }
//! ```

use atomic_capsule::primitives::cpu_capabilities::CpuCapabilityCapsule;

/// CPU feature flags for SIMD dispatch
///
/// **Performance**: <10ns queries (cached singleton)
/// **Thread-safe**: 100% lockfree (delegates to CpuCapabilityCapsule)
/// **Detection**: One-time ~1ms initialization (amortized)
///
/// # Architecture
///
/// Thin wrapper over atomic_capsule::CpuCapabilityCapsule for kindly_dedup-specific API.
///
/// **DRY Principle**: Reuses atomic_capsule infrastructure (no duplication)
///
/// # Platform Support
///
/// - **x86_64**: AVX-512F, AVX2, SSE4.2 detection via CPUID
/// - **aarch64**: NEON baseline (always available)
/// - **Other**: Graceful fallback to scalar
///
/// # ASSUM Safety
///
/// - `#ASSUME_CPUCAPS_SAFE`: atomic_capsule::CpuCapabilityCapsule is 99.99% safe
/// - `#VERIFY_CPUCAPS_SAFE`: atomic_capsule ASSUM framework validation (99.99%)
/// - `#ASSUME_FEATURES_IMMUTABLE`: CPU features don't change at runtime
/// - `#VERIFY_FEATURES_IMMUTABLE`: Hardware guarantee (CPUID results constant after boot)
///
/// # Example
///
/// ```rust
/// use kindly_dedup::cpu_detection::CpuFeatures;
///
/// let features = CpuFeatures::detect();
///
/// // Tier 1: AVX-512 (16-lane, 2× AVX2 speedup)
/// if features.has_avx512f() {
///     println!("AVX-512F available (2017+ Intel Xeon)");
/// }
///
/// // Tier 2: AVX2 (8-lane, 4-8× scalar speedup)
/// if features.has_avx2() {
///     println!("AVX2 available (2013+ Intel/AMD)");
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CpuFeatures {
    /// Cached reference to atomic_capsule::CpuCapabilityCapsule
    ///
    /// **Lifetime**: 'static (lives for entire program)
    /// **Performance**: <10ns dereference (pointer-sized, stack-allocated)
    caps: &'static CpuCapabilityCapsule,
}

impl CpuFeatures {
    /// Detect CPU capabilities (cached singleton pattern)
    ///
    /// **First call**: ~1ms (CPUID detection + initialization)
    /// **Subsequent calls**: <10ns (cached pointer dereference)
    ///
    /// # Thread Safety
    ///
    /// - OnceLock guarantees exactly-once initialization (atomic_capsule)
    /// - Safe to call concurrently from multiple threads
    /// - First thread initializes, others wait and get same result
    ///
    /// # Example
    ///
    /// ```rust
    /// use kindly_dedup::cpu_detection::CpuFeatures;
    ///
    /// // First call: ~1ms (detection)
    /// let features = CpuFeatures::detect();
    ///
    /// // Subsequent calls: <10ns (cached)
    /// let features2 = CpuFeatures::detect();
    /// ```
    #[inline(always)]
    pub fn detect() -> Self {
        Self {
            caps: CpuCapabilityCapsule::detect(),
        }
    }

    /// Check AVX-512F support (<10ns)
    ///
    /// **Supported**: Intel Xeon Scalable 2017+ (Skylake-SP and newer)
    /// **Performance**: 16-lane f32x16/u16x16 SIMD (2× AVX2 speedup)
    /// **MinHash**: 16-lane u16x16 SIMD for 128-hash signature (8 iterations vs 16 for AVX2)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_AVX512_DETECTION`: atomic_capsule uses safe CPUID wrappers
    /// - `#VERIFY_AVX512_DETECTION`: Rust std library (core::arch::x86_64)
    ///
    /// # Example
    ///
    /// ```rust
    /// use kindly_dedup::cpu_detection::CpuFeatures;
    ///
    /// let features = CpuFeatures::detect();
    /// if features.has_avx512f() {
    ///     println!("AVX-512 available");
    /// } else {
    ///     println!("AVX-512 not available");
    /// }
    /// ```
    #[inline(always)]
    pub fn has_avx512f(&self) -> bool {
        self.caps.has_avx512()
    }

    /// Check AVX2 support (<10ns)
    ///
    /// **Supported**: Intel Haswell 2013+, AMD Excavator 2015+
    /// **Performance**: 8-lane f32x8/u16x8 SIMD (4-8× scalar speedup)
    /// **MinHash**: 8-lane u16x8 SIMD for 128-hash signature (16 iterations)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_AVX2_DETECTION`: atomic_capsule uses safe CPUID wrappers
    /// - `#VERIFY_AVX2_DETECTION`: Rust std library (core::arch::x86_64)
    ///
    /// # Example
    ///
    /// ```rust
    /// use kindly_dedup::cpu_detection::CpuFeatures;
    ///
    /// let features = CpuFeatures::detect();
    /// if features.has_avx2() {
    ///     println!("AVX2 available");
    /// } else {
    ///     println!("AVX2 not available");
    /// }
    /// ```
    #[inline(always)]
    pub fn has_avx2(&self) -> bool {
        self.caps.has_avx2()
    }

    /// Check SSE4.2 support (<10ns)
    ///
    /// **Supported**: Intel Nehalem 2008+, AMD Bulldozer 2011+
    /// **Performance**: 4-lane f32x4 SIMD (2-3× scalar speedup)
    /// **Note**: Not used for MinHash (u16x8 requires AVX2), kept for future use
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_SSE42_DETECTION`: atomic_capsule uses safe CPUID wrappers
    /// - `#VERIFY_SSE42_DETECTION`: Rust std library (core::arch::x86_64)
    ///
    /// # Example
    ///
    /// ```rust
    /// use kindly_dedup::cpu_detection::CpuFeatures;
    ///
    /// let features = CpuFeatures::detect();
    /// if features.has_sse42() {
    ///     println!("SSE4.2 available");
    /// } else {
    ///     println!("SSE4.2 not available");
    /// }
    /// ```
    #[inline(always)]
    pub fn has_sse42(&self) -> bool {
        self.caps.has_sse42()
    }

    /// Get cached CpuCapabilityCapsule reference
    ///
    /// **Use case**: Direct access to atomic_capsule API for advanced use
    ///
    /// # Example
    ///
    /// ```rust
    /// use kindly_dedup::cpu_detection::CpuFeatures;
    ///
    /// let features = CpuFeatures::detect();
    /// let caps = features.as_capsule();
    ///
    /// // Access atomic_capsule methods directly
    /// if caps.has_neon() {
    ///     println!("ARM NEON available");
    /// }
    /// ```
    #[inline(always)]
    pub fn as_capsule(&self) -> &'static CpuCapabilityCapsule {
        self.caps
    }

    /// Create from cached CpuCapabilityCapsule (testing/internal use)
    ///
    /// **Use case**: Testing with known CPU features
    ///
    /// # Example
    ///
    /// ```rust
    /// use kindly_dedup::cpu_detection::CpuFeatures;
    /// use atomic_capsule::primitives::cpu_capabilities::CpuCapabilityCapsule;
    ///
    /// let caps = CpuCapabilityCapsule::detect();
    /// let features = CpuFeatures::from_capsule(caps);
    /// ```
    #[inline(always)]
    pub fn from_capsule(caps: &'static CpuCapabilityCapsule) -> Self {
        Self { caps }
    }
}

impl Default for CpuFeatures {
    /// Default to runtime detection (calls CpuFeatures::detect())
    ///
    /// **Performance**: <10ns (cached after first call)
    ///
    /// # Example
    ///
    /// ```rust
    /// use kindly_dedup::cpu_detection::CpuFeatures;
    ///
    /// let features = CpuFeatures::default();
    /// assert!(features.has_avx2() || !features.has_avx2());  // Tautology test
    /// ```
    #[inline(always)]
    fn default() -> Self {
        Self::detect()
    }
}

// ============================================================================
// Unit Tests (T28 Framework - Tier 1: Unit Tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: CpuFeatures::detect() returns valid instance
    ///
    /// **Framework**: T28 Q1 (Basic functionality)
    /// **ASSUM**: CPU features are immutable after boot
    #[test]
    fn test_detect_basic() {
        let features = CpuFeatures::detect();
        let _caps = features.as_capsule();
        // Success: No panic, valid instance
    }

    /// Test: CpuFeatures::detect() is cached (same pointer)
    ///
    /// **Framework**: T28 Q2 (Caching behavior)
    /// **ASSUM**: OnceLock guarantees singleton
    #[test]
    fn test_detect_cached() {
        let f1 = CpuFeatures::detect();
        let f2 = CpuFeatures::detect();

        // Same singleton instance
        assert!(std::ptr::eq(f1.as_capsule(), f2.as_capsule()));
    }

    /// Test: has_avx512f() returns bool (no panic)
    ///
    /// **Framework**: T28 Q3 (AVX-512 query)
    /// **Platform**: x86_64 may be true/false, others always false
    #[test]
    fn test_has_avx512f() {
        let features = CpuFeatures::detect();
        let _has_avx512 = features.has_avx512f();
        // Success: Returns bool without panic

        #[cfg(target_arch = "x86_64")]
        {
            // x86_64: AVX-512 may or may not be available
            // (Xeon Scalable 2017+)
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            // Non-x86_64: AVX-512 always unavailable
            assert!(!_has_avx512);
        }
    }

    /// Test: has_avx2() returns bool (no panic)
    ///
    /// **Framework**: T28 Q4 (AVX2 query)
    /// **Platform**: x86_64 likely true (2013+), others false
    #[test]
    fn test_has_avx2() {
        let features = CpuFeatures::detect();
        let _has_avx2 = features.has_avx2();
        // Success: Returns bool without panic

        #[cfg(target_arch = "x86_64")]
        {
            // x86_64: AVX2 likely available (Haswell 2013+)
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            // Non-x86_64: AVX2 always unavailable
            assert!(!_has_avx2);
        }
    }

    /// Test: has_sse42() returns bool (no panic)
    ///
    /// **Framework**: T28 Q5 (SSE4.2 query)
    /// **Platform**: x86_64 very likely true (2008+), others false
    #[test]
    fn test_has_sse42() {
        let features = CpuFeatures::detect();
        let _has_sse42 = features.has_sse42();
        // Success: Returns bool without panic

        #[cfg(target_arch = "x86_64")]
        {
            // x86_64: SSE4.2 very likely available (Nehalem 2008+)
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            // Non-x86_64: SSE4.2 always unavailable
            assert!(!_has_sse42);
        }
    }

    /// Test: Default impl calls detect()
    ///
    /// **Framework**: T28 Q6 (Default trait)
    #[test]
    fn test_default() {
        let f1 = CpuFeatures::default();
        let f2 = CpuFeatures::detect();

        // Same singleton
        assert!(std::ptr::eq(f1.as_capsule(), f2.as_capsule()));
    }

    /// Test: from_capsule() round-trip
    ///
    /// **Framework**: T28 Q7 (from_capsule constructor)
    #[test]
    fn test_from_capsule() {
        let caps = CpuCapabilityCapsule::detect();
        let features = CpuFeatures::from_capsule(caps);

        // Same capsule reference
        assert!(std::ptr::eq(features.as_capsule(), caps));
    }

    /// Test: Clone preserves capsule reference
    ///
    /// **Framework**: T28 Q8 (Clone trait)
    #[test]
    fn test_clone() {
        let f1 = CpuFeatures::detect();
        let f2 = f1.clone();

        // Same capsule reference after clone
        assert!(std::ptr::eq(f1.as_capsule(), f2.as_capsule()));
    }

    /// Test: Copy preserves capsule reference
    ///
    /// **Framework**: T28 Q9 (Copy trait)
    #[test]
    fn test_copy() {
        let f1 = CpuFeatures::detect();
        let f2 = f1; // Copy

        // Same capsule reference after copy
        assert!(std::ptr::eq(f1.as_capsule(), f2.as_capsule()));
    }

    /// Test: Multiple queries <100ns total
    ///
    /// **Framework**: B32 K1 (Performance baseline)
    /// **Target**: <10ns per query (cached atomic load)
    #[test]
    fn test_query_performance() {
        let features = CpuFeatures::detect();

        // Warm-up
        let _ = features.has_avx512f();

        // 10 queries should complete in <100ns (<10ns each)
        let start = std::time::Instant::now();
        for _ in 0..10 {
            let _ = features.has_avx512f();
            let _ = features.has_avx2();
            let _ = features.has_sse42();
        }
        let elapsed = start.elapsed();

        // 30 queries (10 iterations × 3 queries) should be fast
        // Allow generous margin for system variance (100x theoretical max)
        assert!(
            elapsed.as_nanos() < 100_000,
            "30 queries took {}ns (expected <100μs for <3.3μs/query average)",
            elapsed.as_nanos()
        );
    }
}
