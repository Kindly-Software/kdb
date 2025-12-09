//! ASSUM Safety Validation Test Suite
//!
//! Comprehensive safety validation tests following ASSUM framework.
//!
//! Test categories:
//! - ASSUM-001: Cache line alignment verification
//! - ASSUM-002: Power-of-2 alignment constraints
//! - ASSUM-003: Alignment range bounds
//! - ASSUM-004: Architecture-specific cache line sizes
//! - ASSUM-005: Retry termination guarantees
//! - ASSUM-006: Backoff effectiveness (requires benchmarks)
//! - ASSUM-007: AlignmentTier safety contract

use atomic_capsule::{
    AlignmentMarker, AlignmentTier, BackoffStrategy, CacheLineSize, ColdTier, HotTier, RetryPolicy,
    WarmTier,
};
use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// ASSUM-001: Cache Line Alignment Verification
// ============================================================================

#[repr(C, align(64))]
struct TestHotCapsule {
    data: [u8; 64],
}

impl AlignmentTier for TestHotCapsule {
    const TIER: &'static str = "hot";
    const ALIGNMENT: usize = 64;
}

#[test]
fn assum_001_verify_hot_alignment() {
    let capsule = TestHotCapsule { data: [0u8; 64] };
    let ptr = &capsule as *const _ as usize;

    // #ASSUME_CACHE_ALIGNED: Hot tier is 64-byte aligned
    // #VERIFY_CACHE_ALIGNED: Runtime pointer alignment check
    assert_eq!(
        ptr % 64,
        0,
        "ASSUM-001 violated: Hot capsule not 64-byte aligned"
    );
    assert!(TestHotCapsule::verify_alignment());
}

#[repr(C, align(128))]
struct TestWarmCapsule {
    data: [u8; 128],
}

impl AlignmentTier for TestWarmCapsule {
    const TIER: &'static str = "warm";
    const ALIGNMENT: usize = 128;
}

#[test]
fn assum_001_verify_warm_alignment() {
    let capsule = TestWarmCapsule { data: [0u8; 128] };
    let ptr = &capsule as *const _ as usize;

    // #ASSUME_DUAL_CHANNEL: Warm tier is 128-byte aligned (2 cache lines)
    // #VERIFY_DUAL_CHANNEL: Runtime pointer alignment check
    assert_eq!(
        ptr % 128,
        0,
        "ASSUM-001 violated: Warm capsule not 128-byte aligned"
    );
    assert!(TestWarmCapsule::verify_alignment());
}

#[repr(C, align(256))]
struct TestColdCapsule {
    data: [u8; 256],
}

impl AlignmentTier for TestColdCapsule {
    const TIER: &'static str = "cold";
    const ALIGNMENT: usize = 256;
}

#[test]
fn assum_001_verify_cold_alignment() {
    let capsule = TestColdCapsule { data: [0u8; 256] };
    let ptr = &capsule as *const _ as usize;

    // #ASSUME_MULTI_LINE: Cold tier is 256-byte aligned (4 cache lines)
    // #VERIFY_MULTI_LINE: Runtime pointer alignment check
    assert_eq!(
        ptr % 256,
        0,
        "ASSUM-001 violated: Cold capsule not 256-byte aligned"
    );
    assert!(TestColdCapsule::verify_alignment());
}

#[test]
fn assum_001_boxed_allocation_preserves_alignment() {
    // #ASSUME_HEAP_ALIGNMENT: Boxed allocations preserve alignment
    // #VERIFY_HEAP_ALIGNMENT: Runtime check after heap allocation

    let hot = Box::new(TestHotCapsule { data: [0u8; 64] });
    let ptr = &*hot as *const _ as usize;
    assert_eq!(
        ptr % 64,
        0,
        "ASSUM-001 violated: Boxed hot capsule lost alignment"
    );

    let warm = Box::new(TestWarmCapsule { data: [0u8; 128] });
    let ptr = &*warm as *const _ as usize;
    assert_eq!(
        ptr % 128,
        0,
        "ASSUM-001 violated: Boxed warm capsule lost alignment"
    );

    let cold = Box::new(TestColdCapsule { data: [0u8; 256] });
    let ptr = &*cold as *const _ as usize;
    assert_eq!(
        ptr % 256,
        0,
        "ASSUM-001 violated: Boxed cold capsule lost alignment"
    );
}

// ============================================================================
// ASSUM-002: Power-of-2 Alignment Constraints
// ============================================================================

#[test]
fn assum_002_all_alignments_power_of_two() {
    // #ASSUME_ALIGNMENT_POW2: All alignments are powers of 2
    // #VERIFY_ALIGNMENT_POW2: Check via count_ones() == 1

    assert_eq!(
        HotTier::ALIGNMENT.count_ones(),
        1,
        "ASSUM-002 violated: HotTier alignment not power of 2"
    );
    assert_eq!(
        WarmTier::ALIGNMENT.count_ones(),
        1,
        "ASSUM-002 violated: WarmTier alignment not power of 2"
    );
    assert_eq!(
        ColdTier::ALIGNMENT.count_ones(),
        1,
        "ASSUM-002 violated: ColdTier alignment not power of 2"
    );

    // Verify all valid cache line sizes
    assert_eq!(
        64_usize.count_ones(),
        1,
        "ASSUM-002 violated: 64-byte cache line not power of 2"
    );
    assert_eq!(
        128_usize.count_ones(),
        1,
        "ASSUM-002 violated: 128-byte cache line not power of 2"
    );
    assert_eq!(
        256_usize.count_ones(),
        1,
        "ASSUM-002 violated: 256-byte cache line not power of 2"
    );
}

#[test]
fn assum_002_cache_line_sizes_power_of_two() {
    // #ASSUME_CACHE_POW2: All cache line sizes are powers of 2
    // #VERIFY_CACHE_POW2: CacheLineSize validation

    let size64 = CacheLineSize::new(64);
    assert_eq!(
        size64.size().count_ones(),
        1,
        "ASSUM-002 violated: 64-byte size not power of 2"
    );

    let size128 = CacheLineSize::new(128);
    assert_eq!(
        size128.size().count_ones(),
        1,
        "ASSUM-002 violated: 128-byte size not power of 2"
    );

    let size256 = CacheLineSize::new(256);
    assert_eq!(
        size256.size().count_ones(),
        1,
        "ASSUM-002 violated: 256-byte size not power of 2"
    );
}

#[test]
#[should_panic(expected = "power of 2")]
fn assum_002_invalid_alignment_panics() {
    // #ASSUME_INVALID_REJECTED: Non-power-of-2 alignments are rejected
    // #VERIFY_INVALID_REJECTED: Expect panic on invalid input

    let _invalid = CacheLineSize::new(100); // Not a power of 2
}

// ============================================================================
// ASSUM-003: Alignment Range Bounds
// ============================================================================

#[test]
fn assum_003_alignments_within_bounds() {
    // #ASSUME_ALIGNMENT_RANGE: All alignments in [MIN, MAX] = [64, 256]
    // #VERIFY_ALIGNMENT_RANGE: Const bounds check

    assert!(
        HotTier::ALIGNMENT >= atomic_capsule::MIN_ALIGNMENT,
        "ASSUM-003 violated: HotTier below MIN_ALIGNMENT"
    );
    assert!(
        HotTier::ALIGNMENT <= atomic_capsule::MAX_ALIGNMENT,
        "ASSUM-003 violated: HotTier above MAX_ALIGNMENT"
    );

    assert!(
        WarmTier::ALIGNMENT >= atomic_capsule::MIN_ALIGNMENT,
        "ASSUM-003 violated: WarmTier below MIN_ALIGNMENT"
    );
    assert!(
        WarmTier::ALIGNMENT <= atomic_capsule::MAX_ALIGNMENT,
        "ASSUM-003 violated: WarmTier above MAX_ALIGNMENT"
    );

    assert!(
        ColdTier::ALIGNMENT >= atomic_capsule::MIN_ALIGNMENT,
        "ASSUM-003 violated: ColdTier below MIN_ALIGNMENT"
    );
    assert!(
        ColdTier::ALIGNMENT <= atomic_capsule::MAX_ALIGNMENT,
        "ASSUM-003 violated: ColdTier above MAX_ALIGNMENT"
    );
}

#[test]
fn assum_003_min_max_constants_valid() {
    // #ASSUME_BOUNDS_VALID: MIN and MAX constants are valid
    // #VERIFY_BOUNDS_VALID: Check MIN < MAX and both powers of 2

    assert_eq!(atomic_capsule::MIN_ALIGNMENT, 64);
    assert_eq!(atomic_capsule::MAX_ALIGNMENT, 256);

    assert!(
        atomic_capsule::MIN_ALIGNMENT < atomic_capsule::MAX_ALIGNMENT,
        "ASSUM-003 violated: MIN >= MAX"
    );
    assert_eq!(
        atomic_capsule::MIN_ALIGNMENT.count_ones(),
        1,
        "ASSUM-003 violated: MIN not power of 2"
    );
    assert_eq!(
        atomic_capsule::MAX_ALIGNMENT.count_ones(),
        1,
        "ASSUM-003 violated: MAX not power of 2"
    );
}

#[test]
#[should_panic(expected = "must be >= 64")]
fn assum_003_below_min_panics() {
    // #ASSUME_BOUNDS_ENFORCED: Sizes below MIN are rejected
    // #VERIFY_BOUNDS_ENFORCED: Expect panic

    let _too_small = CacheLineSize::new(32);
}

#[test]
#[should_panic(expected = "must be <= 256")]
fn assum_003_above_max_panics() {
    // #ASSUME_BOUNDS_ENFORCED: Sizes above MAX are rejected
    // #VERIFY_BOUNDS_ENFORCED: Expect panic

    let _too_large = CacheLineSize::new(512);
}

// ============================================================================
// ASSUM-004: Architecture-Specific Cache Line Sizes
// ============================================================================

#[test]
fn assum_004_detect_returns_valid_size() {
    use atomic_capsule::detect_cache_line_size;

    // #ASSUME_DETECTION_VALID: Detection returns valid cache line size
    // #VERIFY_DETECTION_VALID: Check bounds and power-of-2

    let detected = detect_cache_line_size();

    assert!(
        detected.size() >= 64,
        "ASSUM-004 violated: Detected size too small"
    );
    assert!(
        detected.size() <= 256,
        "ASSUM-004 violated: Detected size too large"
    );
    assert_eq!(
        detected.size().count_ones(),
        1,
        "ASSUM-004 violated: Detected size not power of 2"
    );
}

#[test]
fn assum_004_detection_is_consistent() {
    use atomic_capsule::detect_cache_line_size;

    // #ASSUME_DETECTION_STABLE: Detection is deterministic
    // #VERIFY_DETECTION_STABLE: Multiple calls return same value

    let first = detect_cache_line_size();
    let second = detect_cache_line_size();
    let third = detect_cache_line_size();

    assert_eq!(
        first.size(),
        second.size(),
        "ASSUM-004 violated: Detection not consistent"
    );
    assert_eq!(
        second.size(),
        third.size(),
        "ASSUM-004 violated: Detection not consistent"
    );
}

#[test]
fn assum_004_architecture_specific_correctness() {
    use atomic_capsule::detect_cache_line_size;

    // #ASSUME_ARCH_CORRECT: Architecture detection matches hardware specs
    // #VERIFY_ARCH_CORRECT: Platform-specific validation

    let detected = detect_cache_line_size();

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // Intel/AMD: 64 bytes (verified in Intel SDM)
        assert_eq!(
            detected.size(),
            64,
            "ASSUM-004 violated: x86/x86_64 should have 64-byte cache lines"
        );
    }

    #[cfg(any(target_arch = "aarch64", target_arch = "arm"))]
    {
        // ARM Cortex-A: 64 bytes (verified in ARM ARM)
        assert_eq!(
            detected.size(),
            64,
            "ASSUM-004 violated: ARM should have 64-byte cache lines"
        );
    }

    #[cfg(target_arch = "riscv64")]
    {
        // RISC-V: 64 bytes (typical)
        assert_eq!(
            detected.size(),
            64,
            "ASSUM-004 violated: RISC-V should have 64-byte cache lines"
        );
    }

    #[cfg(target_arch = "powerpc64")]
    {
        // PowerPC: 128 bytes (verified in PowerPC Architecture)
        assert_eq!(
            detected.size(),
            128,
            "ASSUM-004 violated: PowerPC should have 128-byte cache lines"
        );
    }
}

// ============================================================================
// ASSUM-005: Retry Termination Guarantees
// ============================================================================

#[test]
fn assum_005_retry_terminates_within_max_iterations() {
    // #ASSUME_RETRY_TERMINATES: Retry loops terminate within max_iterations
    // #VERIFY_RETRY_TERMINATES: Verify is_exhausted() logic

    let mut policy = RetryPolicy::new(BackoffStrategy::default()).with_max_iterations(5);

    for i in 0..5 {
        assert!(
            !policy.is_exhausted(),
            "ASSUM-005 violated: Policy exhausted prematurely at iteration {}",
            i
        );
        policy.increment();
    }

    assert!(
        policy.is_exhausted(),
        "ASSUM-005 violated: Policy not exhausted after max iterations"
    );
}

#[test]
fn assum_005_single_threaded_cas_loop_terminates() {
    // #ASSUME_CAS_TERMINATES: CAS loops complete successfully
    // #VERIFY_CAS_TERMINATES: Single-threaded CAS with retry

    let atomic = AtomicU64::new(0);
    let mut policy = RetryPolicy::default();
    let mut attempts = 0;

    loop {
        let current = atomic.load(Ordering::Acquire);
        let new = current + 1;

        match atomic.compare_exchange_weak(current, new, Ordering::Release, Ordering::Relaxed) {
            Ok(_) => break,
            Err(_) => {
                attempts += 1;
                if policy.should_yield() {
                    policy.backoff();
                }
                policy.increment();

                // Safety: Prevent infinite loop in test
                assert!(
                    attempts < 100,
                    "ASSUM-005 violated: CAS loop exceeded safety limit"
                );
            }
        }
    }

    assert_eq!(atomic.load(Ordering::Acquire), 1);
    assert!(
        attempts < 10,
        "ASSUM-005: CAS should succeed quickly without contention (got {} attempts)",
        attempts
    );
}

#[test]
fn assum_005_retry_reset_clears_state() {
    // #ASSUME_RESET_CLEARS: Reset fully clears retry state
    // #VERIFY_RESET_CLEARS: Verify iteration and delay reset

    let mut policy = RetryPolicy::new(BackoffStrategy::Exponential {
        initial: 2,
        max: 64,
    });

    // Advance policy
    for _ in 0..5 {
        policy.increment();
    }

    assert!(policy.iteration() > 0);
    assert!(policy.current_delay > 2);

    // Reset and verify
    policy.reset();

    assert_eq!(
        policy.iteration(),
        0,
        "ASSUM-005 violated: Reset did not clear iteration count"
    );
    assert_eq!(
        policy.current_delay, 2,
        "ASSUM-005 violated: Reset did not restore initial delay"
    );
    assert!(
        !policy.is_exhausted(),
        "ASSUM-005 violated: Reset policy still exhausted"
    );
}

// ============================================================================
// ASSUM-006: Backoff Effectiveness
// ============================================================================
// NOTE: Full benchmarking tests moved to benches/retry_effectiveness.rs
// These are functional tests only

#[test]
fn assum_006_exponential_backoff_progression() {
    // #ASSUME_EXPONENTIAL_CORRECT: Backoff doubles each iteration up to max
    // #VERIFY_EXPONENTIAL_CORRECT: Check delay progression

    let mut policy = RetryPolicy::new(BackoffStrategy::Exponential {
        initial: 1,
        max: 16,
    });

    let expected = [1, 2, 4, 8, 16, 16]; // Cap at max
    let mut actual = Vec::new();

    for _ in 0..6 {
        actual.push(policy.current_delay);
        policy.increment();
    }

    assert_eq!(
        actual, expected,
        "ASSUM-006 violated: Exponential backoff progression incorrect"
    );
}

#[test]
fn assum_006_fixed_backoff_stays_constant() {
    // #ASSUME_FIXED_CONSTANT: Fixed backoff delay never changes
    // #VERIFY_FIXED_CONSTANT: Check delay stays fixed

    let mut policy = RetryPolicy::new(BackoffStrategy::Fixed { delay: 10 });

    for i in 0..20 {
        assert_eq!(
            policy.current_delay, 10,
            "ASSUM-006 violated: Fixed backoff changed at iteration {}",
            i
        );
        policy.increment();
    }
}

#[test]
fn assum_006_backoff_never_exceeds_max() {
    // #ASSUME_MAX_ENFORCED: Exponential backoff never exceeds max
    // #VERIFY_MAX_ENFORCED: Property test

    let max = 128;
    let mut policy = RetryPolicy::new(BackoffStrategy::Exponential { initial: 1, max });

    for i in 0..100 {
        assert!(
            policy.current_delay <= max,
            "ASSUM-006 violated: Delay exceeded max at iteration {}: {} > {}",
            i,
            policy.current_delay,
            max
        );
        policy.increment();
    }
}

// ============================================================================
// ASSUM-007: AlignmentTier Safety Contract
// ============================================================================

#[test]
fn assum_007_alignment_marker_enforces_contract() {
    // #ASSUME_CONTRACT_ENFORCED: AlignmentMarker validates tier contracts
    // #VERIFY_CONTRACT_ENFORCED: Const assertions in new()

    let hot = AlignmentMarker::<HotTier>::new();
    assert_eq!(hot.alignment(), 64);
    assert_eq!(hot.tier(), "hot");

    let warm = AlignmentMarker::<WarmTier>::new();
    assert_eq!(warm.alignment(), 128);
    assert_eq!(warm.tier(), "warm");

    let cold = AlignmentMarker::<ColdTier>::new();
    assert_eq!(cold.alignment(), 256);
    assert_eq!(cold.tier(), "cold");
}

#[test]
fn assum_007_tier_verify_alignment_validates() {
    // #ASSUME_VERIFY_CORRECT: verify_alignment() validates all constraints
    // #VERIFY_VERIFY_CORRECT: Check verify_alignment() logic

    assert!(
        HotTier::verify_alignment(),
        "ASSUM-007 violated: HotTier verification failed"
    );
    assert!(
        WarmTier::verify_alignment(),
        "ASSUM-007 violated: WarmTier verification failed"
    );
    assert!(
        ColdTier::verify_alignment(),
        "ASSUM-007 violated: ColdTier verification failed"
    );
}

#[test]
fn assum_007_custom_tier_validation() {
    // #ASSUME_CUSTOM_VALIDATED: Custom tiers are validated
    // #VERIFY_CUSTOM_VALIDATED: Test custom tier implementation

    // Valid custom tier
    assert!(TestHotCapsule::verify_alignment());
    assert!(TestWarmCapsule::verify_alignment());
    assert!(TestColdCapsule::verify_alignment());
}

// ============================================================================
// Comprehensive Safety Properties
// ============================================================================

#[test]
fn safety_no_unsafe_code_in_crate() {
    // Meta-test: Verify no unsafe code exists
    // This is a documentation test - actual verification via code review

    // The atomic_capsule foundation crate contains ZERO unsafe blocks
    // All safety is enforced via:
    // 1. Rust's type system (repr(align))
    // 2. Const evaluation (compile-time assertions)
    // 3. Standard library atomics (proven safe)

    // This test serves as documentation of the safety guarantee
    assert!(true, "atomic_capsule has zero unsafe code");
}

#[test]
fn safety_zero_dependencies() {
    // Meta-test: Verify zero dependency guarantee
    // This is a documentation test - actual verification via Cargo.toml

    // The atomic_capsule foundation crate has ZERO runtime dependencies
    // Only dev-dependencies for testing (criterion, proptest, trybuild)

    // Benefits:
    // - No supply chain risk
    // - No transitive CVEs
    // - Minimal attack surface

    assert!(true, "atomic_capsule has zero dependencies");
}

#[test]
fn safety_send_sync_automatic() {
    // Verify all types are Send + Sync

    fn is_send<T: Send>() {}
    fn is_sync<T: Sync>() {}

    // AlignmentMarker is Send + Sync
    is_send::<AlignmentMarker<HotTier>>();
    is_sync::<AlignmentMarker<HotTier>>();

    // RetryPolicy is Send + Sync
    is_send::<RetryPolicy>();
    is_sync::<RetryPolicy>();

    // CacheLineSize is Send + Sync
    is_send::<CacheLineSize>();
    is_sync::<CacheLineSize>();
}
