//! # fix_padding_fields - Automated Padding Field Calculator
//!
//! This tool automatically calculates and fixes padding fields in computational capsules
//! to match the required alignment.
//!
//! ## Purpose
//!
//! When migrating from Phase 1 (manual padding) to Phase 2 (automatic padding), padding fields
//! must be correctly sized. This tool:
//!
//! 1. Parses Rust struct definitions
//! 2. Calculates actual data field sizes
//! 3. Computes required padding to match alignment
//! 4. Adds or fixes `_padding` fields automatically
//! 5. Validates all changes with compilation tests
//!
//! ## UCE34 Framework Alignment
//!
//! - **Q28 (Simplicity)**: Single tool replaces manual padding calculations (100+ capsules)
//! - **Q31 (Rust Transform)**: Syn/quote for accurate AST parsing and generation
//! - **Q33 (Validation)**: Compile-time verification that alignment equals size
//!
//! ## Usage
//!
//! ```bash
//! # Fix padding in a single file
//! cargo run -- fix src/my_capsule.rs
//!
//! # Analyze all capsules in a project
//! cargo run -- analyze /home/samuel/Primitives/atomic_capsule/src
//!
//! # Dry-run (no changes)
//! cargo run -- fix --dry-run src/my_capsule.rs
//!
//! # Report padding issues
//! cargo run -- report src/my_capsule.rs
//! ```

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use fix_padding_fields::{fix_padding_file, extract_capsules, PaddingCalculator, ToolStateCapsule};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use walkdir::WalkDir;

/// Computational capsule padding field calculator and fixer.
#[derive(Parser, Debug)]
#[command(name = "fix_padding_fields")]
#[command(about = "Automated padding field calculation and fixing for computational capsules", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Fix padding fields in a Rust source file or directory.
    Fix {
        /// Path to file or directory to fix
        path: PathBuf,

        /// Dry-run mode (no changes to files)
        #[arg(long)]
        dry_run: bool,

        /// Backup original files before modification
        #[arg(long, default_value = "true")]
        backup: bool,
    },

    /// Analyze padding fields without making changes.
    Analyze {
        /// Path to file or directory to analyze
        path: PathBuf,

        /// Show detailed field breakdown for each capsule
        #[arg(long)]
        verbose: bool,
    },

    /// Generate a report of padding issues.
    Report {
        /// Path to file or directory to analyze
        path: PathBuf,

        /// Output file for the report (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Validate padding in a specific file.
    Validate {
        /// Path to file to validate
        path: PathBuf,

        /// Expected alignment in bytes
        #[arg(long)]
        alignment: usize,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Fix {
            path,
            dry_run,
            backup,
        } => {
            fix_command(&path, dry_run, backup)?;
        }
        Commands::Analyze { path, verbose } => {
            analyze_command(&path, verbose)?;
        }
        Commands::Report { path, output } => {
            report_command(&path, output)?;
        }
        Commands::Validate { path, alignment } => {
            validate_command(&path, alignment)?;
        }
    }

    Ok(())
}

/// Execute fix command: calculate and apply padding fixes.
///
/// Uses ToolStateCapsule for lockfree metrics tracking (P0.5).
fn fix_command(path: &PathBuf, dry_run: bool, backup: bool) -> Result<()> {
    let files = collect_rust_files(path)?;

    if files.is_empty() {
        eprintln!("No Rust files found at: {}", path.display());
        return Ok(());
    }

    // P0.5: Create ToolStateCapsule for metrics tracking
    let state = Arc::new(ToolStateCapsule::new());

    for file_path in files {
        let content = fs::read_to_string(&file_path)?;

        // P0.1: Parse capsules
        let capsules = extract_capsules(&content)?;

        if capsules.is_empty() {
            continue;
        }

        println!("Processing: {}", file_path.display());

        if dry_run {
            println!("  [DRY-RUN] Would fix {} capsule(s)", capsules.len());
            for capsule_name in capsules.iter().map(|c| &c.name) {
                println!("    - {}", capsule_name);
            }
            state.increment_files();
        } else {
            // P0.8: Use unified lib.rs API
            match fix_padding_file(&content, &file_path) {
                Ok((new_content, stats)) => {
                    state.increment_files();

                    if stats.capsules_fixed > 0 {
                        // Backup original
                        if backup {
                            let backup_path = file_path.with_extension("rs.bak");
                            fs::copy(&file_path, &backup_path)?;
                            println!("  Backup: {}", backup_path.display());
                        }

                        // Write modified content
                        fs::write(&file_path, new_content)?;
                        println!("  ✓ Fixed: {} capsule(s)", stats.capsules_fixed);

                        // Update metrics
                        for _ in 0..stats.capsules_fixed {
                            state.increment_fixes();
                        }
                        state.add_bytes(stats.bytes_modified);
                    } else {
                        println!("  - No changes needed");
                    }
                }
                Err(e) => {
                    state.increment_errors();
                    eprintln!("  ✗ Error: {}", e);
                }
            }
        }
    }

    // P0.5: Print final summary from ToolStateCapsule
    let summary = state.summary();
    println!("\n=== Summary ===");
    println!("Files processed: {}", summary.files_processed);
    println!("Capsules fixed:  {}", summary.capsules_fixed);
    println!("Errors:          {}", summary.errors_encountered);
    println!("Bytes modified:  {}", summary.bytes_modified);

    Ok(())
}

/// Execute analyze command: report padding status without changes.
fn analyze_command(path: &PathBuf, verbose: bool) -> Result<()> {
    let files = collect_rust_files(path)?;

    if files.is_empty() {
        eprintln!("No Rust files found at: {}", path.display());
        return Ok(());
    }

    let mut total_capsules = 0;
    let mut needs_fixing = 0;

    for file_path in files {
        let content = fs::read_to_string(&file_path)?;
        let capsules = extract_capsules(&content)?;

        if capsules.is_empty() {
            continue;
        }

        println!("File: {}", file_path.display());
        total_capsules += capsules.len();

        for capsule in capsules {
            let calculator = PaddingCalculator::new(&capsule)?;
            let status = calculator.needs_fixing();

            if status {
                needs_fixing += 1;
                println!(
                    "  ⚠ {}: alignment={}, current_padding={}, needed={}",
                    capsule.name,
                    capsule.alignment,
                    capsule.padding_size().unwrap_or(0),
                    calculator.required_padding()
                );

                if verbose {
                    println!("    Fields:");
                    for field in &capsule.fields {
                        println!("      - {}: {} bytes", field.name, field.size_bytes);
                    }
                    println!("    Total data: {} bytes", calculator.total_data_size());
                    println!(
                        "    Required padding: {} bytes",
                        calculator.required_padding()
                    );
                }
            } else {
                println!("  ✓ {}: OK", capsule.name);
            }
        }
    }

    println!(
        "\nSummary: {} of {} capsules need fixing",
        needs_fixing, total_capsules
    );
    Ok(())
}

/// Execute report command: generate a report of padding issues.
fn report_command(path: &PathBuf, output: Option<PathBuf>) -> Result<()> {
    let files = collect_rust_files(path)?;

    if files.is_empty() {
        eprintln!("No Rust files found at: {}", path.display());
        return Ok(());
    }

    let mut report = String::from("# Padding Field Analysis Report\n\n");
    let mut total_capsules = 0;
    let mut needs_fixing = 0;

    for file_path in files {
        let content = fs::read_to_string(&file_path)?;
        let capsules = extract_capsules(&content)?;

        if capsules.is_empty() {
            continue;
        }

        report.push_str(&format!("## File: {}\n\n", file_path.display()));
        total_capsules += capsules.len();

        for capsule in capsules {
            let calculator = PaddingCalculator::new(&capsule)?;
            let status = calculator.needs_fixing();

            if status {
                needs_fixing += 1;
                report.push_str(&format!("### {} (NEEDS FIXING)\n\n", capsule.name));
                report.push_str(&format!("- **Alignment**: {} bytes\n", capsule.alignment));
                report.push_str(&format!(
                    "- **Current padding**: {} bytes\n",
                    capsule.padding_size().unwrap_or(0)
                ));
                report.push_str(&format!(
                    "- **Required padding**: {} bytes\n",
                    calculator.required_padding()
                ));
                report.push_str(&format!(
                    "- **Data size**: {} bytes\n\n",
                    calculator.total_data_size()
                ));

                report.push_str("**Fields**:\n");
                for field in &capsule.fields {
                    report.push_str(&format!("- `{}`: {} bytes\n", field.name, field.size_bytes));
                }
                report.push('\n');
            } else {
                report.push_str(&format!("### {} (OK)\n\n", capsule.name));
                report.push_str("Padding is correctly configured.\n\n");
            }
        }
    }

    report.push_str("\n## Summary\n\n");
    report.push_str(&format!("- **Total capsules**: {}\n", total_capsules));
    report.push_str(&format!("- **Need fixing**: {}\n", needs_fixing));
    report.push_str(&format!("- **OK**: {}\n", total_capsules - needs_fixing));

    if let Some(output_path) = output {
        fs::write(&output_path, report)?;
        println!("Report written to: {}", output_path.display());
    } else {
        println!("{}", report);
    }

    Ok(())
}

/// Execute validate command: check padding for specific alignment.
fn validate_command(path: &PathBuf, alignment: usize) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("File not found: {}", path.display()));
    }

    let content = fs::read_to_string(path)?;
    let capsules = extract_capsules(&content)?;

    if capsules.is_empty() {
        println!("No computational capsules found in: {}", path.display());
        return Ok(());
    }

    let mut valid = 0;
    let mut invalid = 0;

    for capsule in capsules {
        if capsule.alignment == alignment {
            let calculator = PaddingCalculator::new(&capsule)?;
            if !calculator.needs_fixing() {
                println!(
                    "✓ {}: Valid (size={}, alignment={})",
                    capsule.name, capsule.total_size, alignment
                );
                valid += 1;
            } else {
                println!(
                    "✗ {}: Invalid padding (needs {} bytes)",
                    capsule.name,
                    calculator.required_padding()
                );
                invalid += 1;
            }
        }
    }

    if invalid > 0 {
        return Err(anyhow!("{} capsule(s) have invalid padding", invalid));
    }

    println!("\n✓ All {} capsule(s) validated successfully", valid);
    Ok(())
}

/// Collect all Rust files from path (file or directory).
fn collect_rust_files(path: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path.clone());
        }
    } else if path.is_dir() {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        {
            files.push(entry.path().to_path_buf());
        }
    }

    Ok(files)
}
