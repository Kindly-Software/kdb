//! GPU Fallback Integration Tests
//!
//! T28 Tier 4: Production-grade GPU fallback validation
//!
//! Tests GPU timeout → CPU fallback behavior for HybridDedupPipeline.
//! Verifies graceful degradation when GPU unavailable or timeout occurs.
//!
//! Framework Compliance:
//! - T28: Production tests (tier 4 - system behavior under failure)
//! - ASSUM: GPU availability assumptions documented (#ASSUME → #VERIFY)
//! - UCE34: T7 Heterogeneous tier validation (CPU+GPU coordination)

#![cfg(feature = "gpu-hybrid")]

use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
use atomic_capsule::CpuCapabilityCapsule;

#[test]
fn test_gpu_auto_fallback() {
    // #ASSUME: GPU may or may not be available
    // #VERIFY: Pipeline works regardless of GPU availability

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = HybridDedupPipeline::new(10_000, PipelineMode::Auto, &cpu_caps)
        .expect("Failed to create HybridDedupPipeline");

    // Add test documents
    let test_docs = vec![
        (0, "The quick brown fox jumps over the lazy dog"),
        (1, "A fast auburn canine leaps above an idle hound"),
        (2, "The quick brown fox jumps over the lazy dog"), // Duplicate of 0
        (3, "Completely different document about quantum physics"),
    ];

    for (id, text) in test_docs.iter() {
        pipeline.add_document(*id, text)
            .expect("Failed to add document");
    }

    // Verify pipeline is operational (GPU or CPU)
    let duplicates = pipeline.find_duplicates(0.85)
        .expect("Failed to find duplicates");

    // Should detect duplicates regardless of GPU availability
    assert!(!duplicates.is_empty(), "Should detect at least one duplicate cluster");

    // Log GPU status (informational only, not asserted)
    let using_gpu = pipeline.is_using_gpu();
    println!("Pipeline using GPU: {}", using_gpu);
}

#[test]
fn test_gpu_force_cpu_mode() {
    // #ASSUME: CPU mode always available as fallback
    // #VERIFY: PipelineMode::Cpu forces CPU execution

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = HybridDedupPipeline::new(10_000, PipelineMode::Cpu, &cpu_caps)
        .expect("Failed to create HybridDedupPipeline in CPU mode");

    // Verify CPU mode enforced
    assert!(!pipeline.is_using_gpu(), "PipelineMode::Cpu should not use GPU");

    // Add documents and verify functionality
    pipeline.add_document(0, "Test document one")
        .expect("Failed to add document");
    pipeline.add_document(1, "Test document two")
        .expect("Failed to add document");

    let duplicates = pipeline.find_duplicates(0.85)
        .expect("Failed to find duplicates in CPU mode");

    // Should work in CPU mode
    assert!(duplicates.is_empty(), "No duplicates expected for different documents");
}

#[test]
#[cfg_attr(not(feature = "gpu"), ignore = "GPU feature not enabled")]
fn test_gpu_graceful_degradation() {
    // #ASSUME: GPU may fail to initialize (driver issues, VRAM exhaustion, etc.)
    // #VERIFY: Pipeline falls back to CPU without panicking

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Try to create pipeline with Auto mode
    let result = HybridDedupPipeline::new(10_000, PipelineMode::Auto, &cpu_caps);

    // Pipeline creation should always succeed (GPU or CPU fallback)
    assert!(result.is_ok(), "Pipeline creation should not fail even if GPU unavailable");

    let mut pipeline = result.unwrap();

    // Process documents (may use GPU or CPU)
    for i in 0..100 {
        pipeline.add_document(i, &format!("Document {}", i))
            .expect("Failed to add document during graceful degradation test");
    }

    // Verify pipeline is functional
    let duplicates = pipeline.find_duplicates(0.85)
        .expect("Failed to find duplicates during graceful degradation");

    println!("Graceful degradation test completed, using GPU: {}", pipeline.is_using_gpu());
    assert!(duplicates.is_empty(), "No duplicates expected for unique documents");
}
