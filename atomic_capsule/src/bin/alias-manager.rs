//! # alias-manager - Universal Shell Alias Manager
//!
//! Multi-process-safe shell alias management using AliasCapsule (T6 Mixed: T0+T1+T9)
//!
//! ## Features
//! - Multi-shell support (Bash, Zsh, Fish)
//! - Atomic updates (no corruption)
//! - Multi-process coordination
//! - Command validation
//! - Q34 audit trail
//!
//! ## Usage
//! ```bash
//! # Add alias
//! alias-manager add g git-coordinated-v2
//!
//! # List all aliases
//! alias-manager list
//!
//! # Check if alias exists
//! alias-manager exists g
//!
//! # Get alias target
//! alias-manager get g
//!
//! # Remove alias
//! alias-manager remove g
//!
//! # Show help
//! alias-manager help
//! ```

use atomic_capsule::cli::{CliCapsule, CommandSpec};
use atomic_capsule::shell::AliasCapsule;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Build CLI parser
    let cli = CliCapsule::builder("alias-manager", "0.1.0")
        .about("Universal shell alias manager - Multi-process-safe alias management for Bash, Zsh, and Fish")
        .command(
            CommandSpec::new("add")
                .about("Add a new alias")
                .required_args(&["name", "command"])
        )
        .command(
            CommandSpec::new("remove")
                .about("Remove an alias")
                .required_args(&["name"])
        )
        .command(
            CommandSpec::new("list")
                .about("List all aliases")
        )
        .command(
            CommandSpec::new("exists")
                .about("Check if alias exists")
                .required_args(&["name"])
        )
        .command(
            CommandSpec::new("get")
                .about("Get alias target command")
                .required_args(&["name"])
        )
        .command(
            CommandSpec::new("stats")
                .about("Show coordinator statistics")
        )
        .build();

    // Parse command-line arguments (skip program name at argv[0])
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = cli.parse(&args)?;

    // Create AliasCapsule
    let aliases = AliasCapsule::new()?;

    // Execute command
    match parsed.command.as_str() {
        "add" => {
            let name = parsed.positional_args.get(0).ok_or("Missing alias name")?;
            let command = parsed.positional_args.get(1).ok_or("Missing command")?;

            aliases.add(name, command)?;
            println!("✓ Added alias: {} -> {}", name, command);
            println!("  Shell: {}", aliases.shell_type().as_str());
            println!("  Config: {}", aliases.config_path().display());
        }

        "remove" => {
            let name = parsed.positional_args.get(0).ok_or("Missing alias name")?;

            aliases.remove(name)?;
            println!("✓ Removed alias: {}", name);
        }

        "list" => {
            let alias_list = aliases.list()?;

            if alias_list.is_empty() {
                println!("No aliases configured.");
            } else {
                println!("Aliases ({} total):", alias_list.len());
                for alias in alias_list {
                    println!("  {} -> {}", alias.name, alias.command);
                }
            }
        }

        "exists" => {
            let name = parsed.positional_args.get(0).ok_or("Missing alias name")?;

            if aliases.exists(name) {
                println!("✓ Alias '{}' exists", name);
                if let Some(command) = aliases.get(name) {
                    println!("  -> {}", command);
                }
                std::process::exit(0);
            } else {
                println!("✗ Alias '{}' does not exist", name);
                std::process::exit(1);
            }
        }

        "get" => {
            let name = parsed.positional_args.get(0).ok_or("Missing alias name")?;

            if let Some(command) = aliases.get(name) {
                println!("{}", command);
            } else {
                eprintln!("Alias '{}' not found", name);
                std::process::exit(1);
            }
        }

        "stats" => {
            let stats = aliases.stats();

            println!("AliasCapsule Coordinator Statistics:");
            println!("  Lock acquires:     {}", stats.lock_acquires);
            println!("  Lock contentions:  {}", stats.lock_contentions);
            println!("  Stale recoveries:  {}", stats.lock_stale_recoveries);

            #[cfg(feature = "queue-bounded")]
            {
                println!("  Queue enqueues:    {}", stats.queue_enqueues);
                println!("  Queue dequeues:    {}", stats.queue_dequeues);
                println!("  Queue depth:       {}/{}", stats.queue_depth, stats.queue_capacity);
                println!("  Queue max depth:   {}", stats.queue_max_depth);
            }

            println!("  Audit entries:     {}", stats.audit_entries);
            println!("  Audit chain head:  0x{:016x}", stats.audit_chain_head);
        }

        "help" | _ => {
            println!("{}", cli.help());
        }
    }

    Ok(())
}
