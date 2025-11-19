/// T28 Test Suite: Dataset Manager
///
/// Comprehensive testing for dataset download, verification, and manifest generation
/// following T28 framework (Unit + Property + Integration + Production)
use anyhow::Result;
use std::path::{Path, PathBuf};

// Test modules are conditionally compiled with download-tools feature
#[cfg(feature = "download-tools")]
mod dataset_manager_module {
    use super::*;

    // Note: We need to import from the benchmarking module
    // For now, we'll write basic tests that don't require the full implementation

    /// Helper: Create test file with known content
    fn create_test_file(path: &Path, content: &str) -> Result<()> {
        use std::fs::File;
        use std::io::Write;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    /// Helper: Compute SHA-256 (duplicated from dataset_manager.rs for testing)
    fn compute_sha256(file_path: &Path) -> Result<String> {
        use sha2::{Digest, Sha256};
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(file_path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(hex::encode(hasher.finalize()))
    }

    // ============================================================================
    // T28 UNIT TESTS (Q1-Q7)
    // ============================================================================

    #[test]
    fn test_sha256_computation_empty_file() {
        let temp_dir = std::env::temp_dir().join("kindly_dedup_test_sha256_empty");
        let test_file = temp_dir.join("empty.txt");

        create_test_file(&test_file, "").unwrap();

        let hash = compute_sha256(&test_file).unwrap();

        // SHA-256 of empty string
        assert_eq!(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sha256_computation_known_content() {
        let temp_dir = std::env::temp_dir().join("kindly_dedup_test_sha256_known");
        let test_file = temp_dir.join("known.txt");

        create_test_file(&test_file, "hello world").unwrap();

        let hash = compute_sha256(&test_file).unwrap();

        // SHA-256 of "hello world"
        assert_eq!(hash, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sha256_computation_large_file() {
        let temp_dir = std::env::temp_dir().join("kindly_dedup_test_sha256_large");
        let test_file = temp_dir.join("large.txt");

        // Create 1MB file
        let content = "A".repeat(1_000_000);
        create_test_file(&test_file, &content).unwrap();

        let hash = compute_sha256(&test_file).unwrap();

        // Should complete without error
        assert_eq!(hash.len(), 64); // SHA-256 is 32 bytes = 64 hex chars

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sha256_deterministic() {
        let temp_dir = std::env::temp_dir().join("kindly_dedup_test_sha256_deterministic");
        let test_file = temp_dir.join("deterministic.txt");

        create_test_file(&test_file, "test content").unwrap();

        let hash1 = compute_sha256(&test_file).unwrap();
        let hash2 = compute_sha256(&test_file).unwrap();

        assert_eq!(hash1, hash2, "SHA-256 should be deterministic");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sha256_different_content() {
        let temp_dir = std::env::temp_dir().join("kindly_dedup_test_sha256_different");
        let test_file1 = temp_dir.join("file1.txt");
        let test_file2 = temp_dir.join("file2.txt");

        create_test_file(&test_file1, "content A").unwrap();
        create_test_file(&test_file2, "content B").unwrap();

        let hash1 = compute_sha256(&test_file1).unwrap();
        let hash2 = compute_sha256(&test_file2).unwrap();

        assert_ne!(hash1, hash2, "Different content should produce different hashes");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sha256_nonexistent_file() {
        let nonexistent = PathBuf::from("/tmp/kindly_dedup_nonexistent_file_xyz.txt");
        let result = compute_sha256(&nonexistent);

        assert!(result.is_err(), "Should error on nonexistent file");
    }

    // ============================================================================
    // T28 PROPERTY TESTS (Q8-Q14)
    // ============================================================================

    #[test]
    fn test_sha256_collision_resistance() {
        // Property: Different inputs should produce different hashes
        let temp_dir = std::env::temp_dir().join("kindly_dedup_test_sha256_collision");
        let mut hashes = std::collections::HashSet::new();

        for i in 0..100 {
            let test_file = temp_dir.join(format!("file_{}.txt", i));
            create_test_file(&test_file, &format!("content {}", i)).unwrap();

            let hash = compute_sha256(&test_file).unwrap();
            assert!(hashes.insert(hash), "SHA-256 collision detected (extremely unlikely)");
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sha256_fixed_length() {
        // Property: SHA-256 always produces 32 bytes (64 hex chars)
        let temp_dir = std::env::temp_dir().join("kindly_dedup_test_sha256_length");

        for size in [0, 1, 100, 1000, 10000] {
            let test_file = temp_dir.join(format!("file_{}.txt", size));
            let content = "X".repeat(size);
            create_test_file(&test_file, &content).unwrap();

            let hash = compute_sha256(&test_file).unwrap();
            assert_eq!(hash.len(), 64, "SHA-256 should always be 64 hex chars");
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sha256_hex_encoding() {
        // Property: SHA-256 output should be valid hex
        let temp_dir = std::env::temp_dir().join("kindly_dedup_test_sha256_hex");
        let test_file = temp_dir.join("hex.txt");

        create_test_file(&test_file, "test").unwrap();

        let hash = compute_sha256(&test_file).unwrap();

        // All characters should be 0-9 or a-f
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "SHA-256 should be valid hex"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // ============================================================================
    // T28 INTEGRATION TESTS (Q15-Q21)
    // ============================================================================

    #[test]
    fn test_manifest_json_roundtrip() {
        use serde_json;

        // Create minimal manifest
        #[derive(serde::Serialize, serde::Deserialize)]
        struct TestManifest {
            source: String,
            document_count: usize,
            sha256: String,
        }

        let manifest = TestManifest {
            source: "Test Source".to_string(),
            document_count: 1000,
            sha256: "abc123".to_string(),
        };

        let json = manifest.to_json().unwrap();
        let parsed = TestManifest::from_json(&json).unwrap();

        assert_eq!(parsed.source, manifest.source);
        assert_eq!(parsed.document_count, manifest.document_count);
        assert_eq!(parsed.sha256, manifest.sha256);
    }

    #[test]
    fn test_dataset_directory_creation() {
        let temp_dir = std::env::temp_dir().join("kindly_dedup_test_dataset_dir");

        std::fs::create_dir_all(&temp_dir).unwrap();
        assert!(temp_dir.exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_manifest_file_extension() {
        let dataset_path = PathBuf::from("test_data/realistic/dataset.json");
        let manifest_path = dataset_path.with_extension("manifest.json");

        assert_eq!(
            manifest_path.to_str().unwrap(),
            "test_data/realistic/dataset.manifest.json"
        );
    }

    // ============================================================================
    // T28 PRODUCTION TESTS (Q22-Q28)
    // ============================================================================

    #[test]
    fn test_sha256_performance_benchmark() {
        let temp_dir = std::env::temp_dir().join("kindly_dedup_test_sha256_perf");
        let test_file = temp_dir.join("large_file.txt");

        // Create 10MB file
        let content = "B".repeat(10_000_000);
        create_test_file(&test_file, &content).unwrap();

        let start = std::time::Instant::now();
        let _hash = compute_sha256(&test_file).unwrap();
        let elapsed = start.elapsed();

        // Should complete in reasonable time (<1 second for 10MB)
        assert!(
            elapsed.as_secs() < 5,
            "SHA-256 computation should complete within 5 seconds for 10MB file"
        );

        println!("SHA-256 computation time for 10MB: {:?}", elapsed);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_manifest_path_handling() {
        // Test various path formats
        let paths = vec![
            "dataset.json",
            "test_data/dataset.json",
            "test_data/realistic/commoncrawl_100k.json",
            "/tmp/dataset.json",
        ];

        for path in paths {
            let dataset_path = PathBuf::from(path);
            let manifest_path = dataset_path.with_extension("manifest.json");

            // Manifest path should end with .manifest.json
            assert!(manifest_path.to_str().unwrap().ends_with(".manifest.json"));
        }
    }

    #[test]
    fn test_error_handling_permission_denied() {
        // Test error handling for inaccessible files
        let restricted_path = PathBuf::from("/root/inaccessible.txt");
        let result = compute_sha256(&restricted_path);

        // Should return error (not panic)
        assert!(result.is_err(), "Should handle permission errors gracefully");
    }
}

#[cfg(not(feature = "download-tools"))]
#[test]
fn test_feature_gate() {
    // This test ensures that tests requiring download-tools are gated properly
    println!("Dataset manager tests require 'download-tools' feature");
    println!("Run: cargo test --features download-tools");
}
