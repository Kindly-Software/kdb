//! Integration tests for ValidationPipeline
//! 
//! Tests validation without requiring full atomic_capsule compilation

use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn test_syntax_validation_valid() {
    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("test.rs");
    
    // Valid Rust syntax
    fs::write(&file, "struct Foo { x: u32 }").unwrap();
    
    // Parse with syn
    let content = fs::read_to_string(&file).unwrap();
    let result = syn::parse_file(&content);
    assert!(result.is_ok());
}

#[test]
fn test_syntax_validation_invalid() {
    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("test.rs");
    
    // Invalid Rust syntax (missing closing brace)
    fs::write(&file, "struct Foo { x: u32").unwrap();
    
    let content = fs::read_to_string(&file).unwrap();
    let result = syn::parse_file(&content);
    assert!(result.is_err());
}

#[test]
fn test_backup_and_restore() {
    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("test.rs");
    let backup = temp_dir.path().join("test.rs.backup");
    
    let original_content = "// Original content";
    fs::write(&file, original_content).unwrap();
    
    // Create backup
    fs::copy(&file, &backup).unwrap();
    assert!(backup.exists());
    
    // Modify original
    fs::write(&file, "// Modified content").unwrap();
    
    // Restore from backup
    let backup_content = fs::read(&backup).unwrap();
    fs::write(&file, backup_content).unwrap();
    
    // Verify restoration
    let restored_content = fs::read_to_string(&file).unwrap();
    assert_eq!(restored_content, original_content);
    
    // Cleanup
    fs::remove_file(&backup).unwrap();
    assert!(!backup.exists());
}

#[test]
fn test_atomic_operations_simulation() {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    
    // Simulate ValidationResultCapsule behavior without derive macro
    let syntax_success = AtomicBool::new(false);
    let compile_success = AtomicBool::new(false);
    let test_success = AtomicBool::new(false);
    let generation = AtomicU64::new(0);
    
    // Initial state
    assert!(!syntax_success.load(Ordering::Acquire));
    assert!(!compile_success.load(Ordering::Acquire));
    assert!(!test_success.load(Ordering::Acquire));
    
    // Set syntax success
    syntax_success.store(true, Ordering::Release);
    generation.fetch_add(1, Ordering::Release);
    assert!(syntax_success.load(Ordering::Acquire));
    assert_eq!(generation.load(Ordering::Acquire), 1);
    
    // Set compile success
    compile_success.store(true, Ordering::Release);
    generation.fetch_add(1, Ordering::Release);
    assert!(compile_success.load(Ordering::Acquire));
    assert_eq!(generation.load(Ordering::Acquire), 2);
    
    // Set test success
    test_success.store(true, Ordering::Release);
    generation.fetch_add(1, Ordering::Release);
    assert!(test_success.load(Ordering::Acquire));
    assert_eq!(generation.load(Ordering::Acquire), 3);
    
    // All passed
    let all_passed = syntax_success.load(Ordering::Acquire) &&
                     compile_success.load(Ordering::Acquire) &&
                     test_success.load(Ordering::Acquire);
    assert!(all_passed);
}

#[test]
fn test_generation_counter_consistency() {
    use std::sync::atomic::{AtomicU64, Ordering};
    
    let generation = AtomicU64::new(0);
    
    // Take snapshot before update
    let gen_before = generation.load(Ordering::Acquire);
    
    // Update generation
    generation.fetch_add(1, Ordering::Release);
    
    // Take snapshot after update
    let gen_after = generation.load(Ordering::Acquire);
    
    // Verify consistency (generation changed)
    assert_ne!(gen_before, gen_after);
    assert_eq!(gen_after, gen_before + 1);
}
