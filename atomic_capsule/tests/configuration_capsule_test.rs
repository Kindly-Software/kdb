// Integration test for ConfigurationCapsule
#![allow(non_snake_case)]

use atomic_capsule::{ConfigurationCapsule, Q16Fixed};

#[test]
fn test_q16_basic_conversion() {
    let q = Q16Fixed::from_f64(1.5);
    assert_eq!(q.bits(), 98304);
    assert_eq!(q.to_f64(), 1.5);
}

#[test]
fn test_configuration_new() {
    let config = ConfigurationCapsule::new();
    assert_eq!(config.threshold_f64(), 0.0);
    assert_eq!(config.threads(), 1);
    assert_eq!(config.memory_limit_mb(), 0);
    assert_eq!(config.feature_flags(), 0);
}

#[test]
fn test_configuration_set_threshold() {
    let config = ConfigurationCapsule::new().set_threshold(2.5);
    assert_eq!(config.threshold_f64(), 2.5);
}

#[test]
fn test_configuration_set_threads() {
    let config = ConfigurationCapsule::new().set_threads(8);
    assert_eq!(config.threads(), 8);
}

#[test]
fn test_configuration_set_memory_limit() {
    let config = ConfigurationCapsule::new().set_memory_limit_mb(512);
    assert_eq!(config.memory_limit_mb(), 512);
}

#[test]
fn test_feature_enable() {
    let config = ConfigurationCapsule::new().enable_feature(ConfigurationCapsule::FEATURE_SIMD);
    assert!(config.is_feature_enabled(ConfigurationCapsule::FEATURE_SIMD));
}

#[test]
fn test_checksum_valid() {
    let config = ConfigurationCapsule::new().set_threshold(1.5).set_threads(4);
    assert!(config.is_valid());
}

#[test]
fn test_deterministic_round_trip() {
    for &value in &[0.0, 1.0, 1.5, 2.25, 10.5, -1.0, -3.75] {
        let config = ConfigurationCapsule::new().set_threshold(value);
        assert_eq!(config.threshold_f64(), value, "Failed for value {}", value);
    }
}

#[test]
fn test_alignment() {
    assert_eq!(std::mem::size_of::<ConfigurationCapsule>(), 128);
    assert_eq!(std::mem::align_of::<ConfigurationCapsule>(), 128);
}

#[test]
fn test_full_configuration_workflow() {
    let config = ConfigurationCapsule::new()
        .set_threshold(1.5)
        .set_threads(16)
        .set_memory_limit_mb(1024)
        .enable_feature(ConfigurationCapsule::FEATURE_SIMD)
        .enable_feature(ConfigurationCapsule::FEATURE_AUDIT_TRAIL)
        .enable_feature(ConfigurationCapsule::FEATURE_COMPRESSION);

    assert_eq!(config.threshold_f64(), 1.5);
    assert_eq!(config.threads(), 16);
    assert_eq!(config.memory_limit_mb(), 1024);
    assert!(config.is_feature_enabled(ConfigurationCapsule::FEATURE_SIMD));
    assert!(config.is_feature_enabled(ConfigurationCapsule::FEATURE_AUDIT_TRAIL));
    assert!(config.is_feature_enabled(ConfigurationCapsule::FEATURE_COMPRESSION));
    assert!(config.is_valid());
}

#[test]
fn test_copy_semantics() {
    let config1 = ConfigurationCapsule::new().set_threshold(2.5).set_threads(8);
    let config2 = config1; // Copy

    assert_eq!(config1, config2);
}

#[test]
fn test_feature_toggle() {
    let config = ConfigurationCapsule::new()
        .toggle_feature(ConfigurationCapsule::FEATURE_SIMD);
    assert!(config.is_feature_enabled(ConfigurationCapsule::FEATURE_SIMD));

    let config = config.toggle_feature(ConfigurationCapsule::FEATURE_SIMD);
    assert!(!config.is_feature_enabled(ConfigurationCapsule::FEATURE_SIMD));
}

#[test]
fn test_threads_bounds_min() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ConfigurationCapsule::new().set_threads(0);
    }));
    assert!(result.is_err(), "threads = 0 should panic");
}

#[test]
fn test_threads_bounds_max() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ConfigurationCapsule::new().set_threads(257);
    }));
    assert!(result.is_err(), "threads = 257 should panic");
}

#[test]
fn test_threads_bounds_valid() {
    let config1 = ConfigurationCapsule::new().set_threads(1);
    assert_eq!(config1.threads(), 1);

    let config256 = ConfigurationCapsule::new().set_threads(256);
    assert_eq!(config256.threads(), 256);
}

#[test]
fn test_checksum_deterministic() {
    let config1 = ConfigurationCapsule::new()
        .set_threshold(3.14)
        .set_threads(8)
        .set_memory_limit_mb(512);

    let config2 = ConfigurationCapsule::new()
        .set_threshold(3.14)
        .set_threads(8)
        .set_memory_limit_mb(512);

    assert_eq!(config1, config2);
}

#[test]
fn test_q16_integer_part() {
    let q = Q16Fixed::from_f64(5.5);
    assert_eq!(q.integer_part(), 5);
}

#[test]
fn test_q16_fractional_part() {
    let q = Q16Fixed::from_f64(1.5);
    let frac = q.fractional_part();
    assert_eq!(frac, 32768); // 0.5 * 65536
}

#[test]
fn test_q16_predicates() {
    let pos = Q16Fixed::from_f64(1.0);
    assert!(pos.is_positive());
    assert!(!pos.is_negative());
    assert!(!pos.is_zero());

    let neg = Q16Fixed::from_f64(-1.0);
    assert!(neg.is_negative());
    assert!(!neg.is_positive());
    assert!(!neg.is_zero());

    let zero = Q16Fixed::from_f64(0.0);
    assert!(zero.is_zero());
    assert!(!zero.is_positive());
    assert!(!zero.is_negative());
}
