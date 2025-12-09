//! Integration tests for fixed-point type detection
//!
//! These tests demonstrate real-world usage in derive macro contexts.

use atomic_capsule::serialize::fixed_point_type_detection::{
    check_precision_loss, check_type_conflict, detect_fixed_point_type, DetectionStrategy,
    FixedPointType, PrecisionLoss,
};

// ============================================================================
// Basic Detection Tests
// ============================================================================

#[test]
fn test_detect_from_full_path() {
    let info =
        detect_fixed_point_type("atomic_capsule::serialize::fixed_point_impls::Q16_16").unwrap();
    assert_eq!(info.fp_type, FixedPointType::Q16_16);
    assert_eq!(info.strategy, DetectionStrategy::Path);
    assert_eq!(info.container_depth, 0);
    assert!(!info.is_wrapped());
}

#[test]
fn test_detect_from_short_path() {
    let info = detect_fixed_point_type("fixed_point_impls::Q8_8").unwrap();
    assert_eq!(info.fp_type, FixedPointType::Q8_8);
    assert_eq!(info.strategy, DetectionStrategy::Path);
}

#[test]
fn test_detect_from_type_name() {
    let info = detect_fixed_point_type("Q32_32").unwrap();
    assert_eq!(info.fp_type, FixedPointType::Q32_32);
    assert_eq!(info.strategy, DetectionStrategy::TypeName);
}

#[test]
fn test_detect_newtype_wrapper() {
    // Custom NewType wrapper: pub struct MyPrice(Q16_16)
    let info = detect_fixed_point_type("PriceQ16_16").unwrap();
    assert_eq!(info.fp_type, FixedPointType::Q16_16);
    assert_eq!(info.strategy, DetectionStrategy::TypeName);
}

// ============================================================================
// Container Detection Tests
// ============================================================================

#[test]
fn test_detect_option_wrapper() {
    let info = detect_fixed_point_type("Option<Q16_16>").unwrap();
    assert_eq!(info.fp_type, FixedPointType::Q16_16);
    assert_eq!(info.strategy, DetectionStrategy::Container);
    assert_eq!(info.container_depth, 1);
    assert!(info.is_wrapped());
}

#[test]
fn test_detect_vec_wrapper() {
    let info = detect_fixed_point_type("Vec<Q8_8>").unwrap();
    assert_eq!(info.fp_type, FixedPointType::Q8_8);
    assert_eq!(info.strategy, DetectionStrategy::Container);
    assert_eq!(info.container_depth, 1);
}

#[test]
fn test_detect_box_wrapper() {
    let info = detect_fixed_point_type("Box<Q32_32>").unwrap();
    assert_eq!(info.fp_type, FixedPointType::Q32_32);
    assert_eq!(info.strategy, DetectionStrategy::Container);
}

#[test]
fn test_detect_arc_wrapper() {
    let info = detect_fixed_point_type("Arc<Q16_16>").unwrap();
    assert_eq!(info.fp_type, FixedPointType::Q16_16);
    assert_eq!(info.strategy, DetectionStrategy::Container);
}

#[test]
fn test_detect_nested_containers() {
    let info = detect_fixed_point_type("Option<Vec<Q16_16>>").unwrap();
    assert_eq!(info.fp_type, FixedPointType::Q16_16);
    assert_eq!(info.strategy, DetectionStrategy::Container);
    assert_eq!(info.container_depth, 2);
}

#[test]
fn test_detect_deeply_nested_containers() {
    let info = detect_fixed_point_type("Option<Vec<Box<Q32_32>>>").unwrap();
    assert_eq!(info.fp_type, FixedPointType::Q32_32);
    assert_eq!(info.strategy, DetectionStrategy::Container);
    assert_eq!(info.container_depth, 3);
}

// ============================================================================
// Error Detection Tests
// ============================================================================

#[test]
fn test_unknown_type_error() {
    let result = detect_fixed_point_type("CompletelyUnknownType");
    assert!(result.is_err());

    let err = result.unwrap_err();
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Unknown fixed-point type"));
    assert!(error_msg.contains("Q8_8"));
    assert!(error_msg.contains("Q16_16"));
    assert!(error_msg.contains("Q32_32"));
}

#[test]
fn test_fuzzy_matching_close_typo() {
    let result = detect_fixed_point_type("Q16_15"); // Off by one
    assert!(result.is_err());

    let err = result.unwrap_err();
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Q16_16")); // Should suggest correct type
}

#[test]
fn test_fuzzy_matching_missing_underscore() {
    let result = detect_fixed_point_type("Q1616"); // Missing underscore
    assert!(result.is_err());

    let err = result.unwrap_err();
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("Q16_16")); // Should suggest correct type
}

// ============================================================================
// Type Conflict Tests
// ============================================================================

#[test]
fn test_type_conflict_detection_error() {
    let result = check_type_conflict(FixedPointType::Q8_8, FixedPointType::Q16_16, "amount");
    assert!(result.is_err());

    let err = result.unwrap_err();
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("type conflict"));
    assert!(error_msg.contains("amount"));
    assert!(error_msg.contains("Q8_8"));
    assert!(error_msg.contains("Q16_16"));
}

#[test]
fn test_type_conflict_same_type_ok() {
    let result = check_type_conflict(FixedPointType::Q16_16, FixedPointType::Q16_16, "amount");
    assert!(result.is_ok());
}

// ============================================================================
// Precision Loss Tests
// ============================================================================

#[test]
fn test_precision_loss_upcast_safe() {
    // Q8_8 → Q16_16 (safe upcast)
    let result = check_precision_loss(FixedPointType::Q8_8, FixedPointType::Q16_16, "upcast");
    assert!(result.is_ok());
}

#[test]
fn test_precision_loss_downcast_unsafe() {
    // Q16_16 → Q8_8 (unsafe downcast)
    let result = check_precision_loss(FixedPointType::Q16_16, FixedPointType::Q8_8, "downcast");
    assert!(result.is_err());

    let err = result.unwrap_err();
    let error_msg = format!("{}", err);
    assert!(error_msg.contains("precision loss"));
    assert!(error_msg.contains("Q16_16"));
    assert!(error_msg.contains("Q8_8"));
}

#[test]
fn test_precision_loss_identity_safe() {
    // Q16_16 → Q16_16 (identity)
    let result = check_precision_loss(FixedPointType::Q16_16, FixedPointType::Q16_16, "identity");
    assert!(result.is_ok());
}

#[test]
fn test_precision_loss_all_upcasts() {
    // All safe upcasts
    assert!(check_precision_loss(FixedPointType::Q8_8, FixedPointType::Q16_16, "upcast").is_ok());
    assert!(check_precision_loss(FixedPointType::Q8_8, FixedPointType::Q32_32, "upcast").is_ok());
    assert!(check_precision_loss(FixedPointType::Q16_16, FixedPointType::Q32_32, "upcast").is_ok());
}

#[test]
fn test_precision_loss_all_downcasts() {
    // All unsafe downcasts
    assert!(
        check_precision_loss(FixedPointType::Q16_16, FixedPointType::Q8_8, "downcast").is_err()
    );
    assert!(
        check_precision_loss(FixedPointType::Q32_32, FixedPointType::Q8_8, "downcast").is_err()
    );
    assert!(
        check_precision_loss(FixedPointType::Q32_32, FixedPointType::Q16_16, "downcast").is_err()
    );
}

// ============================================================================
// FixedPointType Property Tests
// ============================================================================

#[test]
fn test_fixed_point_type_properties() {
    // Q8_8
    assert_eq!(FixedPointType::Q8_8.as_str(), "Q8_8");
    assert_eq!(FixedPointType::Q8_8.integer_bits(), 8);
    assert_eq!(FixedPointType::Q8_8.fractional_bits(), 8);
    assert_eq!(FixedPointType::Q8_8.total_bits(), 16);
    assert_eq!(FixedPointType::Q8_8.storage_type(), "i16");

    // Q16_16
    assert_eq!(FixedPointType::Q16_16.as_str(), "Q16_16");
    assert_eq!(FixedPointType::Q16_16.integer_bits(), 16);
    assert_eq!(FixedPointType::Q16_16.fractional_bits(), 16);
    assert_eq!(FixedPointType::Q16_16.total_bits(), 32);
    assert_eq!(FixedPointType::Q16_16.storage_type(), "i32");

    // Q32_32
    assert_eq!(FixedPointType::Q32_32.as_str(), "Q32_32");
    assert_eq!(FixedPointType::Q32_32.integer_bits(), 32);
    assert_eq!(FixedPointType::Q32_32.fractional_bits(), 32);
    assert_eq!(FixedPointType::Q32_32.total_bits(), 64);
    assert_eq!(FixedPointType::Q32_32.storage_type(), "i64");
}

#[test]
fn test_fixed_point_type_precision_calculation() {
    // Q8_8: 1/256 ≈ 0.00390625
    let q8_precision = FixedPointType::Q8_8.precision();
    assert!((q8_precision - 1.0 / 256.0).abs() < 1e-10);

    // Q16_16: 1/65536 ≈ 0.0000152587890625
    let q16_precision = FixedPointType::Q16_16.precision();
    assert!((q16_precision - 1.0 / 65536.0).abs() < 1e-10);

    // Q32_32: 1/4294967296 ≈ 2.3283064365386963e-10
    let q32_precision = FixedPointType::Q32_32.precision();
    assert!((q32_precision - 1.0 / 4294967296.0).abs() < 1e-15);
}

#[test]
fn test_fixed_point_type_full_path() {
    assert_eq!(
        FixedPointType::Q8_8.full_path(),
        "::atomic_capsule::serialize::fixed_point_impls::Q8_8"
    );
    assert_eq!(
        FixedPointType::Q16_16.full_path(),
        "::atomic_capsule::serialize::fixed_point_impls::Q16_16"
    );
    assert_eq!(
        FixedPointType::Q32_32.full_path(),
        "::atomic_capsule::serialize::fixed_point_impls::Q32_32"
    );
}

// ============================================================================
// PrecisionLoss Tests
// ============================================================================

#[test]
fn test_precision_loss_safe_conversions() {
    // Identity
    assert_eq!(
        FixedPointType::Q8_8.precision_loss_from(FixedPointType::Q8_8),
        PrecisionLoss::None
    );
    assert_eq!(
        FixedPointType::Q16_16.precision_loss_from(FixedPointType::Q16_16),
        PrecisionLoss::None
    );
    assert_eq!(
        FixedPointType::Q32_32.precision_loss_from(FixedPointType::Q32_32),
        PrecisionLoss::None
    );

    // Upcasts
    assert_eq!(
        FixedPointType::Q16_16.precision_loss_from(FixedPointType::Q8_8),
        PrecisionLoss::None
    );
    assert_eq!(
        FixedPointType::Q32_32.precision_loss_from(FixedPointType::Q8_8),
        PrecisionLoss::None
    );
    assert_eq!(
        FixedPointType::Q32_32.precision_loss_from(FixedPointType::Q16_16),
        PrecisionLoss::None
    );
}

#[test]
fn test_precision_loss_unsafe_conversions() {
    // Downcasts
    let loss = FixedPointType::Q8_8.precision_loss_from(FixedPointType::Q16_16);
    assert_eq!(
        loss,
        PrecisionLoss::Unsafe {
            from: FixedPointType::Q16_16,
            to: FixedPointType::Q8_8
        }
    );
    assert!(loss.is_unsafe());
    assert!(!loss.is_safe());

    let loss = FixedPointType::Q8_8.precision_loss_from(FixedPointType::Q32_32);
    assert_eq!(
        loss,
        PrecisionLoss::Unsafe {
            from: FixedPointType::Q32_32,
            to: FixedPointType::Q8_8
        }
    );

    let loss = FixedPointType::Q16_16.precision_loss_from(FixedPointType::Q32_32);
    assert_eq!(
        loss,
        PrecisionLoss::Unsafe {
            from: FixedPointType::Q32_32,
            to: FixedPointType::Q16_16
        }
    );
}

// ============================================================================
// Real-World Usage Examples
// ============================================================================

/// Example: Detect field types in a payment struct
#[test]
fn test_payment_struct_detection() {
    // Simulated field types from a payment struct
    let amount_type = detect_fixed_point_type("Q16_16").unwrap();
    let fee_type = detect_fixed_point_type("Q16_16").unwrap();
    let rate_type = detect_fixed_point_type("Q8_8").unwrap();

    // Verify amount and fee use same type
    assert!(check_type_conflict(amount_type.fp_type, fee_type.fp_type, "fee").is_ok());

    // Mixing Q16_16 and Q8_8 is allowed (different fields)
    // But need to be aware of precision differences
    assert_ne!(amount_type.fp_type, rate_type.fp_type);
}

/// Example: Detect container types for portfolio
#[test]
fn test_portfolio_container_detection() {
    // Portfolio with optional positions
    let positions_type = detect_fixed_point_type("Vec<Q16_16>").unwrap();
    assert_eq!(positions_type.fp_type, FixedPointType::Q16_16);
    assert_eq!(positions_type.container_depth, 1);

    // Optional total (may be None)
    let total_type = detect_fixed_point_type("Option<Q16_16>").unwrap();
    assert_eq!(total_type.fp_type, FixedPointType::Q16_16);
    assert_eq!(total_type.container_depth, 1);

    // Verify both use same underlying type
    assert_eq!(positions_type.fp_type, total_type.fp_type);
}

/// Example: Detect precision loss in aggregation
#[test]
fn test_aggregation_precision_analysis() {
    // Aggregating Q8_8 values into Q16_16 (safe)
    let source = FixedPointType::Q8_8;
    let target = FixedPointType::Q16_16;
    assert!(check_precision_loss(source, target, "aggregation").is_ok());

    // Downsampling Q32_32 to Q16_16 (unsafe)
    let source = FixedPointType::Q32_32;
    let target = FixedPointType::Q16_16;
    assert!(check_precision_loss(source, target, "downsampling").is_err());
}
