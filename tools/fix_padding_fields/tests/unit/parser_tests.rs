//! Unit tests for parser module (T28 Q1-Q7)
//!
//! These tests verify individual parser functions work correctly
//! in isolation with various inputs and edge cases.

use fix_padding_fields::parser::{extract_capsules, CapsuleInfo};

#[path = "../fixtures/mod.rs"]
mod fixtures;

// Q1: Test basic functionality with valid input
#[test]
fn test_extract_simple_capsule() {
    let capsules = extract_capsules(fixtures::SIMPLE_CAPSULE).expect("Should parse");

    assert_eq!(capsules.len(), 1);
    assert_eq!(capsules[0].name, "SimpleCapsule");
    assert_eq!(capsules[0].alignment, 64);
    assert_eq!(capsules[0].total_size, 64);
    assert_eq!(capsules[0].padding_size(), Some(56));
    assert_eq!(capsules[0].fields.len(), 1);
    assert_eq!(capsules[0].fields[0].name, "state");
}

// Q2: Test extraction with DualAtomicU64 (16 bytes)
#[test]
fn test_extract_dual_atomic_capsule() {
    let capsules = extract_capsules(fixtures::DUAL_ATOMIC_CAPSULE).expect("Should parse");

    assert_eq!(capsules.len(), 1);
    assert_eq!(capsules[0].name, "DualCapsule");
    assert_eq!(capsules[0].alignment, 128);
    assert_eq!(capsules[0].fields.len(), 1);
    assert_eq!(capsules[0].fields[0].size_bytes, 16); // DualAtomicU64
}

// Q3: Test extraction with multiple data fields
#[test]
fn test_extract_multi_field_capsule() {
    let capsules = extract_capsules(fixtures::MULTI_FIELD_CAPSULE).expect("Should parse");

    assert_eq!(capsules.len(), 1);
    assert_eq!(capsules[0].fields.len(), 3);

    let total_data: usize = capsules[0].fields.iter().map(|f| f.size_bytes).sum();
    assert_eq!(total_data, 20); // 8 + 4 + 8 = 20
}

// Q4: Test extraction with missing padding
#[test]
fn test_extract_missing_padding() {
    let capsules = extract_capsules(fixtures::MISSING_PADDING).expect("Should parse");

    assert_eq!(capsules.len(), 1);
    assert_eq!(capsules[0].padding_size(), None);
}

// Q5: Test extraction with multiple capsules in one file
#[test]
fn test_extract_multiple_capsules() {
    let capsules = extract_capsules(fixtures::MULTI_CAPSULE_FILE).expect("Should parse");

    assert_eq!(capsules.len(), 2);
    assert_eq!(capsules[0].name, "FirstCapsule");
    assert_eq!(capsules[1].name, "SecondCapsule");
    assert_eq!(capsules[0].alignment, 64);
    assert_eq!(capsules[1].alignment, 128);
}

// Q6: Test with non-capsule struct (should return empty)
#[test]
fn test_extract_non_capsule() {
    let capsules = extract_capsules(fixtures::NON_CAPSULE_STRUCT).expect("Should parse");

    assert_eq!(capsules.len(), 0);
}

// Q7: Test with invalid Rust syntax (error path)
#[test]
fn test_extract_invalid_syntax() {
    let invalid = "struct Invalid { this is not valid rust }";
    let result = extract_capsules(invalid);

    assert!(result.is_err());
}

// Q1: Test alignment attribute extraction (32/64/128/256)
#[test]
fn test_extract_alignment_values() {
    let test_cases = [
        (32, "alignment = 32"),
        (64, "alignment = 64"),
        (128, "alignment = 128"),
        (256, "alignment = 256"),
    ];

    for (expected, attr) in test_cases {
        let source = format!(r#"
            use atomic_capsule_derive::ComputationalCapsule;
            use core::sync::atomic::AtomicU64;

            #[derive(ComputationalCapsule)]
            #[capsule({})]
            #[repr(C, align({}))]
            struct TestCapsule {{
                state: AtomicU64,
            }}
        "#, attr, expected);

        let capsules = extract_capsules(&source).expect("Should parse");
        assert_eq!(capsules[0].alignment, expected);
    }
}

// Q2: Test size attribute extraction (when different from alignment)
#[test]
fn test_extract_size_attribute() {
    let source = r#"
        use atomic_capsule_derive::ComputationalCapsule;
        use core::sync::atomic::AtomicU64;

        #[derive(ComputationalCapsule)]
        #[capsule(alignment = 64, size = 128)]
        #[repr(C, align(64))]
        struct TestCapsule {
            state: AtomicU64,
        }
    "#;

    let capsules = extract_capsules(source).expect("Should parse");
    assert_eq!(capsules[0].alignment, 64);
    assert_eq!(capsules[0].total_size, 128);
}

// Q3: Test array field size estimation
#[test]
fn test_extract_array_field() {
    let capsules = extract_capsules(fixtures::ARRAY_FIELD_CAPSULE).expect("Should parse");

    let buffer_field = capsules[0].fields.iter()
        .find(|f| f.name == "buffer")
        .expect("Should have buffer field");

    assert_eq!(buffer_field.size_bytes, 32);
}

// Q4: Test generic capsule extraction
#[test]
fn test_extract_generic_capsule() {
    let capsules = extract_capsules(fixtures::GENERIC_CAPSULE).expect("Should parse");

    assert_eq!(capsules.len(), 1);
    assert_eq!(capsules[0].name, "GenericCapsule");
    // PhantomData is zero-sized, so should not contribute to data size
}

// Q5: Test multiple padding field detection
#[test]
fn test_extract_multiple_padding_fields() {
    let capsules = extract_capsules(fixtures::MULTI_PADDING_CAPSULE).expect("Should parse");

    assert_eq!(capsules.len(), 1);
    // Should detect last padding field
    assert!(capsules[0].padding_size().is_some());
}

// Q6: Test cold tier (256-byte) capsule
#[test]
fn test_extract_cold_tier_capsule() {
    let capsules = extract_capsules(fixtures::COLD_TIER_CAPSULE).expect("Should parse");

    assert_eq!(capsules[0].alignment, 256);
    assert_eq!(capsules[0].total_size, 256);
}

// Q7: Test real-world circuit breaker capsule
#[test]
fn test_extract_circuit_breaker() {
    let capsules = extract_capsules(fixtures::CIRCUIT_BREAKER_CAPSULE).expect("Should parse");

    assert_eq!(capsules[0].name, "CircuitBreakerCapsule");
    assert_eq!(capsules[0].alignment, 64);
    assert_eq!(capsules[0].padding_size(), Some(56));
}
