//! Command-line tool for fixing padding fields in Rust code.

use atomic_capsule_tools::fix_padding_recursive;
use std::env;
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <directory>", args[0]);
        eprintln!("Scans directory recursively and reports padding field issues.");
        process::exit(1);
    }

    let dir = Path::new(&args[1]);

    if !dir.exists() {
        eprintln!("Error: Directory does not exist: {}", dir.display());
        process::exit(1);
    }

    println!("Scanning {} for padding field issues...", dir.display());

    match fix_padding_recursive(dir) {
        Ok(results) => {
            if results.is_empty() {
                println!("✓ No padding field issues found.");
            } else {
                println!("\nFound {} files with padding issues:\n", results.len());
                for (path, result) in &results {
                    println!("  {} (changed: {})", path.display(), result.changed);
                }

                println!("\nTo fix these issues, run with --fix flag (not yet implemented).");
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}
