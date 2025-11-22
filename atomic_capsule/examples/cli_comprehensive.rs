//! Comprehensive CliCapsule Example
//!
//! Demonstrates all features of the CliCapsule:
//! - Phase 1: Command structure with flags and positional args
//! - Phase 2: Default values for flags
//! - Phase 3: Validators for flag values
//! - Phase 4: Global flags (infrastructure in place)
//! - Phase 5: Environment variable support
//!
//! This example shows a realistic deduplication tool with:
//! - Multiple commands (process, status, validate)
//! - Various flag types (required, optional, with defaults)
//! - Input validation (thresholds, paths, integers)
//! - Environment variable fallback configuration
//!
//! # Run examples:
//!
//! ```bash
//! # Basic usage
//! cargo run --example cli_comprehensive -- process input.txt
//!
//! # With flags
//! cargo run --example cli_comprehensive -- process input.txt --threshold 0.9 --threads 4
//!
//! # Use defaults
//! cargo run --example cli_comprehensive -- process input.txt
//!
//! # Validation errors
//! cargo run --example cli_comprehensive -- process input.txt --threshold 1.5
//! cargo run --example cli_comprehensive -- process input.txt --threads -1
//!
//! # Status command
//! cargo run --example cli_comprehensive -- status
//! cargo run --example cli_comprehensive -- status --format csv
//!
//! # Help
//! cargo run --example cli_comprehensive -- --help
//! cargo run --example cli_comprehensive -- --version
//! ```

use atomic_capsule::cli::{CliCapsule, CommandSpec, validators};

fn main() {
    // Build CLI with all features
    let cli = CliCapsule::builder("kindly-dedup", "2.0.0")
        .about("High-performance LLM dataset deduplication using computational capsules")

        // ============================================================================
        // COMMAND 1: process
        // Demonstrates: required args, optional flags, defaults, validators, env vars
        // ============================================================================
        .command(
            CommandSpec::new("process")
                .about("Process and deduplicate a document corpus")

                // Positional argument
                .required_args(&["input"])

                // Phase 2: Flags with defaults (output format)
                .flag("--format", "Output format (json|csv|text)")
                .default_value("--format", "json")

                // Phase 2 + Phase 3: Threshold with default and validator
                .flag("--threshold", "Jaccard similarity threshold [0.0-1.0]")
                .default_value("--threshold", "0.85")
                .validator("--threshold", validators::range_0_1)

                // Phase 2 + Phase 3: Thread count with default and validator
                .flag("--threads", "Number of threads (0=auto)")
                .default_value("--threads", "0")
                .validator("--threads", validators::non_negative_int)

                // Phase 5: Environment variable support
                // Output file can come from CLI, DEDUP_OUTPUT env var, or be omitted
                .flag("--output", "Output file (optional, env: DEDUP_OUTPUT)")
                .env_mapping("--output", "DEDUP_OUTPUT")

                // Phase 3: Required flag with validator
                .required_flag("--input-format", "Input format (json|csv|text)")
                .validator("--input-format", validate_format)

                // Additional optional flags
                .flag("--max-docs", "Maximum documents to process")
                .default_value("--max-docs", "0")
                .validator("--max-docs", validators::non_negative_int)

                .flag("--bloom-bits", "Bloom filter bits per document")
                .default_value("--bloom-bits", "16")
                .validator("--bloom-bits", validators::positive_int)

                .flag("--lsh-tables", "Number of LSH hash tables")
                .default_value("--lsh-tables", "5")
                .validator("--lsh-tables", validators::positive_int)

                .flag("--verbose", "Verbose output (boolean flag)")
        )

        // ============================================================================
        // COMMAND 2: status
        // Demonstrates: simpler command with optional flags and defaults
        // ============================================================================
        .command(
            CommandSpec::new("status")
                .about("Show deduplication job status")

                .flag("--format", "Output format (json|csv|text)")
                .default_value("--format", "text")

                .flag("--job-id", "Specific job ID to check (optional)")

                .flag("--full", "Show detailed status (boolean flag)")
        )

        // ============================================================================
        // COMMAND 3: validate
        // Demonstrates: validation of existing data
        // ============================================================================
        .command(
            CommandSpec::new("validate")
                .about("Validate deduplication results")

                .required_args(&["results_file"])

                .flag("--sample-size", "Sample size for validation")
                .default_value("--sample-size", "1000")
                .validator("--sample-size", validators::positive_int)

                .flag("--confidence", "Confidence level [0.0-1.0]")
                .default_value("--confidence", "0.95")
                .validator("--confidence", validators::range_0_1)

                .flag("--output", "Output report file (env: VALIDATION_OUTPUT)")
                .env_mapping("--output", "VALIDATION_OUTPUT")
        )

        // ============================================================================
        // COMMAND 4: demo
        // Demonstrates: simple command with minimal configuration
        // ============================================================================
        .command(
            CommandSpec::new("demo")
                .about("Run interactive demo")

                .flag("--dataset-size", "Demo dataset size (small|medium|large)")
                .default_value("--dataset-size", "small")
        )

        .build();

    // Parse arguments
    let args: Vec<String> = std::env::args().skip(1).collect();

    match cli.parse(&args) {
        Ok(parsed) => {
            println!("╔════════════════════════════════════════════════════════╗");
            println!("║         Kindly Dedup CLI - Phase 2-5 Demo             ║");
            println!("╚════════════════════════════════════════════════════════╝\n");

            println!("Command: {}", parsed.command);
            println!();

            // Show parsed values
            if !parsed.flags.is_empty() {
                println!("┌─ Flags ─────────────────────────────────────────┐");
                for (flag, value) in &parsed.flags {
                    if value.is_empty() {
                        println!("│ {} = <boolean flag>", flag);
                    } else {
                        println!("│ {} = {}", flag, value);
                    }
                }
                println!("└──────────────────────────────────────────────────┘\n");
            }

            // Show positional args
            if !parsed.positional_args.is_empty() {
                println!("┌─ Arguments ─────────────────────────────────────┐");
                for (i, arg) in parsed.positional_args.iter().enumerate() {
                    println!("│ [{}] = {}", i, arg);
                }
                println!("└──────────────────────────────────────────────────┘\n");
            }

            // Command-specific handling
            match parsed.command.as_str() {
                "process" => {
                    println!("✓ Processing deduplication task");
                    println!();

                    let input = parsed.positional_args.first().unwrap();
                    let format = parsed.get_flag("--format").unwrap_or("json");
                    let threshold = parsed.get_flag("--threshold").unwrap_or("0.85");
                    let threads = parsed.get_flag("--threads").unwrap_or("0");
                    let input_format = parsed.get_flag("--input-format").unwrap_or("json");

                    println!("  Input file: {}", input);
                    println!("  Input format: {}", input_format);
                    println!("  Output format: {}", format);
                    println!("  Threshold: {}", threshold);
                    println!("  Threads: {}", threads);

                    if let Some(output) = parsed.get_flag("--output") {
                        println!("  Output file: {}", output);
                    }

                    if parsed.has_flag("--verbose") {
                        println!("  Verbose: ON");
                    }

                    let max_docs = parsed.get_flag("--max-docs").unwrap_or("0");
                    let bloom = parsed.get_flag("--bloom-bits").unwrap_or("16");
                    let lsh = parsed.get_flag("--lsh-tables").unwrap_or("5");

                    println!("  Max docs: {}", max_docs);
                    println!("  Bloom bits: {}", bloom);
                    println!("  LSH tables: {}", lsh);

                    println!();
                    println!("✓ Configuration validated successfully");
                }

                "status" => {
                    println!("✓ Checking deduplication status");
                    println!();

                    let format = parsed.get_flag("--format").unwrap_or("text");

                    println!("  Output format: {}", format);

                    if let Some(job_id) = parsed.get_flag("--job-id") {
                        println!("  Job ID: {}", job_id);
                    } else {
                        println!("  Job ID: <showing all jobs>");
                    }

                    if parsed.has_flag("--full") {
                        println!("  Detailed: ON");
                    }

                    println!();
                    println!("Status (example):");
                    match format {
                        "json" => {
                            println!("  {{\n    \"status\": \"running\",\n    \"processed\": 12345,\n    \"duplicates\": 3456\n  }}");
                        }
                        "csv" => {
                            println!("  status,processed,duplicates");
                            println!("  running,12345,3456");
                        }
                        _ => {
                            println!("  Status: running");
                            println!("  Processed: 12345 documents");
                            println!("  Duplicates found: 3456");
                        }
                    }
                }

                "validate" => {
                    println!("✓ Validating deduplication results");
                    println!();

                    let results = parsed.positional_args.first().unwrap();
                    let sample = parsed.get_flag("--sample-size").unwrap_or("1000");
                    let confidence = parsed.get_flag("--confidence").unwrap_or("0.95");

                    println!("  Results file: {}", results);
                    println!("  Sample size: {}", sample);
                    println!("  Confidence level: {}", confidence);

                    if let Some(output) = parsed.get_flag("--output") {
                        println!("  Report output: {}", output);
                    }

                    println!();
                    println!("Validation Results (example):");
                    println!("  Accuracy: 98.7%");
                    println!("  Precision: 97.2%");
                    println!("  Recall: 99.1%");
                }

                "demo" => {
                    println!("✓ Running interactive demo");
                    println!();

                    let size = parsed.get_flag("--dataset-size").unwrap_or("small");

                    println!("  Dataset size: {}", size);

                    let docs = match size {
                        "medium" => 100_000,
                        "large" => 1_000_000,
                        _ => 10_000, // small
                    };

                    println!();
                    println!("Demo Configuration:");
                    println!("  Test documents: {}", docs);
                    println!("  Features: SIMD MinHash, Bloom filter, LSH");
                    println!("  Expected throughput: {} docs/sec", docs / 10);
                }

                _ => {
                    println!("Unknown command: {}", parsed.command);
                }
            }

            println!();
            println!("╔════════════════════════════════════════════════════════╗");
            println!("║✓ All validations passed successfully                   ║");
            println!("╚════════════════════════════════════════════════════════╝");
        }

        Err(e) => {
            eprintln!("╔════════════════════════════════════════════════════════╗");
            eprintln!("║✗ CLI Error                                             ║");
            eprintln!("╚════════════════════════════════════════════════════════╝\n");
            eprintln!("✗ Error: {}", e);
            eprintln!();
            eprintln!("Try running with --help for usage information:");
            eprintln!("  cargo run --example cli_comprehensive -- --help");
            std::process::exit(1);
        }
    }
}

/// Custom validator for format strings
fn validate_format(s: &str) -> Result<String, String> {
    match s {
        "json" | "csv" | "text" => Ok(s.to_string()),
        _ => Err(format!("Invalid format '{}'. Expected: json, csv, or text", s)),
    }
}
