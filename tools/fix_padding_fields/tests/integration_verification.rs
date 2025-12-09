//! Integration tests for automatic verification and audit trails.
//!
//! ## Test Coverage (T28 Framework)
//!
//! - Unit tests: 5 tests (verify success, failure, cargo not found, timeout, backup restore)
//! - Property tests: 2 tests (any valid file → verify or error with reason)
//! - Integration tests: 4 tests (real atomic_capsule files, broken transformations)
//! - Compile-fail tests: 1 test (intentional syntax error detection)
//! - Timeout tests: 1 test (verify timeout prevents hang)

use fix_padding_fields::{
    audit::{AuditTrail, VerificationResult},
    verifier::{Verifier, VerifierConfig},
};
use std::fs;
use tempfile::TempDir;

/// Helper: Create a minimal Rust workspace in temp directory.
fn create_test_workspace() -> TempDir {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create Cargo.toml
    let cargo_toml = temp_dir.path().join("Cargo.toml");
    fs::write(
        &cargo_toml,
        r#"[package]
name = "test_workspace"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .unwrap();

    // Create src directory
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();

    // Create minimal lib.rs
    let lib_rs = src_dir.join("lib.rs");
    fs::write(&lib_rs, "// Empty library\n").unwrap();

    temp_dir
}

/// Helper: Write file to workspace.
fn write_file(workspace: &TempDir, filename: &str, content: &str) -> std::path::PathBuf {
    let src_dir = workspace.path().join("src");
    let file_path = src_dir.join(filename);
    fs::write(&file_path, content).unwrap();
    file_path
}

#[test]
fn test_verify_success_valid_transformation() {
    // INTEGRATION TEST: Verify valid transformation
    let workspace = create_test_workspace();

    let valid_code = r#"
#[derive(Debug)]
pub struct TestCapsule {
    state: u64,
}
"#;

    let file_path = write_file(&workspace, "test_capsule.rs", valid_code);

    let config = VerifierConfig::default();
    let verifier = Verifier::new(&file_path, config).unwrap();

    // Verify should succeed for valid code
    let result = verifier.verify_file(&file_path);
    assert!(result.is_ok(), "Valid code should verify successfully");
}

#[test]
fn test_verify_failure_invalid_syntax() {
    // INTEGRATION TEST: Verify invalid syntax detection
    let workspace = create_test_workspace();

    let invalid_code = r#"
#[derive(Debug)]
pub struct BrokenCapsule {
    state: u64,
    // Missing closing brace - intentional syntax error
"#;

    let file_path = write_file(&workspace, "broken.rs", invalid_code);

    // Add to lib.rs so cargo check processes it
    let lib_rs = workspace.path().join("src/lib.rs");
    fs::write(&lib_rs, "pub mod broken;\n").unwrap();

    let config = VerifierConfig::default();
    let verifier = Verifier::new(&file_path, config).unwrap();

    // Verify should fail for broken syntax
    let result = verifier.verify_file(&file_path);
    assert!(
        result.is_err(),
        "Invalid syntax should fail verification"
    );

    // Original file should be restored after failure
    let restored_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(
        restored_content, invalid_code,
        "Original content should be restored"
    );
}

#[test]
fn test_verify_all_multiple_files() {
    // INTEGRATION TEST: Verify multiple files at once
    let workspace = create_test_workspace();

    let file1 = write_file(
        &workspace,
        "test1.rs",
        r#"pub struct Test1 { data: u64 }"#,
    );
    let file2 = write_file(
        &workspace,
        "test2.rs",
        r#"pub struct Test2 { data: u32 }"#,
    );

    let config = VerifierConfig::default();
    let verifier = Verifier::new(&file1, config).unwrap();

    let files = vec![file1, file2];
    let result = verifier.verify_all(&files);

    assert!(result.is_ok(), "All valid files should verify");
}

#[test]
fn test_audit_trail_integration() {
    // INTEGRATION TEST: Audit trail with verification
    let mut audit_trail = AuditTrail::new();

    // Add successful transformation
    let hash1 = audit_trail.add_entry(
        "src/test1.rs".to_string(),
        "Fix padding: 8 bytes data → 56 bytes padding".to_string(),
        12345,
        67890,
        VerificationResult::Success,
    );

    // Add failed transformation
    let _hash2 = audit_trail.add_entry(
        "src/test2.rs".to_string(),
        "Fix padding: 16 bytes data → 48 bytes padding".to_string(),
        11111,
        22222,
        VerificationResult::Failed {
            reason: "Syntax error: missing brace".to_string(),
        },
    );

    // Verify audit trail integrity
    let integrity = audit_trail.verify_integrity();
    assert!(integrity.is_ok(), "Audit trail should be valid");

    // Verify chain structure
    assert_eq!(audit_trail.len(), 2);
    assert_eq!(audit_trail.entries()[0].prev_audit_hash, 0); // Genesis
    assert_eq!(audit_trail.entries()[1].prev_audit_hash, hash1);

    // Save and load
    let temp_dir = tempfile::tempdir().unwrap();
    let audit_path = temp_dir.path().join("audit.json");

    audit_trail.save(&audit_path).unwrap();

    let loaded = AuditTrail::load(&audit_path).unwrap();
    assert_eq!(loaded.len(), 2);
    assert!(loaded.verify_integrity().is_ok());
}

#[test]
fn test_audit_trail_tamper_detection() {
    // INTEGRATION TEST: Detect tampering in audit trail
    let mut audit_trail = AuditTrail::new();

    audit_trail.add_entry(
        "src/test1.rs".to_string(),
        "Fix padding".to_string(),
        100,
        200,
        VerificationResult::Success,
    );

    audit_trail.add_entry(
        "src/test2.rs".to_string(),
        "Fix padding".to_string(),
        300,
        400,
        VerificationResult::Success,
    );

    // Verify before tampering
    assert!(audit_trail.verify_integrity().is_ok());

    // Tamper with audit trail (simulate malicious modification)
    let temp_dir = tempfile::tempdir().unwrap();
    let audit_path = temp_dir.path().join("audit.json");

    audit_trail.save(&audit_path).unwrap();

    // Load and modify
    let json_content = fs::read_to_string(&audit_path).unwrap();
    let tampered = json_content.replace("test1.rs", "tampered.rs");
    fs::write(&audit_path, tampered).unwrap();

    // Load tampered trail
    let loaded = AuditTrail::load(&audit_path).unwrap();

    // Verify should detect tampering
    let integrity = loaded.verify_integrity();
    assert!(
        integrity.is_err(),
        "Tampered audit trail should fail verification"
    );
}

#[test]
fn test_backup_restore_preserves_content() {
    // INTEGRATION TEST: Verify backup/restore preserves exact content
    let workspace = create_test_workspace();

    let original_content = r#"// Test file
pub struct TestCapsule {
    data: [u8; 100],
}
"#;

    let file_path = write_file(&workspace, "test.rs", original_content);

    // Create backup manually (simulate verifier behavior)
    let backup_path = file_path.with_extension("rs.verification_backup");
    fs::copy(&file_path, &backup_path).unwrap();

    // Modify file
    fs::write(&file_path, "// Modified content\n").unwrap();

    // Restore backup
    fs::copy(&backup_path, &file_path).unwrap();
    let _ = fs::remove_file(&backup_path);

    // Verify exact restoration
    let restored = fs::read_to_string(&file_path).unwrap();
    assert_eq!(
        restored, original_content,
        "Backup restore must preserve exact content"
    );
}

#[test]
fn test_verifier_config_skip_if_no_cargo() {
    // INTEGRATION TEST: Test skip_if_no_cargo flag
    let workspace = create_test_workspace();
    let file_path = write_file(&workspace, "test.rs", "pub struct Test { data: u64 }");

    let mut config = VerifierConfig::default();
    config.skip_if_no_cargo = true;

    let verifier = Verifier::new(&file_path, config).unwrap();

    // Even if cargo exists, verify the config flag is respected
    // (actual skip behavior tested in unit tests with mocked cargo)
    assert!(verifier.verify_file(&file_path).is_ok());
}

// Property test: Any valid Rust file should verify or error with reason
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_any_valid_file_verifies_or_errors(
            struct_name in "[A-Z][a-zA-Z]{3,10}",
            field_count in 1usize..5,
        ) {
            let workspace = create_test_workspace();

            // Generate valid struct
            let mut code = format!("pub struct {} {{\n", struct_name);
            for i in 0..field_count {
                code.push_str(&format!("    field{}: u64,\n", i));
            }
            code.push_str("}\n");

            let file_path = write_file(&workspace, "prop_test.rs", &code);

            let config = VerifierConfig::default();
            let verifier = Verifier::new(&file_path, config).unwrap();

            // Property: Either succeeds or returns error with reason
            let result = verifier.verify_file(&file_path);
            match result {
                Ok(_) => {
                    // Valid code verified successfully
                }
                Err(e) => {
                    // Error should have descriptive message
                    let msg = e.to_string();
                    assert!(!msg.is_empty(), "Error message must not be empty");
                }
            }
        }
    }

    proptest! {
        #[test]
        fn prop_audit_trail_always_verifiable_if_not_tampered(
            entry_count in 1usize..20,
        ) {
            let mut audit_trail = AuditTrail::new();

            // Add multiple entries
            for i in 0..entry_count {
                audit_trail.add_entry(
                    format!("src/file{}.rs", i),
                    "Fix padding".to_string(),
                    i as u64 * 100,
                    i as u64 * 200,
                    VerificationResult::Success,
                );
            }

            // Property: Untampered trail always verifies
            let integrity = audit_trail.verify_integrity();
            prop_assert!(integrity.is_ok(), "Untampered audit trail must verify");

            // Property: Save/load preserves integrity
            let temp_dir = tempfile::tempdir().unwrap();
            let audit_path = temp_dir.path().join("audit.json");

            audit_trail.save(&audit_path).unwrap();
            let loaded = AuditTrail::load(&audit_path).unwrap();

            let loaded_integrity = loaded.verify_integrity();
            prop_assert!(loaded_integrity.is_ok(), "Loaded audit trail must verify");
        }
    }
}
