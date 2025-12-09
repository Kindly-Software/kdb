use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

/// CCPM Installer - Install Claude Code Project Manager across all projects
#[derive(Parser)]
#[command(name = "ccpm-install")]
#[command(about = "Install CCPM (Claude Code Project Manager) across all Rust projects", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan and install CCPM in all projects under a directory
    InstallAll {
        /// Root directory to scan for projects (default: ~/Primitives)
        #[arg(short, long, default_value = "~/Primitives")]
        root: String,

        /// Dry run - show what would be installed without actually installing
        #[arg(short, long)]
        dry_run: bool,

        /// Skip projects that already have .claude directory
        #[arg(short, long)]
        skip_existing: bool,
    },

    /// Install CCPM in a specific project
    Install {
        /// Project directory
        path: PathBuf,

        /// Force installation even if .claude exists
        #[arg(short, long)]
        force: bool,
    },

    /// List all detected projects
    List {
        /// Root directory to scan
        #[arg(short, long, default_value = "~/Primitives")]
        root: String,
    },

    /// Download CCPM repository (one-time setup)
    Download {
        /// Where to download CCPM
        #[arg(short, long, default_value = "/tmp/ccpm-master")]
        output: PathBuf,
    },

    /// Verify CCPM installation in a project
    Verify {
        /// Project directory
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::InstallAll {
            root,
            dry_run,
            skip_existing,
        } => install_all(&root, dry_run, skip_existing)?,
        Commands::Install { path, force } => install_single(&path, force)?,
        Commands::List { root } => list_projects(&root)?,
        Commands::Download { output } => download_ccpm(&output)?,
        Commands::Verify { path } => verify_installation(&path)?,
    }

    Ok(())
}

/// Find all Rust projects (directories with Cargo.toml)
fn find_projects(root: &str) -> Result<Vec<PathBuf>> {
    let expanded_root = expand_tilde(root);
    let root_path = Path::new(&expanded_root);

    if !root_path.exists() {
        anyhow::bail!("Directory does not exist: {}", root);
    }

    println!(
        "{}",
        format!("🔍 Scanning for projects in: {}", root_path.display()).cyan()
    );

    let mut projects = Vec::new();

    for entry in WalkDir::new(root_path)
        .max_depth(3) // Don't go too deep
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name() == "Cargo.toml" {
            if let Some(project_dir) = entry.path().parent() {
                // Skip target directories and hidden directories
                if !project_dir.to_string_lossy().contains("/target/")
                    && !project_dir.to_string_lossy().contains("/.")
                {
                    projects.push(project_dir.to_path_buf());
                }
            }
        }
    }

    projects.sort();
    projects.dedup();

    Ok(projects)
}

/// Expand ~ to home directory
fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return format!("{}{}", home.to_string_lossy(), &path[1..]);
        }
    }
    path.to_string()
}

/// Download CCPM from GitHub
fn download_ccpm(output: &Path) -> Result<()> {
    println!("{}", "📦 Downloading CCPM from GitHub...".green().bold());

    // Create output directory
    fs::create_dir_all(output)
        .with_context(|| format!("Failed to create directory: {}", output.display()))?;

    // Clone using git
    let status = Command::new("git")
        .args(&[
            "clone",
            "--depth=1",
            "https://github.com/automazeio/ccpm.git",
            output.to_str().unwrap(),
        ])
        .status()
        .context("Failed to run git clone")?;

    if !status.success() {
        anyhow::bail!("Git clone failed");
    }

    // Remove .git directory
    let git_dir = output.join(".git");
    if git_dir.exists() {
        fs::remove_dir_all(&git_dir)?;
    }

    println!(
        "{}",
        format!("✓ CCPM downloaded to: {}", output.display())
            .green()
            .bold()
    );

    Ok(())
}

/// Install CCPM in a single project
fn install_single(project_path: &Path, force: bool) -> Result<()> {
    if !project_path.exists() {
        anyhow::bail!("Project directory does not exist: {}", project_path.display());
    }

    let claude_dir = project_path.join(".claude");

    // Check if .claude already exists
    if claude_dir.exists() && !force {
        println!(
            "{}",
            format!(
                "⚠ .claude directory already exists in: {}",
                project_path.display()
            )
            .yellow()
        );
        println!("  Use --force to overwrite, or manually merge the directories");
        return Ok(());
    }

    println!(
        "{}",
        format!("📦 Installing CCPM in: {}", project_path.display()).cyan()
    );

    // Download CCPM to temp location
    let temp_dir = tempfile::tempdir()?;
    let ccpm_path = temp_dir.path().join("ccpm");

    download_ccpm(&ccpm_path)?;

    // Copy .claude directory
    let source_claude = ccpm_path.join(".claude");
    if !source_claude.exists() {
        anyhow::bail!("CCPM .claude directory not found in downloaded repository");
    }

    copy_dir_recursive(&source_claude, &claude_dir)?;

    println!(
        "{}",
        format!("✓ CCPM installed in: {}", project_path.display())
            .green()
            .bold()
    );
    println!("\n{}", "Next steps:".yellow().bold());
    println!("  1. cd {}", project_path.display());
    println!("  2. Open Claude Code in this directory");
    println!("  3. Run: /pm:init");

    Ok(())
}

/// Install CCPM in all projects
fn install_all(root: &str, dry_run: bool, skip_existing: bool) -> Result<()> {
    let projects = find_projects(root)?;

    println!(
        "{}",
        format!("Found {} projects", projects.len())
            .green()
            .bold()
    );

    if dry_run {
        println!("{}", "\n🔍 DRY RUN - No changes will be made".yellow().bold());
    }

    // Download CCPM once to temp location
    let temp_dir = tempfile::tempdir()?;
    let ccpm_path = temp_dir.path().join("ccpm");

    if !dry_run {
        download_ccpm(&ccpm_path)?;
    }

    let source_claude = ccpm_path.join(".claude");

    let mut installed = 0;
    let mut skipped = 0;
    let mut errors = 0;

    for project in &projects {
        let claude_dir = project.join(".claude");
        let project_name = project
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        if claude_dir.exists() {
            if skip_existing {
                println!("  {} {}", "⏭".yellow(), project_name.dimmed());
                skipped += 1;
                continue;
            } else {
                println!(
                    "  {} {} {}",
                    "⚠".yellow(),
                    project_name,
                    "(will merge)".dimmed()
                );
            }
        }

        if dry_run {
            println!("  {} {} {}", "→".cyan(), project_name, "(would install)".dimmed());
        } else {
            match copy_dir_recursive(&source_claude, &claude_dir) {
                Ok(_) => {
                    println!("  {} {}", "✓".green(), project_name);
                    installed += 1;
                }
                Err(e) => {
                    println!("  {} {} - {}", "✗".red(), project_name, e);
                    errors += 1;
                }
            }
        }
    }

    println!("\n{}", "Summary:".bold());
    println!("  Installed: {}", installed.to_string().green());
    println!("  Skipped: {}", skipped.to_string().yellow());
    if errors > 0 {
        println!("  Errors: {}", errors.to_string().red());
    }

    if !dry_run && installed > 0 {
        println!("\n{}", "Next steps:".yellow().bold());
        println!("  For each project, run in Claude Code:");
        println!("    /pm:init");
    }

    Ok(())
}

/// List all detected projects
fn list_projects(root: &str) -> Result<()> {
    let projects = find_projects(root)?;

    println!(
        "{}",
        format!("\nFound {} projects:", projects.len())
            .green()
            .bold()
    );

    for project in &projects {
        let claude_exists = project.join(".claude").exists();
        let project_name = project
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let status = if claude_exists {
            "✓ CCPM installed".green()
        } else {
            "○ No CCPM".dimmed()
        };

        println!("  {} - {}", project_name.cyan(), status);
    }

    Ok(())
}

/// Verify CCPM installation
fn verify_installation(project_path: &Path) -> Result<()> {
    let claude_dir = project_path.join(".claude");

    println!(
        "{}",
        format!("🔍 Verifying CCPM in: {}", project_path.display()).cyan()
    );

    if !claude_dir.exists() {
        println!("{}", "  ✗ .claude directory not found".red());
        return Ok(());
    }

    // Check for required CCPM directories/files
    let required_dirs = vec!["agents", "commands", "context", "prds", "epics"];
    let mut missing = Vec::new();

    for dir_name in &required_dirs {
        let dir_path = claude_dir.join(dir_name);
        if dir_path.exists() {
            println!("  {} {}/", "✓".green(), dir_name);
        } else {
            println!("  {} {}/ (missing)", "✗".red(), dir_name);
            missing.push(dir_name);
        }
    }

    if missing.is_empty() {
        println!("\n{}", "✓ CCPM installation verified!".green().bold());
    } else {
        println!(
            "\n{}",
            format!(
                "⚠ Installation incomplete - missing {} directories",
                missing.len()
            )
            .yellow()
        );
    }

    Ok(())
}

/// Recursively copy directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
