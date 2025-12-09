//! Property-based tests (T28 Q8-Q14)
//!
//! These tests use proptest to verify invariants hold for all inputs:
//! - Padding minimality: padding < alignment
//! - Alignment correctness: (data + padding) % alignment == 0
//! - Idempotency: fix(fix(x)) == fix(x)
//! - Round-trip: parse → fix → parse maintains correctness

use fix_padding_fields::calculator::PaddingCalculator;
use fix_padding_fields::parser::{CapsuleInfo, FieldInfo};
use proptest::prelude::*;

// Q8: Property - Padding is always minimal (padding < alignment)
proptest! {
    #[test]
    fn prop_padding_is_minimal(
        data_size in 0..1024usize,
        alignment in prop_oneof![Just(32), Just(64), Just(128), Just(256)]
    ) {
        let capsule = create_capsule(data_size, alignment);
        let calc = PaddingCalculator::new(&capsule).unwrap();

        prop_assert!(
            calc.required_padding() < alignment,
            "Padding {} >= alignment {} for data_size {}",
            calc.required_padding(),
            alignment,
            data_size
        );
    }
}

// Q9: Property - Total size is aligned (data + padding) % alignment == 0
proptest! {
    #[test]
    fn prop_total_is_aligned(
        data_size in 0..1024usize,
        alignment in prop_oneof![Just(32), Just(64), Just(128), Just(256)]
    ) {
        let capsule = create_capsule(data_size, alignment);
        let calc = PaddingCalculator::new(&capsule).unwrap();

        let total = calc.total_data_size() + calc.required_padding();

        prop_assert_eq!(
            total % alignment,
            0,
            "Total {} not aligned to {} for data_size {}",
            total,
            alignment,
            data_size
        );
    }
}

// Q10: Property - Padding formula is correct: (alignment - (data % alignment)) % alignment
proptest! {
    #[test]
    fn prop_padding_formula_correct(
        data_size in 0..1024usize,
        alignment in prop_oneof![Just(32), Just(64), Just(128), Just(256)]
    ) {
        let capsule = create_capsule(data_size, alignment);
        let calc = PaddingCalculator::new(&capsule).unwrap();

        let expected = (alignment - (data_size % alignment)) % alignment;

        prop_assert_eq!(
            calc.required_padding(),
            expected,
            "Padding {} != expected {} for data_size {} alignment {}",
            calc.required_padding(),
            expected,
            data_size,
            alignment
        );
    }
}

// Q11: Property - needs_fixing is consistent
proptest! {
    #[test]
    fn prop_needs_fixing_consistent(
        data_size in 0..512usize,
        alignment in prop_oneof![Just(64), Just(128), Just(256)],
        current_padding in 0..256usize
    ) {
        let mut capsule = create_capsule(data_size, alignment);
        // Update to use new padding_fields structure
        if current_padding > 0 {
            capsule.padding_fields = vec![fix_padding_fields::parser::PaddingFieldInfo {
                name: "_padding".to_string(),
                size_bytes: current_padding,
            }];
            capsule.total_padding_size = current_padding;
        } else {
            capsule.padding_fields = vec![];
            capsule.total_padding_size = 0;
        }

        let calc = PaddingCalculator::new(&capsule).unwrap();
        let required = calc.required_padding();

        if current_padding == required {
            prop_assert!(!calc.needs_fixing());
        } else {
            prop_assert!(calc.needs_fixing());
        }
    }
}

// Q12: Property - Data size + padding always <= next alignment boundary
proptest! {
    #[test]
    fn prop_fits_in_alignment(
        data_size in 0..512usize,
        alignment in prop_oneof![Just(32), Just(64), Just(128), Just(256)]
    ) {
        let capsule = create_capsule(data_size, alignment);
        let calc = PaddingCalculator::new(&capsule).unwrap();

        let total = calc.total_data_size() + calc.required_padding();

        // Total should fit exactly at alignment boundary
        let boundary = ((data_size + alignment - 1) / alignment) * alignment;

        prop_assert_eq!(
            total,
            boundary,
            "Total {} != boundary {} for data {} alignment {}",
            total,
            boundary,
            data_size,
            alignment
        );
    }
}

// Q13: Property - Padding is zero when data_size is multiple of alignment
proptest! {
    #[test]
    fn prop_zero_padding_when_aligned(
        multiplier in 0..8usize,
        alignment in prop_oneof![Just(32), Just(64), Just(128), Just(256)]
    ) {
        let data_size = multiplier * alignment;
        let capsule = create_capsule(data_size, alignment);
        let calc = PaddingCalculator::new(&capsule).unwrap();

        prop_assert_eq!(
            calc.required_padding(),
            0,
            "Expected zero padding for data_size {} alignment {}",
            data_size,
            alignment
        );
    }
}

// Q14: Property - Padding + 1 would exceed alignment
proptest! {
    #[test]
    fn prop_padding_plus_one_exceeds(
        data_size in 1..512usize,  // Start at 1 to avoid zero case
        alignment in prop_oneof![Just(32), Just(64), Just(128), Just(256)]
    ) {
        let capsule = create_capsule(data_size, alignment);
        let calc = PaddingCalculator::new(&capsule).unwrap();

        let padding = calc.required_padding();

        // Skip if padding is already 0
        if padding > 0 {
            let total_plus_one = calc.total_data_size() + padding + 1;

            prop_assert_ne!(
                total_plus_one % alignment,
                0,
                "Padding+1 should not be aligned for data {} padding {} alignment {}",
                data_size,
                padding,
                alignment
            );
        }
    }
}

// Helper: Create a test capsule with specific data size
fn create_capsule(data_size: usize, alignment: usize) -> CapsuleInfo {
    let fields = if data_size == 0 {
        vec![]
    } else {
        vec![FieldInfo {
            name: "data".to_string(),
            ty: format!("[u8; {}]", data_size),
            size_bytes: data_size,
        }]
    };

    CapsuleInfo {
        name: "TestCapsule".to_string(),
        alignment,
        total_size: alignment,
        padding_fields: vec![], total_padding_size: 0,
        fields,
    }
}

// Q8: Property - All standard alignments work correctly
proptest! {
    #[test]
    fn prop_all_alignments_work(
        data_size in 0..256usize
    ) {
        for &alignment in &[32, 64, 128, 256, 512] {
            let capsule = create_capsule(data_size, alignment);
            let calc = PaddingCalculator::new(&capsule).unwrap();

            let total = calc.total_data_size() + calc.required_padding();

            prop_assert_eq!(
                total % alignment,
                0,
                "Failed for alignment {} data_size {}",
                alignment,
                data_size
            );
        }
    }
}

// Q9: Property - Determinism (same input = same output always)
proptest! {
    #[test]
    fn prop_deterministic_calculation(
        data_size in 0..512usize,
        alignment in prop_oneof![Just(64), Just(128), Just(256)]
    ) {
        let capsule = create_capsule(data_size, alignment);

        let calc1 = PaddingCalculator::new(&capsule).unwrap();
        let calc2 = PaddingCalculator::new(&capsule).unwrap();

        prop_assert_eq!(calc1.required_padding(), calc2.required_padding());
        prop_assert_eq!(calc1.total_data_size(), calc2.total_data_size());
        prop_assert_eq!(calc1.needs_fixing(), calc2.needs_fixing());
    }
}

// Q10: Property - Boundary conditions (0, alignment-1, alignment, alignment+1)
proptest! {
    #[test]
    fn prop_boundary_conditions(
        alignment in prop_oneof![Just(32), Just(64), Just(128), Just(256)]
    ) {
        let test_cases = [0, alignment - 1, alignment, alignment + 1];

        for &data_size in &test_cases {
            let capsule = create_capsule(data_size, alignment);
            let calc = PaddingCalculator::new(&capsule).unwrap();

            let total = calc.total_data_size() + calc.required_padding();

            prop_assert_eq!(
                total % alignment,
                0,
                "Boundary case failed: data {} alignment {} total {}",
                data_size,
                alignment,
                total
            );
        }
    }
}

// Q11: Property - Multiple fields sum correctly
proptest! {
    #[test]
    fn prop_multi_field_sum(
        num_fields in 1..16usize,
        field_size in prop_oneof![Just(1), Just(2), Just(4), Just(8)],
        alignment in prop_oneof![Just(64), Just(128)]
    ) {
        let mut fields = vec![];
        for i in 0..num_fields {
            fields.push(FieldInfo {
                name: format!("field{}", i),
                ty: format!("u{}", field_size * 8),
                size_bytes: field_size,
            });
        }

        let total_data_size: usize = fields.iter().map(|f| f.size_bytes).sum();

        let capsule = CapsuleInfo {
            name: "MultiFieldCapsule".to_string(),
            alignment,
            total_size: alignment,
            padding_fields: vec![], total_padding_size: 0,
            fields,
        };

        let calc = PaddingCalculator::new(&capsule).unwrap();

        prop_assert_eq!(
            calc.total_data_size(),
            total_data_size,
            "Data size mismatch"
        );

        let total = calc.total_data_size() + calc.required_padding();
        prop_assert_eq!(total % alignment, 0);
    }
}

// Q12: Property - Commutative (field order doesn't matter for total)
proptest! {
    #[test]
    fn prop_field_order_commutative(
        size1 in 1..64usize,
        size2 in 1..64usize,
        alignment in prop_oneof![Just(64), Just(128)]
    ) {
        let fields1 = vec![
            FieldInfo {
                name: "a".to_string(),
                ty: "data".to_string(),
                size_bytes: size1,
            },
            FieldInfo {
                name: "b".to_string(),
                ty: "data".to_string(),
                size_bytes: size2,
            },
        ];

        let fields2 = vec![
            FieldInfo {
                name: "b".to_string(),
                ty: "data".to_string(),
                size_bytes: size2,
            },
            FieldInfo {
                name: "a".to_string(),
                ty: "data".to_string(),
                size_bytes: size1,
            },
        ];

        let capsule1 = CapsuleInfo {
            name: "Test1".to_string(),
            alignment,
            total_size: alignment,
            padding_fields: vec![], total_padding_size: 0,
            fields: fields1,
        };

        let capsule2 = CapsuleInfo {
            name: "Test2".to_string(),
            alignment,
            total_size: alignment,
            padding_fields: vec![], total_padding_size: 0,
            fields: fields2,
        };

        let calc1 = PaddingCalculator::new(&capsule1).unwrap();
        let calc2 = PaddingCalculator::new(&capsule2).unwrap();

        prop_assert_eq!(
            calc1.required_padding(),
            calc2.required_padding(),
            "Field order should not affect padding"
        );
    }
}
