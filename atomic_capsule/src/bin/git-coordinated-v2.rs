//! git-coordinated - Lockfree git wrapper using GitDaemonCapsule
//!
//! A transparent command-line tool that wraps ANY git command to prevent .git/index.lock
//! conflicts by coordinating all operations through a lockfree daemon.
//!
//! ## Usage
//! ```bash
//! git-coordinated <git-command> [args...]
//!
//! Examples:
//!   git-coordinated add src/main.rs
//!   git-coordinated commit -m "Update feature" --no-verify
//!   git-coordinated push origin main --force
//!   git-coordinated status
//!   git-coordinated rebase -i HEAD~3
//!   git-coordinated cherry-pick abc123
//!   git-coordinated stash push -m "WIP"
//!   git-coordinated stats                        # Show coordination statistics
//! ```
//!
//! ## Benefits
//! - **Fully transparent**: Supports ALL git commands (not just a hardcoded subset)
//! - **Flag preservation**: All flags and options are preserved (--no-verify, --force, -i, etc.)
//! - **No .git/index.lock conflicts**: Serializes operations through lockfree coordinator
//! - **Low overhead**: <50ns coordination overhead per operation
//! - **Stale lock recovery**: Automatic recovery if git process crashes
//! - **Q34 audit trail**: All operations logged for compliance
//! - **Statistics**: Monitor lock contention and coordination overhead
//! - **Alias-ready**: Can be used as `alias git="git-coordinated"` for transparent replacement

use atomic_capsule::daemon::{DaemonError, GitDaemonCapsule};
use std::env;
use std::path::PathBuf;
use std::process;

fn main() {
    // Parse arguments
    let args: Vec<String> = env::args().skip(1).collect();

    // Handle built-in commands
    if args.is_empty() {
        print_help();
        process::exit(0);
    }

    match args[0].as_str() {
        "help" | "--help" | "-h" => {
            print_help();
            process::exit(0);
        }
        "version" | "--version" | "-v" => {
            println!("git-coordinated v0.2.0");
            println!("Transparent lockfree git coordination");
            process::exit(0);
        }
        "stats" => {
            // Find .git directory
            let repo_path = match find_git_repo() {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    process::exit(1);
                }
            };

            // Create daemon and show stats
            let daemon = match GitDaemonCapsule::new(&repo_path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Failed to initialize git daemon: {:?}", e);
                    process::exit(1);
                }
            };
            print_stats(&daemon);
            process::exit(0);
        }
        _ => {
            // ALL OTHER COMMANDS: Passthrough to git with coordination
            handle_git_command(args);
        }
    }
}

/// Handle ANY git command with coordination
fn handle_git_command(args: Vec<String>) {
    // Find .git directory (walk up from current dir)
    let repo_path = match find_git_repo() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(128); // Standard git error code for "not a repository"
        }
    };

    // Create daemon coordinator
    let daemon = match GitDaemonCapsule::new(&repo_path) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("Error: Not a git repository (or any parent up to mount point)");
            process::exit(128);
        }
    };

    // Convert args to &str slices for with_git_cmd
    let cmd_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    // Execute the git command with coordination
    match daemon.with_git_cmd(&cmd_args) {
        Ok(output) => {
            // Feature 1: Output preservation
            if !output.is_empty() {
                print!("{}", output);
            }
            process::exit(0);
        }
        Err(DaemonError::InvalidState) => {
            // Feature 3: Better error handling - Git error was already printed by git itself
            // Just exit with standard git error code
            process::exit(1);
        }
        Err(e) => {
            // Feature 3: Better error messages for coordination errors
            eprintln!("Git coordination error: {}", e);
            process::exit(2);
        }
    }
}

/// Print help message
fn print_help() {
    println!("git-coordinated v0.2.0");
    println!("Transparent lockfree git coordination wrapper");
    println!();
    println!("USAGE:");
    println!("    git-coordinated <COMMAND> [ARGS]...");
    println!();
    println!("COMMANDS:");
    println!("    <any-git-command>   Execute any git command with coordination");
    println!("    stats               Show lock coordination statistics");
    println!("    help, --help, -h    Print this help message");
    println!("    version, --version  Print version information");
    println!();
    println!("EXAMPLES:");
    println!("    git-coordinated add src/main.rs");
    println!("    git-coordinated commit -m \"Update feature\" --no-verify");
    println!("    git-coordinated push origin main --force");
    println!("    git-coordinated rebase -i HEAD~3");
    println!("    git-coordinated cherry-pick abc123");
    println!("    git-coordinated stash push -m \"WIP\"");
    println!("    git-coordinated stats");
    println!();
    println!("FEATURES:");
    println!("    - Fully transparent: Supports ALL git commands");
    println!("    - Flag preservation: All flags and options work (--no-verify, --force, -i, etc.)");
    println!("    - Lock prevention: Prevents .git/index.lock conflicts");
    println!("    - Low overhead: <50ns coordination per operation");
    println!("    - Alias-ready: Can be used as 'alias git=\"git-coordinated\"'");
    println!();
    println!("MORE INFO:");
    println!("    For standard git help, use: git help <command>");
}

/// Find the .git directory by walking up from current directory
fn find_git_repo() -> Result<PathBuf, String> {
    let mut current = env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {}", e))?;

    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() {
            return Ok(current);
        }

        if !current.pop() {
            return Err("Not a git repository (or any parent up to mount point)".to_string());
        }
    }
}

/// Print daemon statistics
fn print_stats(daemon: &GitDaemonCapsule) {
    let stats = daemon.stats();

    println!("Git Coordinator Statistics:");
    println!("  Lock acquires:     {}", stats.lock_acquires);
    println!("  Lock contentions:  {}", stats.lock_contentions);
    println!("  Stale recoveries:  {}", stats.lock_stale_recoveries);

    #[cfg(feature = "queue-bounded")]
    {
        println!("  Queue enqueues:    {}", stats.queue_enqueues);
        println!("  Queue dequeues:    {}", stats.queue_dequeues);
        println!("  Queue max depth:   {}", stats.queue_max_depth);
    }

    println!("  Audit entries:     {}", stats.audit_entries);

    if stats.lock_acquires > 0 {
        let contention_ratio =
            (stats.lock_contentions as f64 / stats.lock_acquires as f64) * 100.0;
        println!();
        if contention_ratio > 0.0 {
            println!("  Contention ratio:  {:.2}%", contention_ratio);
        } else {
            println!("  Contention ratio:  0.00% (uncontended)");
        }
    }

    println!();
    println!("Legend:");
    println!("  - Lock acquires: Total number of lock acquisitions");
    println!("  - Contentions: Number of times lock was held by another process");
    println!("  - Stale recoveries: Locks recovered from dead processes");
    #[cfg(feature = "queue-bounded")]
    {
        println!("  - Queue operations: Batch work queue statistics");
    }
    println!("  - Audit entries: Number of operations logged to audit trail");
}
