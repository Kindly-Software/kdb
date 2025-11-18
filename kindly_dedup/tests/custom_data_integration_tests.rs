//! Custom Data Integration Tests (T28 Framework)
//!
//! **Purpose**: Comprehensive tests for custom data loading functionality
//!
//! **T28 Framework Coverage**:
//! - Q1-Q7: Unit tests (format detection, parsing, error handling)
//! - Q8-Q14: Property tests (reproducibility, correctness)
//! - Q15-Q21: Integration tests (full custom data flow)
//! - Q22-Q28: Production tests (500K docs, performance - see client_demo.rs)
//!
//! **Test Files** (in test_data/custom_format/):
//! - test_corpus.jsonl: 10 documents with duplicates (JSONL format)
//! - test_corpus.json: Same 10 documents (JSON array format)
//! - test_corpus.txt: Same 10 documents (plain text format)
//! - test_invalid.jsonl: Malformed JSONL (3 valid, 2 invalid lines)
//! - test_empty.txt: Empty file
//!
//! **Expected Duplicates** (in test corpus):
//! - doc_0 = doc_2 (exact duplicate)
//! - doc_1 = doc_4 (exact duplicate)
//! - doc_3 = doc_8 (exact duplicate)

use kindly_dedup::custom_data::{
    detect_format, load_custom_corpus, load_json, load_jsonl, load_plaintext, CustomDataError, Document, FileFormat,
};
use std::path::PathBuf;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Get test data directory
fn test_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_data")
        .join("custom_format")
}

/// Get test corpus path
fn test_corpus_path(filename: &str) -> PathBuf {
    test_data_dir().join(filename)
}

// ============================================================================
// T28 Q1-Q7: UNIT TESTS
// ============================================================================

#[test]
fn test_unit_format_detection_jsonl() {
    // Q1: Core behavior - format detection
    let path = test_corpus_path("test_corpus.jsonl");
    let format = detect_format(&path).unwrap();
    assert_eq!(format, FileFormat::Jsonl);
}

#[test]
fn test_unit_format_detection_json() {
    // Q1: Core behavior - format detection
    let path = test_corpus_path("test_corpus.json");
    let format = detect_format(&path).unwrap();
    assert_eq!(format, FileFormat::Json);
}

#[test]
fn test_unit_format_detection_txt() {
    // Q1: Core behavior - format detection
    let path = test_corpus_path("test_corpus.txt");
    let format = detect_format(&path).unwrap();
    assert_eq!(format, FileFormat::PlainText);
}

#[test]
fn test_unit_format_detection_unknown() {
    // Q2: Edge case - unknown format
    let path = PathBuf::from("test.csv");
    let result = detect_format(&path);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CustomDataError::UnknownFormat(_)));
}

#[test]
fn test_unit_load_jsonl_valid() {
    // Q1: Core behavior - load valid JSONL
    let path = test_corpus_path("test_corpus.jsonl");
    let docs = load_jsonl(&path, None).unwrap();

    assert_eq!(docs.len(), 10);
    assert_eq!(docs[0].id, 0);
    assert!(docs[0].text.starts_with("Machine learning"));
    assert_eq!(docs[9].id, 9);
}

#[test]
fn test_unit_load_json_valid() {
    // Q1: Core behavior - load valid JSON array
    let path = test_corpus_path("test_corpus.json");
    let docs = load_json(&path, None).unwrap();

    assert_eq!(docs.len(), 10);
    assert_eq!(docs[0].id, 0);
    assert!(docs[0].text.starts_with("Machine learning"));
}

#[test]
fn test_unit_load_plaintext_valid() {
    // Q1: Core behavior - load valid plain text
    let path = test_corpus_path("test_corpus.txt");
    let docs = load_plaintext(&path, None).unwrap();

    assert_eq!(docs.len(), 10);
    assert_eq!(docs[0].id, 0);
    assert!(docs[0].text.starts_with("Machine learning"));
}

#[test]
fn test_unit_load_invalid_jsonl_partial() {
    // Q2: Edge case - partially invalid JSONL
    let path = test_corpus_path("test_invalid.jsonl");

    // Note: Current implementation stops at first error
    // This behavior is documented in CustomDataError::InvalidJsonl
    let result = load_jsonl(&path, None);

    // Should get error due to invalid line 2
    assert!(result.is_err());

    match result.unwrap_err() {
        CustomDataError::InvalidJsonl { line, .. } => {
            assert_eq!(line, 2); // Second line is first invalid
        }
        _ => panic!("Expected InvalidJsonl error"),
    }
}

#[test]
fn test_unit_load_empty_file() {
    // Q2: Edge case - empty file
    let path = test_corpus_path("test_empty.txt");
    let result = load_plaintext(&path, None);

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CustomDataError::EmptyFile(_)));
}

#[test]
fn test_unit_file_not_found() {
    // Q3: Error handling - file not found
    let path = test_data_dir().join("nonexistent.jsonl");
    let result = load_jsonl(&path, None);

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CustomDataError::FileNotFound(_)));
}

#[test]
fn test_unit_error_messages_friendly() {
    // Q3: Error handling - friendly error messages
    let path = test_data_dir().join("nonexistent.jsonl");
    let result = load_jsonl(&path, None);

    match result {
        Err(CustomDataError::FileNotFound(msg)) => {
            // Error message should be helpful
            assert!(msg.contains("nonexistent.jsonl"));
        }
        _ => panic!("Expected FileNotFound error"),
    }
}

// ============================================================================
// T28 Q8-Q14: PROPERTY TESTS
// ============================================================================

#[test]
fn test_property_reproducibility_jsonl() {
    // Q8: Reproducibility - same input produces same output
    let path = test_corpus_path("test_corpus.jsonl");

    let docs1 = load_jsonl(&path, None).unwrap();
    let docs2 = load_jsonl(&path, None).unwrap();

    assert_eq!(docs1.len(), docs2.len());
    for (d1, d2) in docs1.iter().zip(docs2.iter()) {
        assert_eq!(d1.id, d2.id);
        assert_eq!(d1.text, d2.text);
    }
}

#[test]
fn test_property_reproducibility_json() {
    // Q8: Reproducibility - same input produces same output
    let path = test_corpus_path("test_corpus.json");

    let docs1 = load_json(&path, None).unwrap();
    let docs2 = load_json(&path, None).unwrap();

    assert_eq!(docs1.len(), docs2.len());
    for (d1, d2) in docs1.iter().zip(docs2.iter()) {
        assert_eq!(d1.id, d2.id);
        assert_eq!(d1.text, d2.text);
    }
}

#[test]
fn test_property_format_consistency() {
    // Q9: Correctness - all 3 formats load same content
    let jsonl_docs = load_jsonl(&test_corpus_path("test_corpus.jsonl"), None).unwrap();
    let json_docs = load_json(&test_corpus_path("test_corpus.json"), None).unwrap();
    let txt_docs = load_plaintext(&test_corpus_path("test_corpus.txt"), None).unwrap();

    assert_eq!(jsonl_docs.len(), json_docs.len());
    assert_eq!(jsonl_docs.len(), txt_docs.len());

    // Verify content matches (text should be identical)
    for i in 0..jsonl_docs.len() {
        assert_eq!(jsonl_docs[i].text, json_docs[i].text);
        assert_eq!(jsonl_docs[i].text, txt_docs[i].text);
    }
}

#[test]
fn test_property_document_count_accuracy() {
    // Q10: Correctness - document count matches expected
    let path = test_corpus_path("test_corpus.jsonl");
    let docs = load_jsonl(&path, None).unwrap();

    // Test corpus has exactly 10 documents
    assert_eq!(docs.len(), 10);
}

#[test]
fn test_property_no_data_loss() {
    // Q11: Correctness - no data is lost during loading
    let path = test_corpus_path("test_corpus.jsonl");
    let docs = load_jsonl(&path, None).unwrap();

    // Verify all documents have non-empty text
    for doc in &docs {
        assert!(!doc.text.is_empty(), "Document {} has empty text", doc.id);
        assert!(doc.text.len() > 10, "Document {} text too short", doc.id);
    }
}

#[test]
fn test_property_duplicate_detection_accuracy() {
    // Q12: Correctness - can detect known duplicates
    let path = test_corpus_path("test_corpus.jsonl");
    let docs = load_jsonl(&path, None).unwrap();

    // Known duplicates in test corpus:
    // doc_0 = doc_2 (identical text)
    // doc_1 = doc_4 (identical text)
    // doc_3 = doc_8 (identical text)

    assert_eq!(docs[0].text, docs[2].text, "doc_0 should equal doc_2");
    assert_eq!(docs[1].text, docs[4].text, "doc_1 should equal doc_4");
    assert_eq!(docs[3].text, docs[8].text, "doc_3 should equal doc_8");
}

#[test]
fn test_property_unique_documents() {
    // Q13: Correctness - unique documents remain unique
    let path = test_corpus_path("test_corpus.jsonl");
    let docs = load_jsonl(&path, None).unwrap();

    // doc_5, doc_6, doc_7, doc_9 should all be unique
    let unique_ids = vec![5, 6, 7, 9];

    for i in &unique_ids {
        for j in &unique_ids {
            if i != j {
                assert_ne!(docs[*i].text, docs[*j].text, "doc_{} should not equal doc_{}", i, j);
            }
        }
    }
}

// ============================================================================
// T28 Q15-Q21: INTEGRATION TESTS
// ============================================================================

#[test]
fn test_integration_auto_detect_and_load_jsonl() {
    // Q15: Integration - full flow with auto-detection
    let path = test_corpus_path("test_corpus.jsonl");
    let docs = load_custom_corpus(&path, None).unwrap();

    assert_eq!(docs.len(), 10);
    assert_eq!(docs[0].text, docs[2].text); // Duplicate check
}

#[test]
fn test_integration_auto_detect_and_load_json() {
    // Q15: Integration - full flow with auto-detection
    let path = test_corpus_path("test_corpus.json");
    let docs = load_custom_corpus(&path, None).unwrap();

    assert_eq!(docs.len(), 10);
    assert_eq!(docs[1].text, docs[4].text); // Duplicate check
}

#[test]
fn test_integration_auto_detect_and_load_txt() {
    // Q15: Integration - full flow with auto-detection
    let path = test_corpus_path("test_corpus.txt");
    let docs = load_custom_corpus(&path, None).unwrap();

    assert_eq!(docs.len(), 10);
    assert_eq!(docs[3].text, docs[8].text); // Duplicate check
}

#[test]
fn test_integration_with_dedup_pipeline() {
    // Q16: Integration - load + deduplicate
    use kindly_dedup::DedupPipeline;

    let path = test_corpus_path("test_corpus.jsonl");
    let docs = load_custom_corpus(&path, None).unwrap();

    let mut pipeline = DedupPipeline::new(docs.len());
    for doc in &docs {
        pipeline.add_document(doc.id, &doc.text).unwrap();
    }

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Should find 3 duplicate pairs: (0,2), (1,4), (3,8)
    // This means 3 clusters of size 2 each
    assert!(
        clusters.len() >= 3,
        "Expected at least 3 clusters, got {}",
        clusters.len()
    );

    // Verify specific duplicates are in same clusters
    let mut found_duplicates = 0;
    for cluster in &clusters {
        if cluster.len() == 2 {
            let pair = (cluster[0], cluster[1]);
            if pair == (0, 2) || pair == (1, 4) || pair == (3, 8) {
                found_duplicates += 1;
            }
        }
    }

    assert!(
        found_duplicates >= 2,
        "Should find at least 2 of the 3 known duplicate pairs"
    );
}

#[test]
fn test_integration_error_recovery() {
    // Q17: Integration - graceful error handling
    let nonexistent = test_data_dir().join("does_not_exist.jsonl");
    let result = load_custom_corpus(&nonexistent, None);

    assert!(result.is_err());

    // Error should be descriptive
    match result.unwrap_err() {
        CustomDataError::FileNotFound(msg) => {
            assert!(msg.contains("does_not_exist.jsonl"));
        }
        _ => panic!("Expected FileNotFound error"),
    }
}

#[test]
fn test_integration_empty_file_handling() {
    // Q18: Integration - empty file graceful handling
    let path = test_corpus_path("test_empty.txt");
    let result = load_custom_corpus(&path, None);

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CustomDataError::EmptyFile(_)));
}

#[test]
fn test_integration_mixed_format_detection() {
    // Q19: Integration - correct format detection for all types
    let formats = vec![
        ("test_corpus.jsonl", FileFormat::Jsonl),
        ("test_corpus.json", FileFormat::Json),
        ("test_corpus.txt", FileFormat::PlainText),
    ];

    for (filename, expected) in formats {
        let path = test_corpus_path(filename);
        let detected = detect_format(&path).unwrap();
        assert_eq!(detected, expected, "Failed for {}", filename);
    }
}

#[test]
fn test_integration_backward_compatibility() {
    // Q20: Integration - new loader doesn't break existing code
    // Load using old method (direct function calls)
    let path_jsonl = test_corpus_path("test_corpus.jsonl");
    let docs_old = load_jsonl(&path_jsonl, None).unwrap();

    // Load using new method (auto-detect)
    let docs_new = load_custom_corpus(&path_jsonl, None).unwrap();

    // Results should be identical
    assert_eq!(docs_old.len(), docs_new.len());
    for (old, new) in docs_old.iter().zip(docs_new.iter()) {
        assert_eq!(old.id, new.id);
        assert_eq!(old.text, new.text);
    }
}

#[test]
fn test_integration_case_insensitive_extensions() {
    // Q21: Integration - handle uppercase/mixed-case extensions
    use std::fs;
    use tempfile::NamedTempFile;

    // Create temp file with uppercase extension
    let mut file = NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(file, r#"{{"id": 0, "text": "Test"}}"#).unwrap();
    file.flush().unwrap();

    let path_upper = file.path().with_extension("JSONL");
    fs::copy(file.path(), &path_upper).unwrap();

    // Should detect format correctly despite uppercase
    let format = detect_format(&path_upper).unwrap();
    assert_eq!(format, FileFormat::Jsonl);

    // Should load successfully
    let docs = load_custom_corpus(&path_upper, None).unwrap();
    assert_eq!(docs.len(), 1);

    fs::remove_file(path_upper).unwrap();
}

// ============================================================================
// T28 Q22-Q28: PRODUCTION TESTS (See client_demo.rs for 500K doc tests)
// ============================================================================

#[test]
fn test_production_readiness_all_formats() {
    // Q22: Production - verify all formats work end-to-end
    let formats = vec!["test_corpus.jsonl", "test_corpus.json", "test_corpus.txt"];

    for filename in formats {
        let path = test_corpus_path(filename);
        let result = load_custom_corpus(&path, None);

        assert!(result.is_ok(), "Failed to load {}: {:?}", filename, result.err());

        let docs = result.unwrap();
        assert_eq!(docs.len(), 10, "Incorrect document count for {}", filename);
    }
}

#[test]
fn test_production_error_messages_helpful() {
    // Q23: Production - error messages guide user to solution
    let test_cases = vec![
        ("nonexistent.jsonl", "File not found"),
        ("test_empty.txt", "Empty file"),
        ("test.csv", "Unknown file format"),
    ];

    for (filename, expected_msg) in test_cases {
        let path = if filename == "test.csv" {
            PathBuf::from(filename)
        } else {
            test_corpus_path(filename)
        };

        let result = if filename.ends_with(".csv") {
            detect_format(&path).map(|_| vec![]).map_err(|e| e)
        } else {
            load_custom_corpus(&path, None)
        };

        assert!(result.is_err(), "Expected error for {}", filename);

        let error_msg = format!("{}", result.unwrap_err());
        assert!(
            error_msg.to_lowercase().contains(&expected_msg.to_lowercase()),
            "Error message for {} should mention '{}', got: {}",
            filename,
            expected_msg,
            error_msg
        );
    }
}

#[test]
fn test_production_performance_acceptable() {
    // Q24: Production - loading 10 docs should be <100ms
    use std::time::Instant;

    let path = test_corpus_path("test_corpus.jsonl");

    let start = Instant::now();
    let docs = load_custom_corpus(&path, None).unwrap();
    let elapsed = start.elapsed();

    assert_eq!(docs.len(), 10);
    assert!(
        elapsed.as_millis() < 100,
        "Loading 10 docs took {}ms (expected <100ms)",
        elapsed.as_millis()
    );
}

#[test]
fn test_production_memory_efficiency() {
    // Q25: Production - verify reasonable memory usage
    let path = test_corpus_path("test_corpus.jsonl");
    let docs = load_custom_corpus(&path, None).unwrap();

    // Each document should use reasonable memory (~100-500 bytes)
    let total_text_bytes: usize = docs.iter().map(|d| d.text.len()).sum();
    let avg_bytes_per_doc = total_text_bytes / docs.len();

    assert!(
        avg_bytes_per_doc >= 50 && avg_bytes_per_doc <= 1000,
        "Average document size {} bytes seems unreasonable",
        avg_bytes_per_doc
    );
}

#[test]
fn test_production_data_integrity() {
    // Q26: Production - verify no data corruption
    let path = test_corpus_path("test_corpus.jsonl");
    let docs = load_custom_corpus(&path, None).unwrap();

    // Verify all documents:
    // 1. Have valid IDs
    // 2. Have non-empty text
    // 3. Text doesn't have corruption markers (null bytes, etc.)

    for doc in &docs {
        assert!(doc.id < 1000, "Document ID {} seems invalid", doc.id);
        assert!(!doc.text.is_empty(), "Document {} has empty text", doc.id);
        assert!(!doc.text.contains('\0'), "Document {} has null bytes", doc.id);
    }
}

#[test]
fn test_production_concurrent_loads() {
    // Q27: Production - verify thread safety (multiple simultaneous loads)
    use std::thread;

    let path = test_corpus_path("test_corpus.jsonl");

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let p = path.clone();
            thread::spawn(move || load_custom_corpus(&p, None).unwrap())
        })
        .collect();

    for handle in handles {
        let docs = handle.join().unwrap();
        assert_eq!(docs.len(), 10);
    }
}

#[test]
fn test_production_graceful_degradation() {
    // Q28: Production - system continues despite some errors
    // Test that one bad file doesn't crash the entire system

    let good_path = test_corpus_path("test_corpus.jsonl");
    let bad_path = test_corpus_path("test_invalid.jsonl");

    // Load good file
    let good_result = load_custom_corpus(&good_path, None);
    assert!(good_result.is_ok());

    // Load bad file (should error gracefully)
    let bad_result = load_custom_corpus(&bad_path, None);
    assert!(bad_result.is_err());

    // Can still load good file after bad file error
    let good_again = load_custom_corpus(&good_path, None);
    assert!(good_again.is_ok());
    assert_eq!(good_again.unwrap().len(), 10);
}
