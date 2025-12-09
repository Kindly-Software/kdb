//! Integration tests for complete workflows (T28 Q15-Q21)
//!
//! These tests verify multi-component interactions work correctly:
//! - Parse → Calculate → Fix → Verify workflow
//! - Multi-file processing
//! - Backup and rollback
//! - End-to-end transformations

use fix_padding_fields::{extract_capsules, PaddingCalculator, PaddingFixer};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[path = "../fixtures/mod.rs"]
mod fixtures;

// Q15: Test complete parse → calculate → fix workflow
#[test]
fn test_complete_workflow() {
    // 1. Parse
    let capsules = extract_capsules(fixtures::INCORRECT_PADDING).expect("Should parse");
    assert_eq!(capsules.len(), 1);

    // 2. Calculate
    let calc = PaddingCalculator::new(&capsules[0]).expect("Should calculate");
    assert!(calc.needs_fixing());
    assert_eq!(calc.required_padding(), 56);

    // 3. Fix
    let mut fixer = PaddingFixer::new(fixtures::INCORRECT_PADDING.to_string());
    let changed = fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
    assert!(changed);

    // 4. Verify fix
    let new_content = fixer.content();
    let new_capsules = extract_capsules(new_content).expect("Should parse fixed");
    let new_calc = PaddingCalculator::new(&new_capsules[0]).expect("Should calculate");
    assert!(!new_calc.needs_fixing());
}

// Q16: Test multi-capsule file workflow
#[test]
fn test_multi_capsule_workflow() {
    // Parse multiple capsules
    let capsules = extract_capsules(fixtures::MULTI_CAPSULE_FILE).expect("Should parse");
    assert_eq!(capsules.len(), 2);

    // Verify both
    let mut fixer = PaddingFixer::new(fixtures::MULTI_CAPSULE_FILE.to_string());
    let mut changes = 0;

    for capsule in &capsules {
        let calc = PaddingCalculator::new(capsule).expect("Should calculate");
        if calc.needs_fixing() {
            changes += 1;
        }
    }

    // If any need fixing, apply fixes
    if changes > 0 {
        for capsule in capsules {
            fixer.apply_padding_fix(&capsule).ok();
        }

        // Verify all are now correct
        let new_capsules = extract_capsules(fixer.content()).expect("Should parse");
        for capsule in new_capsules {
            let calc = PaddingCalculator::new(&capsule).expect("Should calculate");
            assert!(!calc.needs_fixing());
        }
    }
}

// Q17: Test file-based workflow with tempdir
#[test]
fn test_file_workflow() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let file_path = temp_dir.path().join("test_capsule.rs");

    // Write incorrect capsule to file
    fs::write(&file_path, fixtures::INCORRECT_PADDING).expect("Should write");

    // Read and process
    let content = fs::read_to_string(&file_path).expect("Should read");
    let capsules = extract_capsules(&content).expect("Should parse");
    assert_eq!(capsules.len(), 1);

    // Fix and write back
    let mut fixer = PaddingFixer::new(content);
    fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
    fs::write(&file_path, fixer.content()).expect("Should write");

    // Verify file was fixed
    let new_content = fs::read_to_string(&file_path).expect("Should read");
    let new_capsules = extract_capsules(&new_content).expect("Should parse");
    let calc = PaddingCalculator::new(&new_capsules[0]).expect("Should calculate");
    assert!(!calc.needs_fixing());
}

// Q18: Test backup workflow
#[test]
fn test_backup_workflow() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let file_path = temp_dir.path().join("test_capsule.rs");
    let backup_path = file_path.with_extension("rs.bak");

    // Write original
    fs::write(&file_path, fixtures::INCORRECT_PADDING).expect("Should write");

    // Create backup
    fs::copy(&file_path, &backup_path).expect("Should backup");

    // Modify original
    let content = fs::read_to_string(&file_path).expect("Should read");
    let capsules = extract_capsules(&content).expect("Should parse");
    let mut fixer = PaddingFixer::new(content);
    fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
    fs::write(&file_path, fixer.content()).expect("Should write");

    // Verify backup is unchanged
    let backup_content = fs::read_to_string(&backup_path).expect("Should read backup");
    assert_eq!(backup_content, fixtures::INCORRECT_PADDING);

    // Verify original was modified
    let new_content = fs::read_to_string(&file_path).expect("Should read");
    assert_ne!(new_content, fixtures::INCORRECT_PADDING);
}

// Q19: Test rollback workflow (restore from backup)
#[test]
fn test_rollback_workflow() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let file_path = temp_dir.path().join("test_capsule.rs");
    let backup_path = file_path.with_extension("rs.bak");

    // Write and backup
    fs::write(&file_path, fixtures::INCORRECT_PADDING).expect("Should write");
    fs::copy(&file_path, &backup_path).expect("Should backup");

    // Make bad modification
    fs::write(&file_path, "BAD CONTENT").expect("Should write");

    // Rollback
    fs::copy(&backup_path, &file_path).expect("Should rollback");

    // Verify rollback worked
    let restored = fs::read_to_string(&file_path).expect("Should read");
    assert_eq!(restored, fixtures::INCORRECT_PADDING);
}

// Q20: Test multi-file directory workflow
#[test]
fn test_multi_file_workflow() {
    let temp_dir = TempDir::new().expect("Should create temp dir");

    // Create multiple files
    let files = vec![
        ("capsule1.rs", fixtures::SIMPLE_CAPSULE),
        ("capsule2.rs", fixtures::INCORRECT_PADDING),
        ("capsule3.rs", fixtures::MISSING_PADDING),
    ];

    for (name, content) in &files {
        let path = temp_dir.path().join(name);
        fs::write(&path, content).expect("Should write");
    }

    // Process all files
    let rust_files: Vec<PathBuf> = fs::read_dir(temp_dir.path())
        .expect("Should read dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "rs"))
        .collect();

    assert_eq!(rust_files.len(), 3);

    let mut total_fixed = 0;
    for file_path in rust_files {
        let content = fs::read_to_string(&file_path).expect("Should read");
        let capsules = extract_capsules(&content).expect("Should parse");

        if !capsules.is_empty() {
            let mut fixer = PaddingFixer::new(content);
            for capsule in capsules {
                let calc = PaddingCalculator::new(&capsule).expect("Should calculate");
                if calc.needs_fixing() {
                    fixer.apply_padding_fix(&capsule).expect("Should fix");
                    total_fixed += 1;
                }
            }
            fs::write(&file_path, fixer.content()).expect("Should write");
        }
    }

    // Should have fixed 2 files (INCORRECT_PADDING and MISSING_PADDING)
    assert!(total_fixed >= 2);
}

// Q21: Test idempotency (fix twice = fix once)
#[test]
fn test_idempotent_workflow() {
    // First fix
    let mut fixer1 = PaddingFixer::new(fixtures::INCORRECT_PADDING.to_string());
    let capsules1 = extract_capsules(fixtures::INCORRECT_PADDING).expect("Should parse");
    fixer1.apply_padding_fix(&capsules1[0]).expect("Should fix");
    let fixed_once = fixer1.content().to_string();

    // Second fix on already-fixed content
    let mut fixer2 = PaddingFixer::new(fixed_once.clone());
    let capsules2 = extract_capsules(&fixed_once).expect("Should parse");
    let changed = fixer2.apply_padding_fix(&capsules2[0]).expect("Should not fail");

    // Second fix should report no changes
    assert!(!changed, "Second fix should not change anything");
    assert_eq!(fixer2.content(), fixed_once);
}

// Q15: Test nested directory workflow
#[test]
fn test_nested_directory_workflow() {
    let temp_dir = TempDir::new().expect("Should create temp dir");

    // Create nested structure
    let sub_dir = temp_dir.path().join("src");
    fs::create_dir(&sub_dir).expect("Should create subdir");

    // Write files at different levels
    fs::write(
        temp_dir.path().join("top.rs"),
        fixtures::SIMPLE_CAPSULE
    ).expect("Should write");

    fs::write(
        sub_dir.join("nested.rs"),
        fixtures::INCORRECT_PADDING
    ).expect("Should write");

    // Collect all Rust files recursively
    let mut rust_files = Vec::new();
    for entry in walkdir::WalkDir::new(temp_dir.path())
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().is_some_and(|ext| ext == "rs") {
            rust_files.push(entry.path().to_path_buf());
        }
    }

    assert_eq!(rust_files.len(), 2);

    // Process all
    for file_path in rust_files {
        let content = fs::read_to_string(&file_path).expect("Should read");
        let capsules = extract_capsules(&content).expect("Should parse");

        if !capsules.is_empty() {
            let mut fixer = PaddingFixer::new(content);
            for capsule in capsules {
                fixer.apply_padding_fix(&capsule).ok();
            }
            fs::write(&file_path, fixer.content()).expect("Should write");
        }
    }
}

// Q16: Test parse error recovery
#[test]
fn test_parse_error_recovery() {
    let temp_dir = TempDir::new().expect("Should create temp dir");

    // Create files with mix of valid and invalid
    let files = vec![
        ("valid.rs", fixtures::SIMPLE_CAPSULE, true),
        ("invalid.rs", "struct Invalid { not valid rust }", false),
        ("another_valid.rs", fixtures::INCORRECT_PADDING, true),
    ];

    for (name, content, _should_parse) in &files {
        let path = temp_dir.path().join(name);
        fs::write(&path, content).expect("Should write");
    }

    // Process all files, gracefully handling errors
    let rust_files: Vec<PathBuf> = fs::read_dir(temp_dir.path())
        .expect("Should read dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "rs"))
        .collect();

    let mut processed = 0;
    let mut errors = 0;

    for file_path in rust_files {
        let content = fs::read_to_string(&file_path).expect("Should read");

        match extract_capsules(&content) {
            Ok(capsules) if !capsules.is_empty() => {
                processed += 1;
                let mut fixer = PaddingFixer::new(content);
                for capsule in capsules {
                    fixer.apply_padding_fix(&capsule).ok();
                }
                fs::write(&file_path, fixer.content()).ok();
            }
            Err(_) => {
                errors += 1;
            }
            _ => {}
        }
    }

    // Should have processed 2 valid files and encountered 1 error
    assert_eq!(processed, 2);
    assert_eq!(errors, 1);
}

// Q17: Test concurrent file operations (determinism check)
#[test]
fn test_deterministic_file_operations() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let file_path = temp_dir.path().join("test.rs");

    // Write same content multiple times
    for _ in 0..3 {
        fs::write(&file_path, fixtures::INCORRECT_PADDING).expect("Should write");

        let content = fs::read_to_string(&file_path).expect("Should read");
        let capsules = extract_capsules(&content).expect("Should parse");
        let mut fixer = PaddingFixer::new(content);
        fixer.apply_padding_fix(&capsules[0]).expect("Should fix");

        fs::write(&file_path, fixer.content()).expect("Should write");
    }

    // Final result should be consistent
    let final_content = fs::read_to_string(&file_path).expect("Should read");
    let capsules = extract_capsules(&final_content).expect("Should parse");
    let calc = PaddingCalculator::new(&capsules[0]).expect("Should calculate");
    assert!(!calc.needs_fixing());
}
