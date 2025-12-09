// Test adaptive selector basic functionality
use kindly_dedup::adaptive::{PipelineSelectorCapsule, PipelineSelection, RamDetectorCapsule};

fn main() {
    println!("=== Adaptive Pipeline Selector Test ===\n");

    // Test 1: RAM Detection
    println!("Test 1: RAM Detection");
    match RamDetectorCapsule::available_ram_gb() {
        Ok(ram_gb) => {
            println!("  ✓ Detected RAM: {:.2} GB", ram_gb);
            assert!(ram_gb > 0.0, "RAM must be positive");
            assert!(ram_gb < 1000.0, "RAM unrealistic");
        }
        Err(e) => {
            println!("  ⚠ RAM detection failed: {} (using fallback 16 GB)", e);
        }
    }

    // Test 2: Small corpus (should select Fast)
    println!("\nTest 2: Small corpus (100K docs)");
    let sel = PipelineSelectorCapsule::select(100_000, None, false, false);
    println!("  Selected: {}", sel.name());
    assert_eq!(sel, PipelineSelection::Fast, "Should select Fast for small corpus");

    // Test 3: Large corpus (should select Streaming)
    println!("\nTest 3: Large corpus (100M docs)");
    let sel = PipelineSelectorCapsule::select(100_000_000, None, false, false);
    println!("  Selected: {}", sel.name());
    assert_eq!(sel, PipelineSelection::Streaming, "Should select Streaming for large corpus");

    // Test 4: Medium corpus with low RAM (should select Streaming)
    println!("\nTest 4: Medium corpus (10M docs) with low RAM (8 GB)");
    let sel = PipelineSelectorCapsule::select(10_000_000, Some(8.0), false, false);
    println!("  Selected: {}", sel.name());
    assert_eq!(sel, PipelineSelection::Streaming, "Should select Streaming with low RAM");

    // Test 5: Medium corpus with high RAM (should select Fast)
    println!("\nTest 5: Medium corpus (10M docs) with high RAM (64 GB)");
    let sel = PipelineSelectorCapsule::select(10_000_000, Some(64.0), false, false);
    println!("  Selected: {}", sel.name());
    assert_eq!(sel, PipelineSelection::Fast, "Should select Fast with high RAM");

    // Test 6: Force streaming flag
    println!("\nTest 6: Force streaming (100K docs, --streaming flag)");
    let sel = PipelineSelectorCapsule::select(100_000, Some(64.0), false, true);
    println!("  Selected: {}", sel.name());
    assert_eq!(sel, PipelineSelection::Streaming, "Should select Streaming when forced");

    // Test 7: Force fast flag
    println!("\nTest 7: Force fast (100M docs, --fast flag)");
    let sel = PipelineSelectorCapsule::select(100_000_000, Some(8.0), true, false);
    println!("  Selected: {}", sel.name());
    assert_eq!(sel, PipelineSelection::Fast, "Should select Fast when forced");

    println!("\n=== All tests passed! ✓ ===");
}
