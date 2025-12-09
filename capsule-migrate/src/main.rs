//! # Capsule Migration CLI
//!
//! Command-line interface for migrating computational capsules with nightly optimizations.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use capsule_migrate::{analyze_project, migrate_capsule};

#[derive(Parser)]
#[command(name = "capsule-migrate")]
#[command(about = "Computational capsule migration tool with nightly optimizations")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a project and generate migration plan
    Analyze {
        /// Project path to analyze
        project: PathBuf,
    },
    /// Execute migration (with optional dry-run)
    Migrate {
        /// Project path to migrate
        project: PathBuf,
        /// Dry-run (don't modify files)
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate migration report
    #[cfg(feature = "reports")]
    Report {
        /// Project path
        project: PathBuf,
        /// Output format (json or toml)
        #[arg(long, default_value = "json")]
        format: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { project } => {
            println!("Analyzing project: {}", project.display());
            let contexts = analyze_project(&project)?;
            println!("Found {} capsules to migrate", contexts.len());
            for context in &contexts {
                println!(
                    "  - {} ({}:{}): {:?} → {:?}",
                    context.struct_name,
                    context.file_path.display(),
                    context.line_number,
                    context.tier,
                    context.strategy
                );
            }
        }
        Commands::Migrate { project, dry_run } => {
            println!("Migrating project: {}", project.display());
            if dry_run {
                println!("[DRY RUN MODE - No files will be modified]");
            }
            let contexts = analyze_project(&project)?;
            for context in contexts {
                let result = migrate_capsule(&context, dry_run)?;
                println!(
                    "✓ Migrated {} in {:.1}s",
                    context.struct_name, result.elapsed_seconds
                );
            }
        }
        #[cfg(feature = "reports")]
        Commands::Report { project, format } => {
            println!("Generating migration report: {}", format);
            let contexts = analyze_project(&project)?;
            let mut results = vec![];
            for context in contexts {
                results.push(migrate_capsule(&context, true)?);
            }
            let report = capsule_migrate::generate_report(&results, &format)?;
            println!("{}", report);
        }
    }

    Ok(())
}
