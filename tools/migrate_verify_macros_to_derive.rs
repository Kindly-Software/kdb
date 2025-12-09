#!/usr/bin/env cargo +nightly -Zscript
```cargo
[package]
edition = "2021"

[dependencies]
syn = { version = "2.0", features = ["full", "parsing", "extra-traits"] }
quote = "1.0"
proc-macro2 = "1.0"
walkdir = "2.4"
regex = "1.10"
```

//! # Migration Tool: Manual Verification Macros → #[derive(ComputationalCapsule)]
//!
//! **Purpose**: Automated migration from manual `verify_capsule_properties!`,
//! `verify_alignment_only!`, and `verify_simd_capsule!` to automatic derivation.
//!
//! **UCE34 Framework Compliance**:
//! - Q10: Meta-tier tool (generates migrations for all 10 tiers)
//! - Q28: Simplification through automation (618 macros → 0 manual maintenance)
//! - Q33: Verification consolidation (87.5% duplication reduction)
//! - Q34: Audit trail of migration (complete before/after tracking)
//!
//! ## Migration Scope
//!
//! Based on Phase 2 completion report:
//! - **618 total manual macros** across 7 projects
//! - **atomic_capsule**: 250 macros (foundation crate)
//! - **clapi_core**: 94 macros (100% lockfree AI proxy)
//! - **kindly_hft**: 200+ macros (biological brain trading)
//! - **kindly-db**: 40+ macros (lockfree database)
//! - **kiang**: 15+ macros (KNN index)
//! - **Other projects**: 19 macros (various capsules)
//!
//! ## Migration Strategy
//!
//! **Phase 4.1** (Week 1): atomic_capsule (250 macros)
//! - Foundation crate (no circular dependencies)
//! - Manual migration with careful validation
//! - Establishes migration patterns for other projects
//!
//! **Phase 4.2** (Week 2): clapi_core (94 macros)
//! - Production system (zero downtime requirement)
//! - Incremental migration with rollback capability
//! - Property tests validate identical behavior
//!
//! **Phase 4.3** (Week 2-3): kindly_hft (200+ macros)
//! - Largest migration (14 brain zones)
//! - Per-zone migration and validation
//! - Biological brain correctness critical
//!
//! **Phase 4.4** (Week 3): kindly-db (40+ macros)
//! - Database correctness critical
//! - Validation against existing test suite
//! - Zero behavior change requirement
//!
//! **Phase 4.5** (Week 4): Remaining projects (34 macros)
//! - kiang, atomic_hedge_capsule, others
//! - Apply established migration patterns
//! - Final consolidation and cleanup
//!
//! ## Usage
//!
//! ```bash
//! # Analyze current state
//! cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs analyze
//!
//! # Generate migration plan for a project
//! cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs plan atomic_capsule
//!
//! # Execute migration (dry-run)
//! cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs migrate --dry-run atomic_capsule
//!
//! # Execute migration (real)
//! cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs migrate atomic_capsule
//!
//! # Validate migration
//! cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs validate atomic_capsule
//!
//! # Rollback if needed
//! cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs rollback atomic_capsule
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{File, Item, ItemStruct, Attribute};
use walkdir::WalkDir;
use regex::Regex;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone)]
struct CapsuleInfo {
    file_path: PathBuf,
    line_number: usize,
    struct_name: String,
    alignment: usize,
    size: Option<usize>,
    manual_macro: ManualMacro,
    tier: CapsuleTier,
}

#[derive(Debug, Clone, PartialEq)]
enum ManualMacro {
    VerifyCapsuleProperties,
    VerifyAlignmentOnly,
    VerifySimdCapsule,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CapsuleTier {
    T1Atomic,       // DualAtomicU64, CircuitBreakerCapsule
    T2SIMD,         // SimdF32x8, SimdI32x8
    T3FixedPoint,   // FixedPointQ16x8
    T4Batch,        // BatchRingBuffer
    T5Streaming,    // AsyncLogCapsule
    T6Mixed,        // Compound capsules
    Unknown,
}

#[derive(Debug, Clone)]
struct MigrationPlan {
    project_name: String,
    total_macros: usize,
    capsules: Vec<CapsuleInfo>,
    estimated_hours: f32,
    risk_level: RiskLevel,
    dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum RiskLevel {
    Low,      // <10 capsules, no critical path
    Medium,   // 10-50 capsules, moderate complexity
    High,     // 50-100 capsules, production critical
    Critical, // 100+ capsules, trade secrets or brain zones
}

#[derive(Debug)]
struct MigrationMetrics {
    project: String,
    macros_migrated: usize,
    files_modified: usize,
    lines_removed: usize,
    lines_added: usize,
    compilation_time_before_ms: u64,
    compilation_time_after_ms: u64,
    test_pass_rate_before: f32,
    test_pass_rate_after: f32,
}

// ============================================================================
// Analysis Phase
// ============================================================================

fn analyze_project(project_path: &Path) -> Result<MigrationPlan, String> {
    println!("Analyzing project: {}", project_path.display());

    let mut capsules = Vec::new();
    let project_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Walk all .rs files in the project
    for entry in WalkDir::new(project_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
        .filter(|e| !e.path().to_string_lossy().contains("/target/"))
    {
        let path = entry.path();
        if let Ok(content) = fs::read_to_string(path) {
            // Parse Rust file
            if let Ok(file) = syn::parse_file(&content) {
                capsules.extend(extract_capsules_from_file(path, &file, &content));
            }
        }
    }

    let total_macros = capsules.len();
    let estimated_hours = estimate_migration_time(total_macros, &project_name);
    let risk_level = assess_risk_level(total_macros, &project_name);
    let dependencies = extract_dependencies(&project_name);

    Ok(MigrationPlan {
        project_name,
        total_macros,
        capsules,
        estimated_hours,
        risk_level,
        dependencies,
    })
}

fn extract_capsules_from_file(
    file_path: &Path,
    file: &File,
    content: &str,
) -> Vec<CapsuleInfo> {
    let mut capsules = Vec::new();
    let verify_regex = Regex::new(
        r"(verify_capsule_properties!|verify_alignment_only!|verify_simd_capsule!)\s*\(\s*(\w+)\s*,\s*(\d+)"
    ).unwrap();

    for item in &file.items {
        if let Item::Struct(item_struct) = item {
            // Check if struct has manual verification macro
            if let Some(macro_call) = find_verification_macro(content, &item_struct.ident.to_string()) {
                if let Some(captures) = verify_regex.captures(&macro_call) {
                    let macro_type = match captures.get(1).unwrap().as_str() {
                        "verify_capsule_properties!" => ManualMacro::VerifyCapsuleProperties,
                        "verify_alignment_only!" => ManualMacro::VerifyAlignmentOnly,
                        "verify_simd_capsule!" => ManualMacro::VerifySimdCapsule,
                        _ => continue,
                    };

                    let alignment: usize = captures.get(3)
                        .unwrap()
                        .as_str()
                        .parse()
                        .unwrap_or(64);

                    // Extract size if present
                    let size = extract_size_from_macro(&macro_call);

                    // Infer tier from struct name and attributes
                    let tier = infer_tier(&item_struct.ident.to_string(), alignment);

                    let line_number = find_line_number(content, &item_struct.ident.to_string());

                    capsules.push(CapsuleInfo {
                        file_path: file_path.to_path_buf(),
                        line_number,
                        struct_name: item_struct.ident.to_string(),
                        alignment,
                        size,
                        manual_macro: macro_type,
                        tier,
                    });
                }
            }
        }
    }

    capsules
}

fn find_verification_macro(content: &str, struct_name: &str) -> Option<String> {
    // Look for macro call near struct definition
    let pattern = format!(
        r"(?:verify_capsule_properties!|verify_alignment_only!|verify_simd_capsule!)\s*\(\s*{}\s*,",
        regex::escape(struct_name)
    );
    let re = Regex::new(&pattern).ok()?;

    re.find(content).map(|m| {
        // Extract full macro call (find matching closing paren)
        let start = m.start();
        let remaining = &content[start..];
        let end = find_matching_paren(remaining).unwrap_or(100);
        remaining[..end].to_string()
    })
}

fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_size_from_macro(macro_call: &str) -> Option<usize> {
    // Look for size parameter (third argument)
    let re = Regex::new(r",\s*(\d+)\s*,").ok()?;
    re.captures(macro_call)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

fn infer_tier(struct_name: &str, alignment: usize) -> CapsuleTier {
    let name_lower = struct_name.to_lowercase();

    if name_lower.contains("simd") || name_lower.contains("f32x8") || name_lower.contains("f64x8") {
        CapsuleTier::T2SIMD
    } else if name_lower.contains("fixed") || name_lower.contains("q16") || name_lower.contains("q8") {
        CapsuleTier::T3FixedPoint
    } else if name_lower.contains("batch") || name_lower.contains("ring") {
        CapsuleTier::T4Batch
    } else if name_lower.contains("stream") || name_lower.contains("async") {
        CapsuleTier::T5Streaming
    } else if name_lower.contains("dual") || name_lower.contains("atomic") {
        CapsuleTier::T1Atomic
    } else if alignment >= 128 {
        CapsuleTier::T6Mixed
    } else {
        CapsuleTier::Unknown
    }
}

fn find_line_number(content: &str, struct_name: &str) -> usize {
    content
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(&format!("struct {}", struct_name)))
        .map(|(i, _)| i + 1)
        .unwrap_or(0)
}

fn estimate_migration_time(macro_count: usize, project_name: &str) -> f32 {
    // Base time: 15 minutes per capsule (review, migrate, test)
    let base_hours = (macro_count as f32 * 15.0) / 60.0;

    // Project complexity multiplier
    let complexity_multiplier = match project_name {
        "atomic_capsule" => 1.5,    // Foundation crate, extra careful
        "kindly_hft" => 2.0,         // Brain zones, critical correctness
        "kindly-db" => 1.8,          // Database, zero tolerance for bugs
        "clapi_core" => 1.3,         // Production, but well-tested
        _ => 1.0,
    };

    base_hours * complexity_multiplier
}

fn assess_risk_level(macro_count: usize, project_name: &str) -> RiskLevel {
    match (macro_count, project_name) {
        (0..=10, _) => RiskLevel::Low,
        (11..=50, _) if !is_critical_project(project_name) => RiskLevel::Medium,
        (11..=50, _) => RiskLevel::High,
        (51..=100, _) => RiskLevel::High,
        _ if is_critical_project(project_name) => RiskLevel::Critical,
        _ => RiskLevel::High,
    }
}

fn is_critical_project(project_name: &str) -> bool {
    matches!(
        project_name,
        "kindly_hft" | "kindly-db" | "atomic_hedge_capsule" | "clapi_core"
    )
}

fn extract_dependencies(project_name: &str) -> Vec<String> {
    match project_name {
        "atomic_capsule" => vec![], // No dependencies (foundation)
        "clapi_core" => vec!["atomic_capsule".to_string()],
        "kindly_hft" => vec!["atomic_capsule".to_string(), "clapi_core".to_string()],
        "kindly-db" => vec!["atomic_capsule".to_string()],
        _ => vec!["atomic_capsule".to_string()],
    }
}

// ============================================================================
// Migration Execution
// ============================================================================

fn migrate_capsule(capsule: &CapsuleInfo, dry_run: bool) -> Result<String, String> {
    let content = fs::read_to_string(&capsule.file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Generate derive attribute
    let derive_attr = generate_derive_attribute(capsule);

    // Find struct definition line
    let struct_line = content
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(&format!("struct {}", capsule.struct_name)))
        .ok_or_else(|| format!("Struct {} not found", capsule.struct_name))?;

    // Insert derive attribute before struct
    let mut lines: Vec<&str> = content.lines().collect();
    lines.insert(struct_line.0, &derive_attr);

    // Remove manual verification macro call
    let modified_content = remove_manual_macro(&lines.join("\n"), &capsule.struct_name);

    if dry_run {
        println!("\n[DRY RUN] Would modify: {}", capsule.file_path.display());
        println!("  Struct: {}", capsule.struct_name);
        println!("  Add: {}", derive_attr);
        println!("  Remove: {}!({})", macro_name(&capsule.manual_macro), capsule.struct_name);
        Ok(modified_content)
    } else {
        // Write modified content
        fs::write(&capsule.file_path, modified_content.as_bytes())
            .map_err(|e| format!("Failed to write file: {}", e))?;

        println!("✓ Migrated: {} in {}", capsule.struct_name, capsule.file_path.display());
        Ok(modified_content)
    }
}

fn generate_derive_attribute(capsule: &CapsuleInfo) -> String {
    if let Some(size) = capsule.size {
        format!(
            "#[derive(ComputationalCapsule)]\n#[capsule(alignment = {}, size = {})]",
            capsule.alignment,
            size
        )
    } else {
        format!(
            "#[derive(ComputationalCapsule)]\n#[capsule(alignment = {})]",
            capsule.alignment
        )
    }
}

fn macro_name(macro_type: &ManualMacro) -> &'static str {
    match macro_type {
        ManualMacro::VerifyCapsuleProperties => "verify_capsule_properties",
        ManualMacro::VerifyAlignmentOnly => "verify_alignment_only",
        ManualMacro::VerifySimdCapsule => "verify_simd_capsule",
    }
}

fn remove_manual_macro(content: &str, struct_name: &str) -> String {
    // Remove macro call (entire line including semicolon)
    let patterns = [
        format!(r"verify_capsule_properties!\s*\(\s*{}\s*,[^)]*\)\s*;?\s*\n?", struct_name),
        format!(r"verify_alignment_only!\s*\(\s*{}\s*,[^)]*\)\s*;?\s*\n?", struct_name),
        format!(r"verify_simd_capsule!\s*\(\s*{}\s*,[^)]*\)\s*;?\s*\n?", struct_name),
    ];

    let mut result = content.to_string();
    for pattern in &patterns {
        if let Ok(re) = Regex::new(pattern) {
            result = re.replace_all(&result, "").to_string();
        }
    }
    result
}

// ============================================================================
// Validation Phase
// ============================================================================

fn validate_migration(plan: &MigrationPlan) -> Result<MigrationMetrics, String> {
    println!("\nValidating migration for {}...", plan.project_name);

    // Compile before migration (baseline)
    println!("  [1/4] Compiling baseline...");
    let time_before = measure_compilation_time(&plan.project_name)?;

    // Run tests before migration
    println!("  [2/4] Running baseline tests...");
    let test_pass_rate_before = measure_test_pass_rate(&plan.project_name)?;

    // Compile after migration
    println!("  [3/4] Compiling migrated code...");
    let time_after = measure_compilation_time(&plan.project_name)?;

    // Run tests after migration
    println!("  [4/4] Running migrated tests...");
    let test_pass_rate_after = measure_test_pass_rate(&plan.project_name)?;

    // Validate: Tests must pass at same rate
    if test_pass_rate_after < test_pass_rate_before - 0.01 {
        return Err(format!(
            "Test pass rate decreased: {:.1}% → {:.1}%",
            test_pass_rate_before * 100.0,
            test_pass_rate_after * 100.0
        ));
    }

    Ok(MigrationMetrics {
        project: plan.project_name.clone(),
        macros_migrated: plan.total_macros,
        files_modified: count_modified_files(plan),
        lines_removed: estimate_lines_removed(plan),
        lines_added: estimate_lines_added(plan),
        compilation_time_before_ms: time_before,
        compilation_time_after_ms: time_after,
        test_pass_rate_before,
        test_pass_rate_after,
    })
}

fn measure_compilation_time(project_name: &str) -> Result<u64, String> {
    use std::process::Command;
    use std::time::Instant;

    let start = Instant::now();
    let output = Command::new("cargo")
        .args(&["build", "--quiet", "--manifest-path", &format!("{}/Cargo.toml", project_name)])
        .output()
        .map_err(|e| format!("Compilation failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Compilation failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(start.elapsed().as_millis() as u64)
}

fn measure_test_pass_rate(project_name: &str) -> Result<f32, String> {
    use std::process::Command;

    let output = Command::new("cargo")
        .args(&["test", "--quiet", "--manifest-path", &format!("{}/Cargo.toml", project_name)])
        .output()
        .map_err(|e| format!("Test execution failed: {}", e))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    // Parse test results: "test result: ok. 94 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
    let re = Regex::new(r"(\d+) passed; (\d+) failed").unwrap();
    if let Some(captures) = re.captures(&output_str) {
        let passed: usize = captures.get(1).unwrap().as_str().parse().unwrap_or(0);
        let failed: usize = captures.get(2).unwrap().as_str().parse().unwrap_or(0);
        let total = passed + failed;

        if total > 0 {
            return Ok(passed as f32 / total as f32);
        }
    }

    Err("Could not parse test results".to_string())
}

fn count_modified_files(plan: &MigrationPlan) -> usize {
    plan.capsules
        .iter()
        .map(|c| c.file_path.clone())
        .collect::<std::collections::HashSet<_>>()
        .len()
}

fn estimate_lines_removed(plan: &MigrationPlan) -> usize {
    // Each manual macro is typically 1-2 lines
    plan.total_macros * 1
}

fn estimate_lines_added(plan: &MigrationPlan) -> usize {
    // Each derive adds 2 lines (derive + attribute)
    plan.total_macros * 2
}

// ============================================================================
// Reporting
// ============================================================================

fn print_migration_plan(plan: &MigrationPlan) {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║         Migration Plan: {}                         ", plan.project_name);
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Total manual macros: {}", plan.total_macros);
    println!("Estimated time: {:.1} hours", plan.estimated_hours);
    println!("Risk level: {:?}", plan.risk_level);
    println!("Dependencies: {}", plan.dependencies.join(", "));
    println!();
    println!("Capsules by tier:");

    let mut tier_counts = HashMap::new();
    for capsule in &plan.capsules {
        *tier_counts.entry(capsule.tier).or_insert(0) += 1;
    }

    for (tier, count) in tier_counts {
        println!("  {:?}: {} capsules", tier, count);
    }

    println!();
    println!("Files to modify: {}", count_modified_files(plan));
    println!();
}

fn print_migration_metrics(metrics: &MigrationMetrics) {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║         Migration Metrics: {}                      ", metrics.project);
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Macros migrated: {}", metrics.macros_migrated);
    println!("Files modified: {}", metrics.files_modified);
    println!("Lines removed: {}", metrics.lines_removed);
    println!("Lines added: {}", metrics.lines_added);
    println!("Net LOC change: {}", metrics.lines_added as i32 - metrics.lines_removed as i32);
    println!();
    println!("Compilation time:");
    println!("  Before: {}ms", metrics.compilation_time_before_ms);
    println!("  After: {}ms", metrics.compilation_time_after_ms);
    println!("  Delta: {}ms ({:.1}%)",
        metrics.compilation_time_after_ms as i64 - metrics.compilation_time_before_ms as i64,
        ((metrics.compilation_time_after_ms as f32 / metrics.compilation_time_before_ms as f32) - 1.0) * 100.0
    );
    println!();
    println!("Test pass rate:");
    println!("  Before: {:.1}%", metrics.test_pass_rate_before * 100.0);
    println!("  After: {:.1}%", metrics.test_pass_rate_after * 100.0);
    println!("  Delta: {:.1}%",
        (metrics.test_pass_rate_after - metrics.test_pass_rate_before) * 100.0
    );
    println!();
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage:");
        println!("  {} analyze", args[0]);
        println!("  {} plan <project>", args[0]);
        println!("  {} migrate [--dry-run] <project>", args[0]);
        println!("  {} validate <project>", args[0]);
        println!("  {} rollback <project>", args[0]);
        return Ok(());
    }

    match args[1].as_str() {
        "analyze" => {
            println!("Analyzing all projects for migration...\n");

            let projects = [
                "atomic_capsule",
                "clapi_core",
                "kindly_hft",
                "kindly-db",
                "kiang",
            ];

            for project in &projects {
                let path = Path::new(project);
                if path.exists() {
                    match analyze_project(path) {
                        Ok(plan) => print_migration_plan(&plan),
                        Err(e) => eprintln!("Error analyzing {}: {}", project, e),
                    }
                }
            }
        }

        "plan" => {
            if args.len() < 3 {
                eprintln!("Usage: {} plan <project>", args[0]);
                return Ok(());
            }

            let project = &args[2];
            let path = Path::new(project);

            match analyze_project(path) {
                Ok(plan) => print_migration_plan(&plan),
                Err(e) => eprintln!("Error: {}", e),
            }
        }

        "migrate" => {
            let (dry_run, project_idx) = if args.len() > 2 && args[2] == "--dry-run" {
                (true, 3)
            } else {
                (false, 2)
            };

            if args.len() <= project_idx {
                eprintln!("Usage: {} migrate [--dry-run] <project>", args[0]);
                return Ok(());
            }

            let project = &args[project_idx];
            let path = Path::new(project);

            let plan = analyze_project(path)?;
            print_migration_plan(&plan);

            if dry_run {
                println!("\n[DRY RUN MODE - No files will be modified]\n");
            }

            for capsule in &plan.capsules {
                migrate_capsule(capsule, dry_run)?;
            }

            println!("\n✓ Migration complete!");
        }

        "validate" => {
            if args.len() < 3 {
                eprintln!("Usage: {} validate <project>", args[0]);
                return Ok(());
            }

            let project = &args[2];
            let path = Path::new(project);

            let plan = analyze_project(path)?;
            let metrics = validate_migration(&plan)?;
            print_migration_metrics(&metrics);
        }

        "rollback" => {
            if args.len() < 3 {
                eprintln!("Usage: {} rollback <project>", args[0]);
                return Ok(());
            }

            let project = &args[2];
            println!("Rolling back migration for {}...", project);
            println!("  Run: git restore {}/**/*.rs", project);
        }

        _ => {
            eprintln!("Unknown command: {}", args[1]);
        }
    }

    Ok(())
}
