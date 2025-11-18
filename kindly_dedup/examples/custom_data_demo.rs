//! Custom Data Loading Demo
//!
//! Demonstrates the custom_data module with all supported formats.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example custom_data_demo
//! ```

use kindly_dedup::custom_data::{
    CustomDataError, load_custom_corpus, load_json, load_jsonl, load_plaintext, print_progress,
};
use std::fs::File;
use std::io::Write;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tempfile::tempdir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("═══════════════════════════════════════════════════════════");
    println!("  Custom Data Loading Demo");
    println!("═══════════════════════════════════════════════════════════\n");

    // Create temporary directory
    let temp_dir = tempdir()?;

    // Demo 1: JSONL format (recommended)
    demo_jsonl(&temp_dir)?;

    // Demo 2: JSON array format
    demo_json(&temp_dir)?;

    // Demo 3: Plain text format
    demo_plaintext(&temp_dir)?;

    // Demo 4: Auto-detect format
    demo_auto_detect(&temp_dir)?;

    // Demo 5: Progress tracking
    demo_progress(&temp_dir)?;

    // Demo 6: Error handling
    demo_error_handling()?;

    println!("\n═══════════════════════════════════════════════════════════");
    println!("  All Demos Complete!");
    println!("═══════════════════════════════════════════════════════════");

    Ok(())
}

// ============================================================================
// Demo 1: JSONL Format (Recommended)
// ============================================================================

fn demo_jsonl(temp_dir: &tempfile::TempDir) -> Result<(), Box<dyn std::error::Error>> {
    println!("Demo 1: JSONL Format (JSON Lines)");
    println!("─────────────────────────────────────");

    // Create JSONL file
    let jsonl_path = temp_dir.path().join("corpus.jsonl");
    let mut file = File::create(&jsonl_path)?;
    writeln!(file, r#"{{"id": 1, "text": "Machine learning is transforming AI"}}"#)?;
    writeln!(file, r#"{{"id": 2, "text": "Neural networks power deep learning", "url": "http://example.com"}}"#)?;
    writeln!(file, r#"{{"id": 3, "text": "Data analysis drives insights"}}"#)?;
    file.flush()?;

    // Load JSONL
    let documents = load_jsonl(&jsonl_path, None)?;

    println!("Loaded {} documents from JSONL:", documents.len());
    for doc in &documents {
        println!("  [{}] {}", doc.id, &doc.text[..50.min(doc.text.len())]);
        if let Some(url) = &doc.url {
            println!("      URL: {}", url);
        }
    }

    println!("✓ JSONL loading successful\n");

    Ok(())
}

// ============================================================================
// Demo 2: JSON Array Format
// ============================================================================

fn demo_json(temp_dir: &tempfile::TempDir) -> Result<(), Box<dyn std::error::Error>> {
    println!("Demo 2: JSON Array Format");
    println!("─────────────────────────────────────");

    // Create JSON file
    let json_path = temp_dir.path().join("corpus.json");
    let mut file = File::create(&json_path)?;
    writeln!(
        file,
        r#"[
  {{"id": 1, "text": "Artificial intelligence advances"}},
  {{"id": 2, "text": "Computational efficiency matters"}},
  {{"id": 3, "text": "Optimization techniques improve performance"}}
]"#
    )?;
    file.flush()?;

    // Load JSON
    let documents = load_json(&json_path, None)?;

    println!("Loaded {} documents from JSON array:", documents.len());
    for doc in &documents {
        println!("  [{}] {}", doc.id, &doc.text[..50.min(doc.text.len())]);
    }

    println!("✓ JSON array loading successful\n");

    Ok(())
}

// ============================================================================
// Demo 3: Plain Text Format
// ============================================================================

fn demo_plaintext(temp_dir: &tempfile::TempDir) -> Result<(), Box<dyn std::error::Error>> {
    println!("Demo 3: Plain Text Format");
    println!("─────────────────────────────────────");

    // Create plain text file
    let txt_path = temp_dir.path().join("corpus.txt");
    let mut file = File::create(&txt_path)?;
    writeln!(file, "Distributed systems enable scalability")?;
    writeln!(file, "Concurrent programming requires care")?;
    writeln!(file, "")?; // Empty line (will be skipped)
    writeln!(file, "Lockfree algorithms avoid contention")?;
    file.flush()?;

    // Load plain text
    let documents = load_plaintext(&txt_path, None)?;

    println!("Loaded {} documents from plain text:", documents.len());
    for doc in &documents {
        println!("  [{}] {}", doc.id, &doc.text[..50.min(doc.text.len())]);
    }

    println!("✓ Plain text loading successful (empty lines skipped)\n");

    Ok(())
}

// ============================================================================
// Demo 4: Auto-Detect Format
// ============================================================================

fn demo_auto_detect(temp_dir: &tempfile::TempDir) -> Result<(), Box<dyn std::error::Error>> {
    println!("Demo 4: Auto-Detect Format");
    println!("─────────────────────────────────────");

    // Create files with different formats
    let jsonl_path = temp_dir.path().join("auto.jsonl");
    let mut file = File::create(&jsonl_path)?;
    writeln!(file, r#"{{"id": 100, "text": "Auto-detected JSONL"}}"#)?;
    file.flush()?;

    // Load with auto-detection
    let documents = load_custom_corpus(&jsonl_path, None)?;

    println!("Auto-detected format: JSONL");
    println!("Loaded {} documents:", documents.len());
    for doc in &documents {
        println!("  [{}] {}", doc.id, doc.text);
    }

    println!("✓ Auto-detection successful\n");

    Ok(())
}

// ============================================================================
// Demo 5: Progress Tracking
// ============================================================================

fn demo_progress(temp_dir: &tempfile::TempDir) -> Result<(), Box<dyn std::error::Error>> {
    println!("Demo 5: Progress Tracking (Lockfree Atomic)");
    println!("─────────────────────────────────────");

    // Create JSONL file with more documents
    let jsonl_path = temp_dir.path().join("progress.jsonl");
    let mut file = File::create(&jsonl_path)?;
    for i in 1..=100 {
        writeln!(
            file,
            r#"{{"id": {}, "text": "Document number {}"}}"#,
            i, i
        )?;
    }
    file.flush()?;

    // Create progress tracker (lockfree atomic)
    let progress = Arc::new(AtomicU64::new(0));

    println!("Loading 100 documents with progress tracking...\n");

    // Load with progress tracking
    let documents = load_jsonl(&jsonl_path, Some(progress.clone()))?;

    // Print final progress
    print_progress(&progress, documents.len(), "Loaded");

    println!("\n✓ Progress tracking successful (lockfree atomic)\n");

    Ok(())
}

// ============================================================================
// Demo 6: Error Handling
// ============================================================================

fn demo_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    println!("Demo 6: Error Handling (Friendly Messages)");
    println!("─────────────────────────────────────");

    // Test 1: File not found
    println!("Test 1: File not found");
    match load_custom_corpus("nonexistent.jsonl", None) {
        Err(CustomDataError::FileNotFound(path)) => {
            println!("  ✓ Caught FileNotFound error: {}", path);
        }
        _ => println!("  ✗ Unexpected result"),
    }

    // Test 2: Unknown format
    println!("\nTest 2: Unknown format");
    match load_custom_corpus("corpus.csv", None) {
        Err(CustomDataError::UnknownFormat(path)) => {
            println!("  ✓ Caught UnknownFormat error: {}", path);
        }
        _ => println!("  ✗ Unexpected result"),
    }

    println!("\n✓ Error handling working correctly\n");

    Ok(())
}
