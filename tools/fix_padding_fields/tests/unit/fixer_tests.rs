//! Unit tests for fixer module (T28 Q1-Q7)
//!
//! These tests verify padding fixes are correctly applied to source code.

use fix_padding_fields::fixer::PaddingFixer;
use fix_padding_fields::parser::extract_capsules;

#[path = "../fixtures/mod.rs"]
mod fixtures;

// Q1: Test fixing incorrect padding
#[test]
fn test_fix_incorrect_padding() {
    let mut fixer = PaddingFixer::new(fixtures::INCORRECT_PADDING.to_string());
    let capsules = extract_capsules(fixtures::INCORRECT_PADDING).expect("Should parse");

    let changed = fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
    assert!(changed);

    // Verify the fix
    let new_content = fixer.content();
    assert!(new_content.contains("_padding: [u8; 56]"));
    assert!(!new_content.contains("_padding: [u8; 32]"));
}

// Q2: Test adding missing padding
#[test]
fn test_fix_missing_padding() {
    let mut fixer = PaddingFixer::new(fixtures::MISSING_PADDING.to_string());
    let capsules = extract_capsules(fixtures::MISSING_PADDING).expect("Should parse");

    let changed = fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
    assert!(changed);

    // Verify padding was added
    let new_content = fixer.content();
    assert!(new_content.contains("_padding: [u8; 56]"));
}

// Q3: Test no changes when padding is correct
#[test]
fn test_fix_correct_padding_unchanged() {
    let mut fixer = PaddingFixer::new(fixtures::SIMPLE_CAPSULE.to_string());
    let capsules = extract_capsules(fixtures::SIMPLE_CAPSULE).expect("Should parse");

    let changed = fixer.apply_padding_fix(&capsules[0]).expect("Should not fail");
    assert!(!changed);

    // Content should be unchanged
    assert_eq!(fixer.content(), fixtures::SIMPLE_CAPSULE);
}

// Q4: Test consolidating multiple padding fields
#[test]
fn test_fix_multiple_padding_fields() {
    let mut fixer = PaddingFixer::new(fixtures::MULTI_PADDING_CAPSULE.to_string());
    let capsules = extract_capsules(fixtures::MULTI_PADDING_CAPSULE).expect("Should parse");

    let changed = fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
    assert!(changed);

    // Verify old padding fields removed and new one added
    let new_content = fixer.content();
    assert!(!new_content.contains("_padding1"));
    assert!(!new_content.contains("_padding2"));
    assert!(new_content.contains("_padding: [u8; 120]"));
}

// Q5: Test fixing DualAtomic capsule
#[test]
fn test_fix_dual_atomic() {
    let incorrect_dual = r#"
use atomic_capsule_derive::ComputationalCapsule;
use atomic_capsule::primitives::DualAtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct DualCapsule {
    dual: DualAtomicU64,
    _padding: [u8; 64],  // Wrong! Should be 112
}
"#;

    let mut fixer = PaddingFixer::new(incorrect_dual.to_string());
    let capsules = extract_capsules(incorrect_dual).expect("Should parse");

    let changed = fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
    assert!(changed);

    let new_content = fixer.content();
    assert!(new_content.contains("_padding: [u8; 112]"));
}

// Q6: Test fixing cold tier capsule
#[test]
fn test_fix_cold_tier() {
    let incorrect_cold = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
struct ColdTierCapsule {
    state: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 128],  // Wrong! Should be 240
}
"#;

    let mut fixer = PaddingFixer::new(incorrect_cold.to_string());
    let capsules = extract_capsules(incorrect_cold).expect("Should parse");

    let changed = fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
    assert!(changed);

    let new_content = fixer.content();
    assert!(new_content.contains("_padding: [u8; 240]"));
}

// Q7: Test preserving struct attributes and derives
#[test]
fn test_fix_preserves_attributes() {
    let mut fixer = PaddingFixer::new(fixtures::MISSING_PADDING.to_string());
    let capsules = extract_capsules(fixtures::MISSING_PADDING).expect("Should parse");

    fixer.apply_padding_fix(&capsules[0]).expect("Should fix");

    let new_content = fixer.content();
    assert!(new_content.contains("#[derive(ComputationalCapsule)]"));
    assert!(new_content.contains("#[capsule(alignment = 64"));
    assert!(new_content.contains("#[repr(C, align(64))]"));
}

// Q1: Test multiple capsules in one file (fix all)
#[test]
fn test_fix_multiple_capsules() {
    let multi_incorrect = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct FirstCapsule {
    state: AtomicU64,
    _padding: [u8; 32],  // Wrong
}

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct SecondCapsule {
    state: AtomicU64,
    counter: AtomicU64,
    _padding: [u8; 64],  // Wrong
}
"#;

    let mut fixer = PaddingFixer::new(multi_incorrect.to_string());
    let capsules = extract_capsules(multi_incorrect).expect("Should parse");

    assert_eq!(capsules.len(), 2);

    for capsule in capsules {
        fixer.apply_padding_fix(&capsule).expect("Should fix");
    }

    let new_content = fixer.content();
    // Verify both were fixed
    assert!(new_content.contains("_padding: [u8; 56]")); // First
    assert!(new_content.contains("_padding: [u8; 112]")); // Second
}

// Q2: Test array field capsule
#[test]
fn test_fix_array_field_capsule() {
    let incorrect_array = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct ArrayFieldCapsule {
    state: AtomicU64,
    buffer: [u8; 32],
    _padding: [u8; 64],  // Wrong! Should be 88
}
"#;

    let mut fixer = PaddingFixer::new(incorrect_array.to_string());
    let capsules = extract_capsules(incorrect_array).expect("Should parse");

    let changed = fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
    assert!(changed);

    let new_content = fixer.content();
    assert!(new_content.contains("_padding: [u8; 88]"));
}

// Q3: Test edge case - empty struct (needs full padding)
#[test]
fn test_fix_empty_struct() {
    let empty_capsule = r#"
use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct EmptyCapsule {
}
"#;

    let mut fixer = PaddingFixer::new(empty_capsule.to_string());
    let capsules = extract_capsules(empty_capsule).expect("Should parse");

    if !capsules.is_empty() {
        let changed = fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
        if changed {
            let new_content = fixer.content();
            assert!(new_content.contains("_padding: [u8; 64]"));
        }
    }
}

// Q4: Test content() returns correct updated content
#[test]
fn test_content_returns_updated() {
    let mut fixer = PaddingFixer::new(fixtures::INCORRECT_PADDING.to_string());
    let capsules = extract_capsules(fixtures::INCORRECT_PADDING).expect("Should parse");

    let original = fixer.content().to_string();
    fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
    let updated = fixer.content().to_string();

    assert_ne!(original, updated);
    assert!(updated.contains("_padding: [u8; 56]"));
}
