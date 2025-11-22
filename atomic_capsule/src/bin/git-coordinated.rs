//! git-coordinated - Lockfree git wrapper using GitDaemonCapsule
//!
//! A command-line tool that wraps git commands to prevent .git/index.lock conflicts
//! by coordinating all operations through a lockfree daemon.
//!
//! ## Usage
//! ```bash
//! git-coordinated <git-command> [args...]
//!
//! Examples:
//!   git-coordinated add src/main.rs
//!   git-coordinated commit -m "Update feature"
//!   git-coordinated push origin main
//!   git-coordinated status
//!   git-coordinated stats    # Show coordination statistics
//! ```
//!
//! ## Benefits
//! - **No .git/index.lock conflicts**: Serializes operations through lockfree coordinator
//! - **Low overhead**: <50ns coordination overhead per operation
//! - **Stale lock recovery**: Automatic recovery if git process crashes
//! - **Q34 audit trail**: All operations logged for compliance
//! - **Statistics**: Monitor lock contention and coordination overhead

use atomic_capsule::daemon::{GitDaemonCapsule, DaemonError};
use std::env;
use std::path::PathBuf;
use std::process;

fn main() {
    // Parse arguments
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: git-coordinated <git-command> [args...]");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  git-coordinated add file.txt");
        eprintln!("  git-coordinated commit -m 'message'");
        eprintln!("  git-coordinated push origin main");
        eprintln!("  git-coordinated status");
        eprintln!("  git-coordinated stats    # Show coordinator statistics");
        process::exit(1);
    }

    // Find .git directory (walk up from current dir)
    let repo_path = match find_git_repo() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    // Create daemon coordinator
    let daemon = match GitDaemonCapsule::new(&repo_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to initialize git daemon: {:?}", e);
            process::exit(1);
        }
    };

    // Handle special commands
    match args[0].as_str() {
        "stats" => {
            print_stats(&daemon);
            return;
        }
        "help" | "--help" | "-h" => {
            print_help();
            return;
        }
        "version" | "--version" | "-v" => {
            println!("git-coordinated 0.1.0");
            return;
        }
        _ => {}
    };

    // Execute git command with coordination
    let result = match args[0].as_str() {
        "add" => {
            let files: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
            if files.is_empty() {
                eprintln!("Error: git add requires at least one file");
                process::exit(1);
            }
            daemon.git_add(&files).map(|_| "".to_string())
        }
        "commit" => {
            // Parse commit message (handle -m flag)
            let message = parse_commit_message(&args[1..]);
            if message.is_empty() {
                eprintln!("Error: commit requires a message (-m flag)");
                process::exit(1);
            }
            daemon.git_commit(&message).map(|_| "".to_string())
        }
        "push" => {
            let remote = args.get(1).map(|s| s.as_str()).unwrap_or("origin");
            let branch = args.get(2).map(|s| s.as_str()).unwrap_or("main");
            daemon.git_push(remote, branch).map(|_| "".to_string())
        }
        "pull" => {
            let remote = args.get(1).map(|s| s.as_str()).unwrap_or("origin");
            let branch = args.get(2).map(|s| s.as_str()).unwrap_or("main");
            daemon.git_pull(remote, branch).map(|_| "".to_string())
        }
        "status" => daemon.git_status(),
        "log" => {
            let max_count = args.get(1).and_then(|s| s.parse().ok());
            daemon.git_log(max_count)
        }
        "diff" => daemon.git_diff(),
        // Passthrough for other commands
        _ => {
            let cmd_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            daemon.with_git_cmd(&cmd_args)
        }
    };

    // Handle result
    match result {
        Ok(output) => {
            if !output.is_empty() {
                print!("{}", output);
            }
        }
        Err(e) => {
            eprintln!("Git operation failed: {:?}", e);
            process::exit(1);
        }
    }
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

/// Parse commit message from arguments (look for -m flag)
fn parse_commit_message(args: &[String]) -> String {
    // Look for -m or --message flag
    for (i, arg) in args.iter().enumerate() {
        if (arg == "-m" || arg == "--message") && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }

    // No -m flag found
    String::new()
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

/// Print help information
fn print_help() {
    println!("git-coordinated - Lockfree git wrapper for conflict-free operations");
    println!();
    println!("USAGE:");
    println!("    git-coordinated <command> [args...]");
    println!();
    println!("COMMANDS:");
    println!("    add <files...>           Stage files for commit");
    println!("    commit -m <message>      Commit staged changes");
    println!("    push [remote] [branch]   Push to remote (default: origin main)");
    println!("    pull [remote] [branch]   Pull from remote (default: origin main)");
    println!("    status                   Show repository status");
    println!("    log [n]                  Show commit log (optional limit)");
    println!("    diff                     Show differences");
    println!("    stats                    Show coordination statistics");
    println!("    <any-git-cmd>            Passthrough to git command");
    println!("    help, --help, -h         Show this help message");
    println!("    version, --version, -v   Show version");
    println!();
    println!("FEATURES:");
    println!("    - Lockfree coordination (<50ns overhead)");
    println!("    - No .git/index.lock conflicts");
    println!("    - Automatic stale lock recovery");
    println!("    - Q34 audit trail for compliance");
    println!("    - Detailed coordination statistics");
    println!();
    println!("EXAMPLES:");
    println!("    git-coordinated add src/*.rs");
    println!("    git-coordinated add . && git-coordinated commit -m 'Update sources'");
    println!("    git-coordinated push origin main");
    println!("    git-coordinated pull origin main");
    println!("    git-coordinated status");
    println!("    git-coordinated log 10");
    println!("    git-coordinated stats");
    println!();
    println!("INSTALLATION:");
    println!("    cargo install --path . --bin git-coordinated --features std,queue-bounded");
    println!();
    println!("DOCUMENTATION:");
    println!("    For more information, visit: https://kindly.dev");
}
