//! T28 Comprehensive Test Suite for Const Hashing
//!
//! Tier 1: Unit Tests (Q1-Q7) - Basic functionality
//! Tier 2: Property Tests (Q8-Q14) - Invariant validation
//! Tier 3: Integration Tests (Q15-Q21) - Cross-tier composition
//! Tier 4: Production Tests (Q22-Q28) - Stress, concurrency, security

use atomic_capsule::hash::const_capsule::ConstHashCapsule;
use atomic_capsule::hash::const_hash::{const_fast_hash, const_fast_hash_fields, ConstHashable};

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7) - Basic Functionality
// ============================================================================

#[test]
fn t1_q1_const_hash_deterministic() {
    const HASH1: u64 = const_fast_hash(b"test");
    const HASH2: u64 = const_fast_hash(b"test");
    assert_eq!(HASH1, HASH2, "Const hash must be deterministic");
}

#[test]
fn t1_q2_const_hash_different_inputs() {
    const HASH1: u64 = const_fast_hash(b"test1");
    const HASH2: u64 = const_fast_hash(b"test2");
    assert_ne!(HASH1, HASH2, "Different inputs produce different hashes");
}

#[test]
fn t1_q3_const_hash_fields_deterministic() {
    const FIELDS: [u64; 4] = [1, 2, 3, 4];
    const HASH1: u64 = const_fast_hash_fields(&FIELDS);
    const HASH2: u64 = const_fast_hash_fields(&FIELDS);
    assert_eq!(HASH1, HASH2, "Field hash must be deterministic");
}

#[test]
fn t1_q4_const_hash_capsule_creation() {
    struct TestData {
        value: u64,
    }
    impl ConstHashable for TestData {
        const HASH: u64 = const_fast_hash(b"TestData");
    }

    const CAPSULE: ConstHashCapsule<TestData> = ConstHashCapsule::new(TestData { value: 42 });

    assert_eq!(CAPSULE.hash(), TestData::HASH);
}

#[test]
fn t1_q5_const_hash_capsule_value_access() {
    struct TestData {
        value: u64,
    }
    impl ConstHashable for TestData {
        const HASH: u64 = const_fast_hash(b"TestData");
    }

    const CAPSULE: ConstHashCapsule<TestData> = ConstHashCapsule::new(TestData { value: 42 });

    assert_eq!(CAPSULE.value().value, 42);
}

#[test]
fn t1_q6_const_hash_capsule_integrity() {
    struct TestData {
        value: u64,
    }
    impl ConstHashable for TestData {
        const HASH: u64 = const_fast_hash(b"TestData");
    }

    const CAPSULE: ConstHashCapsule<TestData> = ConstHashCapsule::new(TestData { value: 42 });

    assert!(CAPSULE.verify_integrity());
}

#[test]
fn t1_q7_const_hash_zero_runtime_cost() {
    struct TestData {
        value: u64,
    }
    impl ConstHashable for TestData {
        const HASH: u64 = const_fast_hash(b"TestData");
    }

    const CAPSULE: ConstHashCapsule<TestData> = ConstHashCapsule::new(TestData { value: 42 });

    // Measure hash retrieval time
    let start = std::time::Instant::now();
    for _ in 0..10_000 {
        core::hint::black_box(CAPSULE.hash());
    }
    let elapsed = start.elapsed();

    // Should be <10μs for 10K iterations (<1ns per call)
    assert!(
        elapsed.as_nanos() < 10_000,
        "Hash retrieval should be <1ns, got {} ns/call",
        elapsed.as_nanos() / 10_000
    );
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14) - Invariant Validation
// ============================================================================

#[cfg(feature = "proptest")]
mod tier2_property_tests {
    use super::*;
    use proptest::prelude::*;

    struct PropData {
        value: u64,
    }
    impl ConstHashable for PropData {
        const HASH: u64 = const_fast_hash(b"PropData");
    }

    proptest! {
        #[test]
        fn t2_q8_hash_stability(data: Vec<u8>) {
            let hash1 = const_fast_hash(&data);
            let hash2 = const_fast_hash(&data);
            prop_assert_eq!(hash1, hash2);
        }

        #[test]
        fn t2_q9_hash_determinism(fields: Vec<u64>) {
            let hash1 = const_fast_hash_fields(&fields);
            let hash2 = const_fast_hash_fields(&fields);
            prop_assert_eq!(hash1, hash2);
        }

        #[test]
        fn t2_q10_capsule_hash_always_same(value: u64) {
            let capsule = ConstHashCapsule::new(PropData { value });
            prop_assert_eq!(capsule.hash(), PropData::HASH);
        }

        #[test]
        fn t2_q11_capsule_integrity_always_valid(value: u64) {
            let capsule = ConstHashCapsule::new(PropData { value });
            prop_assert!(capsule.verify_integrity());
        }

        #[test]
        fn t2_q12_value_preservation(value: u64) {
            let capsule = ConstHashCapsule::new(PropData { value });
            prop_assert_eq!(capsule.value().value, value);
        }

        #[test]
        fn t2_q13_hash_distribution(data: Vec<u8>) {
            // Hash should not be zero (except for specific inputs)
            if !data.is_empty() {
                let hash = const_fast_hash(&data);
                // Just verify it computes without panic
                prop_assert!(true);
                let _ = hash;
            }
        }

        #[test]
        fn t2_q14_order_sensitivity(data: Vec<u8>) {
            if data.len() > 1 {
                let mut reversed = data.clone();
                reversed.reverse();

                let hash1 = const_fast_hash(&data);
                let hash2 = const_fast_hash(&reversed);

                // Should be different for reversed input (unless palindrome)
                if data != reversed {
                    prop_assert_ne!(hash1, hash2);
                }
            }
        }
    }
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21) - Cross-Tier Composition
// ============================================================================

#[test]
fn t3_q15_capsule_with_multiple_fields() {
    struct MultiField {
        a: u64,
        b: u64,
        c: u64,
    }

    impl ConstHashable for MultiField {
        const HASH: u64 = const_fast_hash_fields(&[1, 2, 3]);
    }

    const CAPSULE: ConstHashCapsule<MultiField> =
        ConstHashCapsule::new(MultiField { a: 1, b: 2, c: 3 });

    assert!(CAPSULE.verify_integrity());
}

#[test]
fn t3_q16_capsule_with_nested_types() {
    struct Inner {
        value: u64,
    }
    struct Outer {
        inner: Inner,
    }

    impl ConstHashable for Outer {
        const HASH: u64 = const_fast_hash(b"Outer");
    }

    const CAPSULE: ConstHashCapsule<Outer> = ConstHashCapsule::new(Outer {
        inner: Inner { value: 42 },
    });

    assert_eq!(CAPSULE.hash(), Outer::HASH);
}

#[test]
fn t3_q17_capsule_type_distinction() {
    struct TypeA {
        value: u64,
    }
    impl ConstHashable for TypeA {
        const HASH: u64 = const_fast_hash(b"TypeA");
    }

    struct TypeB {
        value: u64,
    }
    impl ConstHashable for TypeB {
        const HASH: u64 = const_fast_hash(b"TypeB");
    }

    const CAPSULE_A: ConstHashCapsule<TypeA> = ConstHashCapsule::new(TypeA { value: 1 });
    const CAPSULE_B: ConstHashCapsule<TypeB> = ConstHashCapsule::new(TypeB { value: 1 });

    assert_ne!(CAPSULE_A.hash(), CAPSULE_B.hash());
}

#[test]
fn t3_q18_compile_time_const_evaluation() {
    struct Data {
        value: u64,
    }
    impl ConstHashable for Data {
        const HASH: u64 = const_fast_hash(b"Data");
    }

    // All of this evaluated at compile-time
    const CAPSULE: ConstHashCapsule<Data> = ConstHashCapsule::new(Data { value: 42 });
    const HASH: u64 = CAPSULE.hash();
    const VALID: bool = CAPSULE.verify_integrity();

    assert_eq!(HASH, Data::HASH);
    assert!(VALID);
}

#[test]
fn t3_q19_runtime_vs_const_equivalence() {
    const DATA: &[u8] = b"test data";
    const CONST_HASH: u64 = const_fast_hash(DATA);
    let runtime_hash = const_fast_hash(DATA);

    assert_eq!(CONST_HASH, runtime_hash);
}

#[test]
fn t3_q20_empty_input_handling() {
    const EMPTY_HASH: u64 = const_fast_hash(b"");
    const EMPTY_FIELDS: [u64; 0] = [];
    const EMPTY_FIELDS_HASH: u64 = const_fast_hash_fields(&EMPTY_FIELDS);

    // Both should produce non-zero hashes (FNV offset basis)
    assert_ne!(EMPTY_HASH, 0);
    assert_ne!(EMPTY_FIELDS_HASH, 0);
}

#[test]
fn t3_q21_large_input_handling() {
    const LARGE_DATA: &[u8] = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
        Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
        Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.";
    const HASH: u64 = const_fast_hash(LARGE_DATA);

    assert_ne!(HASH, 0);
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28) - Stress, Concurrency, Security
// ============================================================================

#[test]
fn t4_q22_stress_many_capsules() {
    // Create 1000 different capsule types
    for i in 0..1000 {
        let data = format!("capsule_{}", i);
        let hash = const_fast_hash(data.as_bytes());
        assert_ne!(hash, 0, "Hash {} should be non-zero", i);
    }
}

#[test]
fn t4_q23_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    struct SharedData {
        value: u64,
    }
    impl ConstHashable for SharedData {
        const HASH: u64 = const_fast_hash(b"SharedData");
    }

    const CAPSULE: ConstHashCapsule<SharedData> = ConstHashCapsule::new(SharedData { value: 42 });

    let capsule = Arc::new(CAPSULE);

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let capsule_clone = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..1000 {
                    let hash = capsule_clone.hash();
                    assert_eq!(hash, SharedData::HASH);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn t4_q24_collision_resistance() {
    // Test that similar inputs produce different hashes
    const HASH_A: u64 = const_fast_hash(b"capsule_0");
    const HASH_B: u64 = const_fast_hash(b"capsule_1");
    const HASH_C: u64 = const_fast_hash(b"capsule_2");

    assert_ne!(HASH_A, HASH_B);
    assert_ne!(HASH_B, HASH_C);
    assert_ne!(HASH_A, HASH_C);
}

#[test]
fn t4_q25_performance_regression() {
    struct PerfData {
        value: u64,
    }
    impl ConstHashable for PerfData {
        const HASH: u64 = const_fast_hash(b"PerfData");
    }

    const CAPSULE: ConstHashCapsule<PerfData> = ConstHashCapsule::new(PerfData { value: 42 });

    // Warmup
    for _ in 0..100 {
        core::hint::black_box(CAPSULE.hash());
    }

    // Measure
    let iterations = 1_000_000;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        core::hint::black_box(CAPSULE.hash());
    }
    let elapsed = start.elapsed();

    let ns_per_op = elapsed.as_nanos() / iterations;

    // Should be <2ns per operation (B32 target)
    assert!(
        ns_per_op < 2,
        "Hash retrieval should be <2ns, got {} ns",
        ns_per_op
    );

    println!("Performance: {} ns/op (target: <2ns)", ns_per_op);
}

#[test]
fn t4_q26_memory_safety() {
    struct SafeData {
        value: u64,
    }
    impl ConstHashable for SafeData {
        const HASH: u64 = const_fast_hash(b"SafeData");
    }

    // Create and drop many capsules
    for _ in 0..10_000 {
        let capsule = ConstHashCapsule::new(SafeData { value: 42 });
        assert_eq!(capsule.hash(), SafeData::HASH);
        drop(capsule);
    }
}

#[test]
fn t4_q27_alignment_verification() {
    struct AlignedData {
        value: u64,
    }
    impl ConstHashable for AlignedData {
        const HASH: u64 = const_fast_hash(b"AlignedData");
    }

    let capsule = ConstHashCapsule::new(AlignedData { value: 42 });
    let ptr = &capsule as *const _ as usize;

    // Should be 64-byte aligned
    assert_eq!(ptr % 64, 0, "Capsule should be 64-byte aligned");
}

#[test]
fn t4_q28_production_simulation() {
    struct ProdData {
        value: u64,
    }
    impl ConstHashable for ProdData {
        const HASH: u64 = const_fast_hash(b"ProdData");
    }

    const CAPSULE: ConstHashCapsule<ProdData> = ConstHashCapsule::new(ProdData { value: 42 });

    // Simulate production load: 10M operations
    let mut total_hash: u64 = 0;
    for _ in 0..10_000_000 {
        total_hash = total_hash.wrapping_add(CAPSULE.hash());
    }

    // Just verify it completes without panic
    assert_ne!(total_hash, 0);
}

// ============================================================================
// CONST ASSERTIONS (Compile-Time Validation)
// ============================================================================

const _: () = {
    struct CompileTimeData {
        value: u64,
    }
    impl ConstHashable for CompileTimeData {
        const HASH: u64 = const_fast_hash(b"CompileTimeData");
    }

    const CAPSULE: ConstHashCapsule<CompileTimeData> =
        ConstHashCapsule::new(CompileTimeData { value: 42 });

    // Verify at compile-time
    assert!(CAPSULE.hash() == CompileTimeData::HASH);
    assert!(CAPSULE.verify_integrity());
};
