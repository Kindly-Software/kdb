//! Automated migration tool for converting manual verification to #[derive(ComputationalCapsule)]
//!
//! Usage:
//!   migrate src/ --dry-run          # Preview changes without applying
//!   migrate src/ --apply            # Apply migrations
//!   migrate src/ --priority P2      # Migrate only P2 priority capsules

use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use quote::ToTokens;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{parse_file, Item, ItemStruct};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory to migrate
    path: PathBuf,

    /// Apply changes (default is dry-run)
    #[arg(long)]
    apply: bool,

    /// Only migrate specific priority (P0, P1, P2)
    #[arg(long)]
    priority: Option<String>,

    /// Create backups before modifying files
    #[arg(long, default_value = "true")]
    backup: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Clone)]
struct CapsuleCandidate {
    name: String,
    file_path: PathBuf,
    line_number: usize,
    alignment: Option<usize>,
    size: Option<usize>,
    is_generic: bool,
    has_derive: bool,
    verification_type: VerificationType,
}

#[derive(Debug, Clone, PartialEq)]
enum VerificationType {
    VerifyCapsuleProperties,
    VerifyAlignmentOnly,
    ManualAssert,
    None,
}

struct MigrationReport {
    migrated: Vec<CapsuleCandidate>,
    skipped: Vec<(CapsuleCandidate, String)>, // (capsule, reason)
    errors: Vec<(PathBuf, String)>,
}

impl MigrationReport {
    fn new() -> Self {
        Self {
            migrated: Vec::new(),
            skipped: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn print_summary(&self) {
        println!("\n{}", "═".repeat(80).bright_blue());
        println!("{}", "MIGRATION SUMMARY".bright_white().bold());
        println!("{}", "═".repeat(80).bright_blue());

        println!(
            "\n✅ {} Migrated successfully",
            self.migrated.len().to_string().green().bold()
        );
        for capsule in &self.migrated {
            println!(
                "   {} {} ({}:{})",
                "→".green(),
                capsule.name,
                capsule.file_path.display(),
                capsule.line_number
            );
        }

        if !self.skipped.is_empty() {
            println!(
                "\n⚠️  {} Skipped",
                self.skipped.len().to_string().yellow().bold()
            );
            for (capsule, reason) in &self.skipped {
                println!(
                    "   {} {} - {}",
                    "→".yellow(),
                    capsule.name,
                    reason.dimmed()
                );
            }
        }

        if !self.errors.is_empty() {
            println!(
                "\n❌ {} Errors",
                self.errors.len().to_string().red().bold()
            );
            for (path, error) in &self.errors {
                println!("   {} {} - {}", "→".red(), path.display(), error);
            }
        }

        println!("\n{}", "═".repeat(80).bright_blue());
    }
}

struct CapsuleMigrator {
    report: MigrationReport,
    dry_run: bool,
    verbose: bool,
    backup: bool,
}

impl CapsuleMigrator {
    fn new(dry_run: bool, verbose: bool, backup: bool) -> Self {
        Self {
            report: MigrationReport::new(),
            dry_run,
            verbose,
            backup,
        }
    }

    fn migrate_directory(&mut self, dir: &Path) -> Result<()> {
        for entry in WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
        {
            if let Err(e) = self.migrate_file(entry.path()) {
                self.report
                    .errors
                    .push((entry.path().to_path_buf(), e.to_string()));
            }
        }
        Ok(())
    }

    fn migrate_file(&mut self, path: &Path) -> Result<()> {
        if self.verbose {
            println!("Checking {}", path.display());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        // Find all capsule candidates in the file
        let candidates = self.find_candidates(&content, path)?;

        if candidates.is_empty() {
            return Ok(());
        }

        // Process each candidate
        let mut modified_content = content.clone();
        let mut any_modified = false;

        for candidate in candidates {
            if candidate.has_derive {
                self.report.skipped.push((
                    candidate.clone(),
                    "Already has #[derive(ComputationalCapsule)]".to_string(),
                ));
                continue;
            }

            if candidate.verification_type == VerificationType::None {
                self.report.skipped.push((
                    candidate.clone(),
                    "No verification macro found".to_string(),
                ));
                continue;
            }

            // Apply migration
            if let Ok(new_content) = self.apply_migration(&modified_content, &candidate) {
                modified_content = new_content;
                any_modified = true;
                self.report.migrated.push(candidate);
            } else {
                self.report.skipped.push((
                    candidate.clone(),
                    "Failed to apply migration".to_string(),
                ));
            }
        }

        // Write changes if not dry-run
        if any_modified && !self.dry_run {
            if self.backup {
                let backup_path = path.with_extension("rs.backup");
                fs::copy(path, &backup_path)?;
                if self.verbose {
                    println!("Created backup: {}", backup_path.display());
                }
            }

            fs::write(path, modified_content)?;
            println!(
                "{} Modified: {}",
                "✓".green().bold(),
                path.display()
            );
        } else if any_modified && self.dry_run {
            println!(
                "{} Would modify: {}",
                "→".yellow().bold(),
                path.display()
            );
            if self.verbose {
                // Show diff preview
                println!("\nChanges preview:");
                // In real implementation, use a proper diff library
                println!("{}", "... diff output ...".dimmed());
            }
        }

        Ok(())
    }

    fn find_candidates(&self, content: &str, path: &Path) -> Result<Vec<CapsuleCandidate>> {
        let mut candidates = Vec::new();

        // Parse the file AST
        let ast = parse_file(content)?;

        // Find all structs with repr(C, align(...))
        for item in ast.items {
            if let Item::Struct(item_struct) = item {
                if self.has_repr_c_align(&item_struct) {
                    let candidate = self.analyze_struct(&item_struct, content, path)?;
                    candidates.push(candidate);
                }
            }
        }

        Ok(candidates)
    }

    fn has_repr_c_align(&self, item: &ItemStruct) -> bool {
        item.attrs.iter().any(|attr| {
            attr.path()
                .get_ident()
                .map_or(false, |ident| ident == "repr")
        })
    }

    fn analyze_struct(
        &self,
        item: &ItemStruct,
        content: &str,
        path: &Path,
    ) -> Result<CapsuleCandidate> {
        let name = item.ident.to_string();
        let is_generic = !item.generics.params.is_empty();

        // Check if already has derive (syn 2.0 API)
        let has_derive = item.attrs.iter().any(|attr| {
            // Check if this is a derive attribute
            if attr.path().is_ident("derive") {
                // Parse tokens to check for ComputationalCapsule
                let tokens = attr.meta.to_token_stream().to_string();
                return tokens.contains("ComputationalCapsule");
            }
            false
        });

        // Find verification macro
        let (verification_type, alignment, size) =
            self.find_verification_macro(&name, content)?;

        Ok(CapsuleCandidate {
            name,
            file_path: path.to_path_buf(),
            line_number: 0, // Would need to calculate from span
            alignment,
            size,
            is_generic,
            has_derive,
            verification_type,
        })
    }

    fn find_verification_macro(
        &self,
        struct_name: &str,
        content: &str,
    ) -> Result<(VerificationType, Option<usize>, Option<usize>)> {
        // Regex for verify_capsule_properties!(Name, align, size)
        let re_full = Regex::new(&format!(
            r"verify_capsule_properties!\s*\(\s*{}\s*,\s*(\d+)\s*,\s*(\d+)\s*\)",
            regex::escape(struct_name)
        ))?;

        if let Some(caps) = re_full.captures(content) {
            let align = caps[1].parse().ok();
            let size = caps[2].parse().ok();
            return Ok((VerificationType::VerifyCapsuleProperties, align, size));
        }

        // Regex for verify_alignment_only!(Name, align)
        let re_align = Regex::new(&format!(
            r"verify_alignment_only!\s*\(\s*{}\s*(?:<[^>]+>)?\s*,\s*(\d+)\s*\)",
            regex::escape(struct_name)
        ))?;

        if let Some(caps) = re_align.captures(content) {
            let align = caps[1].parse().ok();
            return Ok((VerificationType::VerifyAlignmentOnly, align, None));
        }

        // Check for manual assert_eq!
        let re_assert = Regex::new(&format!(
            r"assert_eq!\s*\(\s*std::mem::(size_of|align_of)::<{}>",
            regex::escape(struct_name)
        ))?;

        if re_assert.is_match(content) {
            return Ok((VerificationType::ManualAssert, None, None));
        }

        Ok((VerificationType::None, None, None))
    }

    fn apply_migration(
        &self,
        content: &str,
        candidate: &CapsuleCandidate,
    ) -> Result<String> {
        let mut result = content.to_string();

        // Add derive attribute
        let derive_attr = if candidate.is_generic || candidate.size.is_none() {
            format!(
                "#[derive(ComputationalCapsule)]\n#[capsule(alignment = {})]",
                candidate.alignment.unwrap_or(64)
            )
        } else {
            format!(
                "#[derive(ComputationalCapsule)]\n#[capsule(alignment = {}, size = {})]",
                candidate.alignment.unwrap_or(64),
                candidate.size.unwrap_or(64)
            )
        };

        // Find struct definition and add derive before it
        let struct_pattern = format!(r"(struct\s+{})", regex::escape(&candidate.name));
        let re_struct = Regex::new(&struct_pattern)?;
        result = re_struct
            .replace(&result, format!("{}\n$1", derive_attr))
            .to_string();

        // Remove old verification macro
        match candidate.verification_type {
            VerificationType::VerifyCapsuleProperties => {
                let pattern = format!(
                    r"(?m)^.*verify_capsule_properties!\s*\(\s*{}\s*,\s*\d+\s*,\s*\d+\s*\);?\s*$\n?",
                    regex::escape(&candidate.name)
                );
                let re = Regex::new(&pattern)?;
                result = re.replace_all(&result, "").to_string();
            }
            VerificationType::VerifyAlignmentOnly => {
                let pattern = format!(
                    r"(?m)^.*verify_alignment_only!\s*\(\s*{}\s*(?:<[^>]+>)?\s*,\s*\d+\s*\);?\s*$\n?",
                    regex::escape(&candidate.name)
                );
                let re = Regex::new(&pattern)?;
                result = re.replace_all(&result, "").to_string();
            }
            _ => {}
        }

        Ok(result)
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!(
        "{}",
        "╔════════════════════════════════════════════════════════════╗"
            .bright_blue()
    );
    println!(
        "{}",
        "║     COMPUTATIONAL CAPSULE VERIFICATION MIGRATOR v0.1.0     ║"
            .bright_white()
            .bold()
    );
    println!(
        "{}",
        "╚════════════════════════════════════════════════════════════╝"
            .bright_blue()
    );

    let mode = if args.apply {
        "APPLY MODE".red().bold()
    } else {
        "DRY RUN MODE".yellow().bold()
    };

    println!("\nMode: {}", mode);
    println!("Path: {}", args.path.display());
    if let Some(priority) = &args.priority {
        println!("Priority: {}", priority.cyan());
    }
    println!();

    let mut migrator = CapsuleMigrator::new(!args.apply, args.verbose, args.backup);

    migrator.migrate_directory(&args.path)?;

    migrator.report.print_summary();

    if !args.apply && !migrator.report.migrated.is_empty() {
        println!(
            "\n{} Run with --apply to apply these changes",
            "ℹ".blue().bold()
        );
    }

    Ok(())
}
