//! Minimal test to reproduce segfault

use atomic_capsule::CpuCapabilityCapsule;
use kindly_dedup::DedupPipeline;

#[test]
fn test_minimal_add_document() {
    eprintln!("Creating CPU caps...");
    let cpu_caps = CpuCapabilityCapsule::detect();

    eprintln!("Creating pipeline with capacity 100...");
    let mut pipeline = DedupPipeline::new(100, &cpu_caps);

    eprintln!("Adding document 0...");
    let result = pipeline.add_document(0, "This is a test document with some text");

    eprintln!("Result: {:?}", result);
    assert!(result.is_ok());

    eprintln!("Test passed!");
}
