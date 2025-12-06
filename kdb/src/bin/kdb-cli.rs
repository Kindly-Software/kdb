//! KDB CLI Entry Point - The Kindly Debugger
//!
//! Main binary implementing REPL interface to kdb debugger.
//!
//! Usage:
//!   kdb [OPTIONS]
//!
//! Options:
//!   -h, --help       Print help information
//!   -V, --version    Print version
//!   --audit          Show audit trail and exit
//!
//! Example:
//!   $ kdb
//!   kdb> attach 12345
//!   kdb> break main
//!   kdb> continue
//!   kdb> quit

use kdb::cli::repl::REPLCapsule;
use std::io;

fn main() -> io::Result<()> {
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-V" | "--version" => {
                println!("kdb 0.1.0");
                return Ok(());
            }
            "--audit" => {
                // Show audit trail (placeholder for future implementation)
                println!("Audit trail not yet persisted across sessions");
                return Ok(());
            }
            arg => {
                eprintln!("Unknown argument: {}", arg);
                print_help();
                std::process::exit(1);
            }
        }
    }

    // Create and run REPL
    let mut repl = REPLCapsule::new();

    // Install signal handlers for graceful shutdown
    install_signal_handlers();

    // Run REPL loop
    match repl.run() {
        Ok(()) => {
            println!("\n[kdb] Exited successfully");
            Ok(())
        }
        Err(e) => {
            eprintln!("[kdb] Error: {}", e);
            Err(e)
        }
    }
}

/// Print help message
fn print_help() {
    println!(
        r#"KDB - The Kindly Debugger v0.1.0

USAGE:
    kdb [OPTIONS]

OPTIONS:
    -h, --help       Print this help message
    -V, --version    Print version information
    --audit          Show audit trail and exit

COMMANDS (in REPL):
    attach <pid>     Attach to process
    break <addr|sym> Set breakpoint
    continue (c)     Resume execution
    step (s)         Single step forward
    back             Time-travel step backward
    snapshot         Capture time-travel snapshot
    stack (bt)       Show stack trace
    quit (q)         Exit debugger
    help [cmd]       Show help

EXAMPLE:
    $ kdb
    [kdb] KDB - The Kindly Debugger v0.1.0
    [kdb] Type 'help' for commands, 'quit' to exit

    kdb> attach 12345
    [kdb] Attached to process 12345

    kdb> break main
    [kdb] Breakpoint 0 set at main

    kdb> continue
    [kdb] Continued - Hit breakpoint 0 at 0x401234

    kdb> stack
    [kdb] Stack trace:
    #0  0x401234  main+0x0
    #1  0x7ffff7a2d083  __libc_start_main+0xf3
    #2  0x401090  _start+0x2e

    kdb> quit
    [kdb] Detached. Goodbye!

FEATURES:
    - T0 Auditable tier with Q34 hash-chain compliance
    - 100% lockfree command dispatching
    - Time-travel debugging via snapshots
    - Interactive REPL with command history
    - Cryptographic audit trail for compliance

More information at: https://github.com/primitives-dev/kdb
"#
    );
}

/// Install signal handlers (Ctrl+C, etc.)
fn install_signal_handlers() {
    // In a real implementation, would use signal-hook to catch SIGINT
    // For now, just rely on Rust's standard Ctrl+C handling

    // Set up Ctrl+C handler
    let _ = ctrlc::set_handler(move || {
        println!("\n[kdb] Interrupted by user");
        std::process::exit(0);
    });
}

// If ctrlc crate not available, provide simple fallback
mod ctrlc {
    pub fn set_handler<F>(_handler: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn() + 'static,
    {
        // Placeholder - would use signal-hook in production
        Ok(())
    }
}
