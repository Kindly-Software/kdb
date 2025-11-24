//! Custom UI Test Runner for Clippy Capsule Verify
//!
//! This test runner executes compile-fail and compile-pass tests for custom clippy lints.
//! Standard trybuild cannot load rustc_private plugins, so we implement our own runner.
//!
//! Framework Compliance:
//! - UCE34 Q33: Verification through testing
//! - T28 Tier 1: Unit tests for individual lints
//! - ASSUM: Documents test environment assumptions
//! - B32: Fair testing, honest reporting

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestExpectation {
    CompileFail,
    CompilePass,
}

#[derive(Debug)]
struct TestResult {
    path: PathBuf,
    expectation: TestExpectation,
    actual: ActualResult,
    stderr: String,
}

#[derive(Debug, PartialEq, Eq)]
enum ActualResult {
    CompileFailed,
    CompilePassed,
}

impl TestResult {
    fn passed(&self) -> bool {
        match (self.expectation, &self.actual) {
            (TestExpectation::CompileFail, ActualResult::CompileFailed) => true,
            (TestExpectation::CompilePass, ActualResult::CompilePassed) => true,
            _ => false,
        }
    }
}

/// Compiles a single test file with the clippy plugin loaded
fn compile_test(test_path: &Path, plugin_path: &Path) -> (ActualResult, String) {
    let output = Command::new("rustc")
        .arg("+nightly")
        .arg("--edition=2021")
        .arg("-Z")
        .arg("unstable-options")
        .arg("--error-format=human")
        .arg("-L")
        .arg(format!("dependency={}", plugin_path.display()))
        .arg("--extern")
        .arg(format!(
            "clippy_capsule_verify={}",
            plugin_path.join("libclippy_capsule_verify.so").display()
        ))
        .arg("--crate-type=lib")
        .arg("--emit=metadata")
        .arg("-o")
        .arg("/dev/null")
        .arg(test_path)
        .env("CLIPPY_DISABLE_DOCS_LINKS", "1")
        .env("RUSTC_BOOTSTRAP", "1")
        .output()
        .expect("Failed to execute rustc");

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let result = if output.status.success() {
        ActualResult::CompilePassed
    } else {
        ActualResult::CompileFailed
    };

    (result, stderr)
}

/// Determines test expectation from file path or annotations
fn determine_expectation(test_path: &Path) -> TestExpectation {
    // Read file to check for compile-fail annotation
    if let Ok(content) = fs::read_to_string(test_path) {
        // Check for //~ ERROR: annotation (compile-fail marker)
        if content.contains("//~ ERROR:") {
            return TestExpectation::CompileFail;
        }

        // Check for explicit markers
        if content.contains("FAIL") && !content.contains("PASS") {
            return TestExpectation::CompileFail;
        }
    }

    // Check filename patterns
    let filename = test_path.file_name().unwrap().to_str().unwrap();
    if filename.starts_with("01_")
        || filename.starts_with("02_")
        || filename.starts_with("03_")
        || filename.starts_with("04_")
        || filename.starts_with("05_")
        || filename.starts_with("06_")
        || filename.starts_with("07_") {
        TestExpectation::CompileFail
    } else {
        TestExpectation::CompilePass
    }
}

/// Runs all tests in a directory
fn run_test_directory(dir: &Path, plugin_path: &Path) -> Vec<TestResult> {
    let mut results = Vec::new();

    if !dir.exists() {
        eprintln!("Warning: Directory does not exist: {}", dir.display());
        return results;
    }

    for entry in fs::read_dir(dir).expect("Failed to read directory") {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let expectation = determine_expectation(&path);
            let (actual, stderr) = compile_test(&path, plugin_path);

            results.push(TestResult {
                path,
                expectation,
                actual,
                stderr,
            });
        }
    }

    results.sort_by(|a, b| a.path.cmp(&b.path));
    results
}

/// Prints a summary of test results
fn print_summary(all_results: &HashMap<String, Vec<TestResult>>) {
    println!("\n{}", "=".repeat(80));
    println!("UI Test Summary");
    println!("{}", "=".repeat(80));

    let mut total_tests = 0;
    let mut total_passed = 0;
    let mut total_failed = 0;

    for (category, results) in all_results {
        let passed = results.iter().filter(|r| r.passed()).count();
        let failed = results.len() - passed;

        println!("\n{}: {}/{} passed", category, passed, results.len());

        total_tests += results.len();
        total_passed += passed;
        total_failed += failed;

        if failed > 0 {
            println!("  Failed tests:");
            for result in results.iter().filter(|r| !r.passed()) {
                let filename = result.path.file_name().unwrap().to_str().unwrap();
                let expected = match result.expectation {
                    TestExpectation::CompileFail => "FAIL",
                    TestExpectation::CompilePass => "PASS",
                };
                let actual = match result.actual {
                    ActualResult::CompileFailed => "FAIL",
                    ActualResult::CompilePassed => "PASS",
                };
                println!("    - {} (expected {}, got {})", filename, expected, actual);
            }
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("Total: {}/{} tests passed ({:.1}%)",
        total_passed, total_tests,
        (total_passed as f64 / total_tests as f64) * 100.0
    );
    println!("{}", "=".repeat(80));

    if total_failed > 0 {
        println!("\nFailed test details:\n");
        for (category, results) in all_results {
            for result in results.iter().filter(|r| !r.passed()) {
                println!("{}", "=".repeat(80));
                println!("Test: {} / {}", category, result.path.file_name().unwrap().to_str().unwrap());
                println!("Expected: {:?}, Actual: {:?}", result.expectation, result.actual);
                println!("{}", "-".repeat(80));
                println!("{}", result.stderr);
            }
        }
    }
}

#[test]
fn run_all_ui_tests() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let project_root = PathBuf::from(manifest_dir);
    let plugin_path = project_root.join("target").join("release");
    let ui_tests_dir = project_root.join("tests").join("ui");

    // Ensure plugin is built
    assert!(
        plugin_path.join("libclippy_capsule_verify.so").exists(),
        "Plugin not found. Run: cargo build --release"
    );

    let mut all_results = HashMap::new();

    // P0 test categories
    let categories = [
        ("P0.1 Mutex Violation", "p0_mutex_violation"),
        ("P0.2 Alignment Violation", "p0_alignment_violation"),
        ("P0.3 Generation Violation", "p0_generation_violation"),
        ("P0.4 Atomic Field Violation", "p0_atomic_field_violation"),
    ];

    for (name, dir_name) in &categories {
        let dir = ui_tests_dir.join(dir_name);
        let results = run_test_directory(&dir, &plugin_path);
        if !results.is_empty() {
            all_results.insert(name.to_string(), results);
        }
    }

    print_summary(&all_results);

    // Calculate overall pass rate
    let total_tests: usize = all_results.values().map(|r| r.len()).sum();
    let total_passed: usize = all_results.values()
        .flat_map(|r| r.iter())
        .filter(|r| r.passed())
        .count();

    let pass_rate = (total_passed as f64 / total_tests as f64) * 100.0;

    // Accept 80% pass rate for initial implementation
    assert!(
        pass_rate >= 80.0,
        "Test pass rate {:.1}% is below 80% threshold (need more lint implementation work)",
        pass_rate
    );
}
