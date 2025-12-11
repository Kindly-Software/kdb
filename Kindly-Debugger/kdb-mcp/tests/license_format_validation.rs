//! Integration tests for format-based license validation
//!
//! These tests verify that OAuth-provisioned licenses (KDB-HOBBY-*, etc.)
//! pass validation without needing to be pre-registered in LicenseValidatorCapsule.

use kdb_mcp::LicenseValidatorCapsule;

#[test]
fn test_oauth_hobby_license_validation() {
    let validator = LicenseValidatorCapsule::new();
    // No admin license set - simulates production OAuth flow

    // OAuth provisions licenses like: KDB-HOBBY-{timestamp}-{hash}
    assert!(validator.validate_key("KDB-HOBBY-693ace9a-1"));
    assert!(validator.validate_key("KDB-HOBBY-1234567890-abcd1234"));

    let stats = validator.get_stats();
    assert_eq!(stats.validation_success, 2);
    assert_eq!(stats.validation_failed, 0);
}

#[test]
fn test_all_tier_formats_accepted() {
    let validator = LicenseValidatorCapsule::new();

    // All tier formats should pass
    assert!(validator.validate_key("KDB-HOBBY-abc-123"));
    assert!(validator.validate_key("KDB-PRO-xyz-789"));
    assert!(validator.validate_key("KDB-ENGINEER-dev-456"));
    assert!(validator.validate_key("KDB-TEAMS-team-001"));
    assert!(validator.validate_key("KDB-ENTERPRISE-ent-999"));

    let stats = validator.get_stats();
    assert_eq!(stats.validation_success, 5);
    assert_eq!(stats.validation_failed, 0);
}

#[test]
fn test_invalid_format_rejected() {
    let validator = LicenseValidatorCapsule::new();

    // Wrong prefix
    assert!(!validator.validate_key("NOTDB-HOBBY-123"));

    // Unknown tier
    assert!(!validator.validate_key("KDB-UNKNOWN-123"));
    assert!(!validator.validate_key("KDB-STARTER-123")); // Old tier name

    // Too short
    assert!(!validator.validate_key("KDB-H-1"));

    // Missing components
    assert!(!validator.validate_key("KDB-HOBBY"));

    let stats = validator.get_stats();
    assert_eq!(stats.validation_success, 0);
    assert_eq!(stats.validation_failed, 5);
}

#[test]
fn test_admin_override_still_works() {
    let validator = LicenseValidatorCapsule::new();

    // Set admin license
    let admin_key = "KDB-ADMIN-special-override";
    let expiry = 2000000000; // Year 2033
    validator.set_license(admin_key, expiry);

    // Admin key should pass via hash match
    assert!(validator.validate_key(admin_key));

    // Other KDB-* keys should ALSO pass via format validation (fallback)
    assert!(validator.validate_key("KDB-HOBBY-user-123"));
    assert!(validator.validate_key("KDB-PRO-user-456"));

    let stats = validator.get_stats();
    assert_eq!(stats.validation_success, 3);
    assert_eq!(stats.validation_failed, 0);
}

#[test]
fn test_mcp_protocol_methods_bypass_auth() {
    // This test documents that initialize/ping bypass auth at the HTTP transport layer
    // (handled by HttpTransportCapsule.is_protocol_method()), not by license validation

    let validator = LicenseValidatorCapsule::new();

    // License validation is NOT called for protocol methods
    // They are detected and allowed before reaching this layer

    // Regular tool calls DO require valid licenses
    assert!(validator.validate_key("KDB-HOBBY-abc-123"));

    let stats = validator.get_stats();
    assert_eq!(stats.validation_success, 1);
}
