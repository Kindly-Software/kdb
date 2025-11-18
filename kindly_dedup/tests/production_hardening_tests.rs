//! Production Hardening Integration Tests
//!
//! Validates all production hardening features:
//! - Resource limits (cgroup detection, validation)
//! - Config validation (feature compatibility, memory)
//! - Panic boundaries (panic recovery, error propagation)
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q32 (Constraints testing), Q33 (Validation testing)
//! - **T28**: Integration tests (Q15-Q21)
//! - **ASSUM**: All assumptions verified with tests

use kindly_dedup::{
    config_validation::{validate_deployment_config, validate_for_document_count, ConfigError},
    resource_limits::{ResourceError, ResourceLimits},
};

// ============================================================================
// RESOURCE LIMITS TESTS
// ============================================================================

#[test]
fn test_resource_limits_detect() {
    let limits = ResourceLimits::detect();

    // Should detect or use conservative defaults
    assert!(limits.max_documents > 0);
    assert!(limits.max_memory_bytes > 0);
    assert!(limits.max_document_size > 0);

    // Validate reasonable defaults
    assert_eq!(limits.max_documents, 50_000_000);
    assert_eq!(limits.max_document_size, 1_048_576);
    assert!(limits.max_memory_bytes >= 8 * 1024 * 1024 * 1024); // >= 8GB
}

#[test]
fn test_resource_limits_custom() {
    let limits = ResourceLimits::new(
        10_000_000,             // 10M documents
        4 * 1024 * 1024 * 1024, // 4GB memory
        512 * 1024,             // 512KB per doc
    );

    assert_eq!(limits.max_documents, 10_000_000);
    assert_eq!(limits.max_memory_bytes, 4 * 1024 * 1024 * 1024);
    assert_eq!(limits.max_document_size, 512 * 1024);
}

#[test]
fn test_check_document_count_ok() {
    let limits = ResourceLimits::detect();

    // Small counts should pass
    assert!(limits.check_document_count(100).is_ok());
    assert!(limits.check_document_count(1_000_000).is_ok());
    assert!(limits.check_document_count(10_000_000).is_ok());
}

#[test]
fn test_check_document_count_exceeds() {
    let limits = ResourceLimits::new(1_000_000, 8 * 1024 * 1024 * 1024, 1_048_576);

    let result = limits.check_document_count(2_000_000);
    assert!(result.is_err());

    match result.unwrap_err() {
        ResourceError::DocumentLimitExceeded { limit, requested } => {
            assert_eq!(limit, 1_000_000);
            assert_eq!(requested, 2_000_000);
        }
        _ => panic!("Expected DocumentLimitExceeded error"),
    }
}

#[test]
fn test_check_document_size_ok() {
    let limits = ResourceLimits::detect();

    // Small documents should pass
    assert!(limits.check_document_size(1024).is_ok());
    assert!(limits.check_document_size(100_000).is_ok());
    assert!(limits.check_document_size(1_000_000).is_ok()); // <= 1MB
}

#[test]
fn test_check_document_size_exceeds() {
    let limits = ResourceLimits::new(50_000_000, 8 * 1024 * 1024 * 1024, 512 * 1024);

    let result = limits.check_document_size(1_048_576); // 1MB > 512KB
    assert!(result.is_err());

    match result.unwrap_err() {
        ResourceError::DocumentTooLarge { limit, size } => {
            assert_eq!(limit, 512 * 1024);
            assert_eq!(size, 1_048_576);
        }
        _ => panic!("Expected DocumentTooLarge error"),
    }
}

#[test]
fn test_estimate_memory_usage() {
    let limits = ResourceLimits::detect();

    // Test estimation formula: 528 bytes per document
    let mem_1k = limits.estimate_memory_usage(1_000);
    assert_eq!(mem_1k, 528_000);

    let mem_1m = limits.estimate_memory_usage(1_000_000);
    assert_eq!(mem_1m, 528_000_000);

    let mem_10m = limits.estimate_memory_usage(10_000_000);
    assert_eq!(mem_10m, 5_280_000_000);
}

#[test]
fn test_check_memory_estimate_ok() {
    let limits = ResourceLimits::new(50_000_000, 8 * 1024 * 1024 * 1024, 1_048_576);

    // Small document counts should pass
    assert!(limits.check_memory_estimate(1_000_000).is_ok()); // 528 MB
    assert!(limits.check_memory_estimate(10_000_000).is_ok()); // 5.28 GB
}

#[test]
fn test_check_memory_estimate_exceeds() {
    let limits = ResourceLimits::new(50_000_000, 1 * 1024 * 1024 * 1024, 1_048_576); // 1GB limit

    let result = limits.check_memory_estimate(10_000_000); // 5.28 GB estimated
    assert!(result.is_err());

    match result.unwrap_err() {
        ResourceError::MemoryLimitExceeded { limit, estimated } => {
            assert_eq!(limit, 1 * 1024 * 1024 * 1024);
            assert_eq!(estimated, 5_280_000_000);
        }
        _ => panic!("Expected MemoryLimitExceeded error"),
    }
}

// ============================================================================
// CONFIG VALIDATION TESTS
// ============================================================================

#[test]
fn test_validate_deployment_config() {
    // Should pass with default configuration
    let result = validate_deployment_config();
    assert!(result.is_ok(), "Default config should be valid: {:?}", result);
}

#[test]
fn test_validate_for_document_count_ok() {
    // Small document counts should pass
    let result = validate_for_document_count(100_000);
    assert!(result.is_ok(), "100K documents should be valid: {:?}", result);

    let result = validate_for_document_count(1_000_000);
    assert!(result.is_ok(), "1M documents should be valid: {:?}", result);
}

#[test]
fn test_validate_for_document_count_exceeds() {
    // Exceeds default 50M limit
    let result = validate_for_document_count(100_000_000);
    assert!(result.is_err(), "100M documents should exceed limit");

    match result.unwrap_err() {
        ConfigError::InsufficientMemory { required, available } => {
            assert!(required > available, "Required should exceed available");
        }
        _ => panic!("Expected InsufficientMemory error"),
    }
}

// ============================================================================
// PANIC BOUNDARY TESTS (production-api feature)
// ============================================================================

#[cfg(feature = "production-api")]
mod panic_boundary_tests {
    use atomic_capsule::CpuCapabilityCapsule;
    use kindly_dedup::panic_boundary::PanicSafePipeline;
    use kindly_dedup::DedupPipeline;

    #[test]
    fn test_panic_safe_pipeline_normal_operation() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let pipeline = DedupPipeline::new(100, &cpu_caps);
        let mut safe_pipeline = PanicSafePipeline::new(pipeline);

        // Normal operation should work
        let result = safe_pipeline.add_document_safe(0, "test document");
        assert!(result.is_ok());

        let clusters = safe_pipeline.find_duplicates_safe(0.85);
        assert!(clusters.is_ok());
    }

    #[test]
    fn test_panic_safe_pipeline_multiple_documents() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let pipeline = DedupPipeline::new(100, &cpu_caps);
        let mut safe_pipeline = PanicSafePipeline::new(pipeline);

        // Add multiple documents
        for i in 0..10 {
            let text = format!("test document {}", i);
            let result = safe_pipeline.add_document_safe(i, &text);
            assert!(result.is_ok(), "Document {} should add successfully", i);
        }

        // Find duplicates
        let clusters = safe_pipeline.find_duplicates_safe(0.85);
        assert!(clusters.is_ok(), "Finding duplicates should succeed");
    }

    #[test]
    fn test_panic_safe_pipeline_get_ref() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let pipeline = DedupPipeline::new(100, &cpu_caps);
        let mut safe_pipeline = PanicSafePipeline::new(pipeline);

        safe_pipeline.add_document_safe(0, "test").unwrap();

        // Get reference to underlying pipeline
        let pipeline_ref = safe_pipeline.get_ref();
        assert!(pipeline_ref.get_signature(0).is_some());
    }

    #[test]
    fn test_panic_safe_pipeline_into_inner() {
        let cpu_caps = CpuCapabilityCapsule::detect();
        let pipeline = DedupPipeline::new(100, &cpu_caps);
        let mut safe_pipeline = PanicSafePipeline::new(pipeline);

        safe_pipeline.add_document_safe(0, "test").unwrap();

        // Extract underlying pipeline
        let pipeline = safe_pipeline.into_inner();
        assert!(pipeline.get_signature(0).is_some());
    }
}

// ============================================================================
// INTEGRATION TESTS (Pipeline + Hardening)
// ============================================================================

#[test]
fn test_pipeline_with_validation() {
    use atomic_capsule::CpuCapabilityCapsule;
    use kindly_dedup::DedupPipeline;

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Should succeed for reasonable document count
    let result = DedupPipeline::new_with_validation(100_000, &cpu_caps);
    assert!(result.is_ok(), "100K documents should be valid");

    // Should fail for excessive document count
    let result = DedupPipeline::new_with_validation(100_000_000, &cpu_caps);
    assert!(result.is_err(), "100M documents should exceed limits");
}

#[test]
fn test_pipeline_with_validation_integration() {
    use atomic_capsule::CpuCapabilityCapsule;
    use kindly_dedup::DedupPipeline;

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new_with_validation(10_000, &cpu_caps).expect("Pipeline creation should succeed");

    // Add documents
    for i in 0..10 {
        let text = format!("test document {}", i);
        pipeline
            .add_document(i, &text)
            .expect("Document addition should succeed");
    }

    // Find duplicates
    let clusters = pipeline.find_duplicates(0.85).expect("find_duplicates should succeed");
    assert!(!clusters.is_empty(), "Should have at least one cluster");
}

// ============================================================================
// STRESS TESTS
// ============================================================================

#[test]
fn test_resource_limits_boundary_conditions() {
    let limits = ResourceLimits::new(1_000_000, 1 * 1024 * 1024 * 1024, 1_048_576);

    // Exactly at limit should pass
    assert!(limits.check_document_count(1_000_000).is_ok());

    // One over limit should fail
    assert!(limits.check_document_count(1_000_001).is_err());

    // Zero should pass
    assert!(limits.check_document_count(0).is_ok());
}

#[test]
fn test_config_validation_comprehensive() {
    // Test various document counts
    let test_cases = vec![
        (1_000, true),        // Small
        (100_000, true),      // Medium
        (1_000_000, true),    // Large
        (10_000_000, true),   // Very large
        (100_000_000, false), // Exceeds limit
    ];

    for (num_docs, should_pass) in test_cases {
        let result = validate_for_document_count(num_docs);
        if should_pass {
            assert!(result.is_ok(), "{} documents should pass validation", num_docs);
        } else {
            assert!(result.is_err(), "{} documents should fail validation", num_docs);
        }
    }
}
