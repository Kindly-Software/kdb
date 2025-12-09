//! Unit tests for calculator module (T28 Q1-Q7)
//!
//! These tests verify padding calculation logic works correctly
//! for all alignment values and data sizes.

use fix_padding_fields::calculator::PaddingCalculator;
use fix_padding_fields::parser::{extract_capsules, CapsuleInfo, FieldInfo};

#[path = "../fixtures/mod.rs"]
mod fixtures;

// Q1: Test padding calculation for 64-byte alignment
#[test]
fn test_calculate_padding_64_byte() {
    let test_cases = [
        (0, 0),   // Empty struct
        (8, 56),  // One AtomicU64
        (16, 48), // Two AtomicU64
        (24, 40), // Three AtomicU64
        (32, 32), // Four AtomicU64
        (56, 8),  // Almost full
        (64, 0),  // Exact match
        (65, 63), // One byte over
    ];

    for (data_size, expected_padding) in test_cases {
        let capsule = CapsuleInfo {
            name: "TestCapsule".to_string(),
            alignment: 64,
            total_size: 64,
            padding_fields: vec![], total_padding_size: 0,
            fields: create_test_fields(data_size),
        };

        let calc = PaddingCalculator::new(&capsule).expect("Should create calculator");
        assert_eq!(
            calc.required_padding(),
            expected_padding,
            "Failed for data_size={}", data_size
        );
    }
}

// Q2: Test padding calculation for 128-byte alignment
#[test]
fn test_calculate_padding_128_byte() {
    let test_cases = [
        (0, 0),     // Empty
        (8, 120),   // One field
        (16, 112),  // DualAtomicU64
        (64, 64),   // Half
        (128, 0),   // Exact
        (129, 127), // Over by one
    ];

    for (data_size, expected_padding) in test_cases {
        let capsule = CapsuleInfo {
            name: "TestCapsule".to_string(),
            alignment: 128,
            total_size: 128,
            padding_fields: vec![], total_padding_size: 0,
            fields: create_test_fields(data_size),
        };

        let calc = PaddingCalculator::new(&capsule).expect("Should create calculator");
        assert_eq!(
            calc.required_padding(),
            expected_padding,
            "Failed for data_size={}", data_size
        );
    }
}

// Q3: Test padding calculation for 256-byte alignment (cold tier)
#[test]
fn test_calculate_padding_256_byte() {
    let test_cases = [
        (0, 0),
        (16, 240),
        (128, 128),
        (256, 0),
    ];

    for (data_size, expected_padding) in test_cases {
        let capsule = CapsuleInfo {
            name: "ColdTierCapsule".to_string(),
            alignment: 256,
            total_size: 256,
            padding_fields: vec![], total_padding_size: 0,
            fields: create_test_fields(data_size),
        };

        let calc = PaddingCalculator::new(&capsule).expect("Should create calculator");
        assert_eq!(calc.required_padding(), expected_padding);
    }
}

// Q4: Test needs_fixing detection (missing padding)
#[test]
fn test_needs_fixing_missing_padding() {
    let capsules = extract_capsules(fixtures::MISSING_PADDING).expect("Should parse");
    let calc = PaddingCalculator::new(&capsules[0]).expect("Should create calculator");

    assert!(calc.needs_fixing());
}

// Q5: Test needs_fixing detection (incorrect padding)
#[test]
fn test_needs_fixing_incorrect_padding() {
    let capsules = extract_capsules(fixtures::INCORRECT_PADDING).expect("Should parse");
    let calc = PaddingCalculator::new(&capsules[0]).expect("Should create calculator");

    assert!(calc.needs_fixing());
}

// Q6: Test needs_fixing detection (correct padding)
#[test]
fn test_needs_fixing_correct_padding() {
    let capsules = extract_capsules(fixtures::SIMPLE_CAPSULE).expect("Should parse");
    let calc = PaddingCalculator::new(&capsules[0]).expect("Should create calculator");

    assert!(!calc.needs_fixing());
}

// Q7: Test total_data_size calculation
#[test]
fn test_total_data_size() {
    let capsules = extract_capsules(fixtures::MULTI_FIELD_CAPSULE).expect("Should parse");
    let calc = PaddingCalculator::new(&capsules[0]).expect("Should create calculator");

    // counter (8) + flags (4) + timestamp (8) = 20
    assert_eq!(calc.total_data_size(), 20);
}

// Q1: Test padding minimality (padding < alignment)
#[test]
fn test_padding_minimality() {
    for alignment in [32, 64, 128, 256] {
        for data_size in 0..alignment * 2 {
            let capsule = CapsuleInfo {
                name: "TestCapsule".to_string(),
                alignment,
                total_size: alignment,
                padding_fields: vec![], total_padding_size: 0,
                fields: create_test_fields(data_size),
            };

            let calc = PaddingCalculator::new(&capsule).expect("Should create calculator");
            assert!(
                calc.required_padding() < alignment,
                "Padding {} >= alignment {} for data_size {}",
                calc.required_padding(),
                alignment,
                data_size
            );
        }
    }
}

// Q2: Test alignment invariant (data + padding) % alignment == 0
#[test]
fn test_alignment_invariant() {
    for alignment in [32, 64, 128, 256] {
        for data_size in 0..alignment * 2 {
            let capsule = CapsuleInfo {
                name: "TestCapsule".to_string(),
                alignment,
                total_size: alignment,
                padding_fields: vec![], total_padding_size: 0,
                fields: create_test_fields(data_size),
            };

            let calc = PaddingCalculator::new(&capsule).expect("Should create calculator");
            let total = calc.total_data_size() + calc.required_padding();

            assert_eq!(
                total % alignment,
                0,
                "Total {} not aligned to {} for data_size {}",
                total,
                alignment,
                data_size
            );
        }
    }
}

// Helper: Create test fields with specific total size
fn create_test_fields(total_size: usize) -> Vec<FieldInfo> {
    if total_size == 0 {
        return vec![];
    }

    let num_u64 = total_size / 8;
    let remainder = total_size % 8;

    let mut fields = vec![];

    for i in 0..num_u64 {
        fields.push(FieldInfo {
            name: format!("field{}", i),
            ty: "AtomicU64".to_string(),
            size_bytes: 8,
        });
    }

    if remainder > 0 {
        fields.push(FieldInfo {
            name: "remainder".to_string(),
            ty: format!("[u8; {}]", remainder),
            size_bytes: remainder,
        });
    }

    fields
}

// Q3: Test real-world capsule (circuit breaker)
#[test]
fn test_calculator_circuit_breaker() {
    let capsules = extract_capsules(fixtures::CIRCUIT_BREAKER_CAPSULE).expect("Should parse");
    let calc = PaddingCalculator::new(&capsules[0]).expect("Should create calculator");

    assert_eq!(calc.total_data_size(), 8); // One AtomicU64
    assert_eq!(calc.required_padding(), 56); // 64 - 8
    assert!(!calc.needs_fixing()); // Already correct
}

// Q4: Test real-world capsule (DualAtomic)
#[test]
fn test_calculator_dual_atomic() {
    let capsules = extract_capsules(fixtures::DUAL_ATOMIC_CAPSULE).expect("Should parse");
    let calc = PaddingCalculator::new(&capsules[0]).expect("Should create calculator");

    assert_eq!(calc.total_data_size(), 16); // DualAtomicU64
    assert_eq!(calc.required_padding(), 112); // 128 - 16
}

// Q5: Test cold tier capsule
#[test]
fn test_calculator_cold_tier() {
    let capsules = extract_capsules(fixtures::COLD_TIER_CAPSULE).expect("Should parse");
    let calc = PaddingCalculator::new(&capsules[0]).expect("Should create calculator");

    assert_eq!(calc.total_data_size(), 16); // Two AtomicU64
    assert_eq!(calc.required_padding(), 240); // 256 - 16
}
