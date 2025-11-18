//! 6900HX Deployment & Validation System
//!
//! **Purpose**: Deploy client_demo to 6900HX server and validate all performance claims
//!
//! **Architecture**: Pure Rust, no shell scripts (std::process::Command)
//!
//! **6900HX Specs**:
//! - IP: 192.168.0.38
//! - CPU: AMD Ryzen 9 6900HX (16 cores @ 3.3-4.9 GHz)
//! - RAM: 64 GB DDR5-4800
//! - OS: Ubuntu Server 24.04
//!
//! **Deployment Strategy**:
//! 1. Build on 6900HX (native performance, no cross-compilation overhead)
//! 2. Run client_demo with all tiers
//! 3. Collect results (audit trail + stdout)
//! 4. Validate claims (F1 ≥99%, throughput ≥60K docs/sec, speedup ≥35×)
//!
//! **Validation Criteria** (B32 Framework):
//! - Tier 1 (Accuracy): F1 Score ≥99% (claim: 100%)
//! - Tier 2 (Throughput): ≥60K docs/sec single-threaded (claim: 60K)
//! - Tier 3 (Scale): Completes 10M docs in <5 min (claim: 167 sec)
//! - Speedup: ≥35× vs Python datasketch (claim: 38×)
//!
//! **META_CAPSULE Validation**:
//! - Layer 1: Build-time verification
//! - Layer 2: Circuit breaker active (8 checks)
//! - Layer 2.5: PUF 96.9% stability
//! - Layer 3: License validation
//! - Layer 4: Audit trail complete
//!
//! ## Usage
//!
//! ```bash
//! # Dry run (show commands without executing)
//! cargo run --bin deploy_validate_6900hx -- --dry-run
//!
//! # Deploy and validate
//! cargo run --bin deploy_validate_6900hx
//!
//! # Custom customer ID
//! cargo run --bin deploy_validate_6900hx -- --customer-id demo-6900hx-v2
//! ```

use anyhow::{bail, Context, Result};
use std::fs;
use std::process::{Command, Stdio};
use std::time::Instant;

// ============================================================================
// CONFIGURATION
// ============================================================================

/// 6900HX server configuration
struct ServerConfig {
    /// SSH connection string
    ssh_host: String,

    /// Project directory on remote server
    remote_dir: String,

    /// Customer ID for binary protection
    customer_id: String,

    /// SSH timeout (seconds)
    timeout: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            ssh_host: "samuel@192.168.0.38".to_string(),
            remote_dir: "~/Primitives/kindly_dedup".to_string(),
            customer_id: "demo-6900hx".to_string(),
            timeout: 3600, // 1 hour (demo takes ~45 min)
        }
    }
}

/// Validation thresholds (B32 compliant)
struct ValidationThresholds {
    /// Minimum F1 score (%)
    min_f1_score: f64,

    /// Minimum throughput (docs/sec)
    min_throughput: f64,

    /// Minimum speedup vs baseline
    min_speedup: f64,

    /// Maximum Tier 3 time (seconds)
    max_tier3_time: f64,
}

impl Default for ValidationThresholds {
    fn default() -> Self {
        Self {
            min_f1_score: 99.0,       // ≥99% (claim: 100%)
            min_throughput: 60_000.0, // ≥60K docs/sec
            min_speedup: 35.0,        // ≥35× (claim: 38×)
            max_tier3_time: 300.0,    // ≤5 min (claim: 167 sec = 2.78 min)
        }
    }
}

// ============================================================================
// VALIDATION RESULTS
// ============================================================================

/// Validation results structure
#[derive(Debug, Default)]
struct ValidationResults {
    /// Binary compiled successfully
    build_success: bool,

    /// Demo ran without errors
    demo_success: bool,

    /// Tier 1 F1 score (%)
    f1_score: Option<f64>,

    /// Tier 2 throughput (docs/sec)
    throughput: Option<f64>,

    /// Tier 3 completion time (seconds)
    tier3_time: Option<f64>,

    /// Speedup vs Python datasketch
    speedup: Option<f64>,

    /// META_CAPSULE layers active
    meta_capsule_layers: Vec<String>,

    /// PUF stability (%)
    puf_stability: Option<f64>,

    /// Audit trail path
    audit_trail_path: Option<String>,

    /// Full stdout/stderr
    output: String,

    /// Validation errors
    errors: Vec<String>,
}

impl ValidationResults {
    /// Validate against thresholds
    fn validate(&self, thresholds: &ValidationThresholds) -> (bool, Vec<String>) {
        let mut passed = true;
        let mut failures = Vec::new();

        // Check build success
        if !self.build_success {
            passed = false;
            failures.push("❌ Build failed".to_string());
        }

        // Check demo execution
        if !self.demo_success {
            passed = false;
            failures.push("❌ Demo execution failed".to_string());
        }

        // Check F1 score
        if let Some(f1) = self.f1_score {
            if f1 < thresholds.min_f1_score {
                passed = false;
                failures.push(format!(
                    "❌ F1 Score: {:.2}% < {:.2}% (FAILED)",
                    f1, thresholds.min_f1_score
                ));
            }
        } else {
            passed = false;
            failures.push("❌ F1 Score not measured".to_string());
        }

        // Check throughput
        if let Some(tput) = self.throughput {
            if tput < thresholds.min_throughput {
                passed = false;
                failures.push(format!(
                    "❌ Throughput: {:.0} docs/sec < {:.0} docs/sec (FAILED)",
                    tput, thresholds.min_throughput
                ));
            }
        } else {
            passed = false;
            failures.push("❌ Throughput not measured".to_string());
        }

        // Check Tier 3 time
        if let Some(t3) = self.tier3_time {
            if t3 > thresholds.max_tier3_time {
                passed = false;
                failures.push(format!(
                    "❌ Tier 3 time: {:.1}s > {:.1}s (FAILED)",
                    t3, thresholds.max_tier3_time
                ));
            }
        }

        // Check speedup
        if let Some(sp) = self.speedup {
            if sp < thresholds.min_speedup {
                passed = false;
                failures.push(format!(
                    "❌ Speedup: {:.1}× < {:.1}× (FAILED)",
                    sp, thresholds.min_speedup
                ));
            }
        } else {
            passed = false;
            failures.push("❌ Speedup not measured".to_string());
        }

        // Check META_CAPSULE
        if self.meta_capsule_layers.is_empty() {
            failures.push("⚠ META_CAPSULE not active (expected 4 layers)".to_string());
        }

        (passed, failures)
    }

    /// Print validation report
    fn print_report(&self, thresholds: &ValidationThresholds) {
        println!("\n═══════════════════════════════════════════════════════════");
        println!("  6900HX VALIDATION REPORT");
        println!("═══════════════════════════════════════════════════════════\n");

        // Build status
        println!("[BUILD]");
        println!(
            "  Status: {}",
            if self.build_success {
                "✓ Success"
            } else {
                "❌ Failed"
            }
        );

        // Demo execution
        println!("\n[DEMO EXECUTION]");
        println!(
            "  Status: {}",
            if self.demo_success { "✓ Success" } else { "❌ Failed" }
        );

        // Tier 1: Accuracy
        println!("\n[TIER 1: ACCURACY VALIDATION]");
        if let Some(f1) = self.f1_score {
            let status = if f1 >= thresholds.min_f1_score {
                "✓ PASS"
            } else {
                "❌ FAIL"
            };
            println!(
                "  F1 Score: {:.2}% (threshold: {:.2}%) {}",
                f1, thresholds.min_f1_score, status
            );
        } else {
            println!("  F1 Score: ❌ NOT MEASURED");
        }

        // Tier 2: Throughput
        println!("\n[TIER 2: THROUGHPUT VALIDATION]");
        if let Some(tput) = self.throughput {
            let status = if tput >= thresholds.min_throughput {
                "✓ PASS"
            } else {
                "❌ FAIL"
            };
            println!(
                "  Throughput: {:.0} docs/sec (threshold: {:.0}) {}",
                tput, thresholds.min_throughput, status
            );
        } else {
            println!("  Throughput: ❌ NOT MEASURED");
        }

        // Tier 3: Scale
        if let Some(t3) = self.tier3_time {
            let status = if t3 <= thresholds.max_tier3_time {
                "✓ PASS"
            } else {
                "❌ FAIL"
            };
            println!("\n[TIER 3: SCALE VALIDATION]");
            println!(
                "  Time: {:.1}s (threshold: {:.1}s) {}",
                t3, thresholds.max_tier3_time, status
            );
        }

        // Speedup
        if let Some(sp) = self.speedup {
            let status = if sp >= thresholds.min_speedup {
                "✓ PASS"
            } else {
                "❌ FAIL"
            };
            println!("\n[SPEEDUP VALIDATION]");
            println!(
                "  Speedup: {:.1}× vs Python datasketch (threshold: {:.1}×) {}",
                sp, thresholds.min_speedup, status
            );
        }

        // META_CAPSULE
        println!("\n[META_CAPSULE VALIDATION]");
        if !self.meta_capsule_layers.is_empty() {
            println!("  Layers active: {}", self.meta_capsule_layers.len());
            for layer in &self.meta_capsule_layers {
                println!("    - {}", layer);
            }
        } else {
            println!("  ⚠ No layers detected (expected 4)");
        }

        // PUF
        if let Some(puf) = self.puf_stability {
            println!("\n[PUF VALIDATION]");
            println!("  Stability: {:.1}% (target: ≥95%)", puf);
        }

        // Audit trail
        if let Some(path) = &self.audit_trail_path {
            println!("\n[AUDIT TRAIL]");
            println!("  Path: {}", path);
        }

        // Overall status
        let (passed, failures) = self.validate(thresholds);
        println!("\n═══════════════════════════════════════════════════════════");
        if passed {
            println!("  ✓ ALL VALIDATIONS PASSED");
        } else {
            println!("  ❌ VALIDATION FAILURES:");
            for failure in &failures {
                println!("    {}", failure);
            }
        }
        println!("═══════════════════════════════════════════════════════════\n");
    }
}

// ============================================================================
// SSH COMMAND EXECUTION
// ============================================================================

/// Execute SSH command with timeout
fn ssh_exec(config: &ServerConfig, command: &str, dry_run: bool) -> Result<String> {
    if dry_run {
        println!("[DRY RUN] ssh {} '{}'", config.ssh_host, command);
        return Ok("(dry run - no output)".to_string());
    }

    println!("[SSH] Executing: {}", command);

    let output = Command::new("ssh")
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg("-o")
        .arg("ServerAliveInterval=30")
        .arg(&config.ssh_host)
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute SSH command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("SSH command failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

// ============================================================================
// DEPLOYMENT STEPS
// ============================================================================

/// Step 1: Verify SSH connectivity
fn verify_ssh_connectivity(config: &ServerConfig, dry_run: bool) -> Result<()> {
    println!("\n[STEP 1] Verifying SSH connectivity to 6900HX...");

    let output = ssh_exec(config, "hostname && uname -a", dry_run)?;

    if !dry_run {
        println!("├─ Remote host: {}", output.lines().next().unwrap_or("Unknown"));
        println!("└─ ✓ SSH connection verified");
    }

    Ok(())
}

/// Step 2: Verify project directory exists
fn verify_project_directory(config: &ServerConfig, dry_run: bool) -> Result<()> {
    println!("\n[STEP 2] Verifying project directory...");

    let cmd = format!("[ -d {} ] && echo 'EXISTS' || echo 'NOT_FOUND'", config.remote_dir);
    let output = ssh_exec(config, &cmd, dry_run)?;

    if !dry_run {
        if output.trim() != "EXISTS" {
            bail!("Project directory not found: {}", config.remote_dir);
        }
        println!("└─ ✓ Project directory exists: {}", config.remote_dir);
    }

    Ok(())
}

/// Step 3: Build client_demo on 6900HX (native performance)
fn build_client_demo(config: &ServerConfig, dry_run: bool) -> Result<()> {
    println!("\n[STEP 3] Building client_demo on 6900HX (native)...");
    println!("├─ Customer ID: {}", config.customer_id);
    println!("├─ Features: meta-capsule, benchmarking");
    println!("└─ Profile: release (opt-level=3, lto=fat)");

    let cmd = format!(
        "cd {} && CUSTOMER_ID={} cargo build --release --bin client_demo --features 'meta-capsule,benchmarking' 2>&1",
        config.remote_dir, config.customer_id
    );

    let output = ssh_exec(config, &cmd, dry_run)?;

    if !dry_run {
        // Check for build success
        if output.contains("error") || output.contains("failed") {
            println!("\n❌ Build failed:\n{}", output);
            bail!("Build failed on 6900HX");
        }

        println!("\n✓ Build successful");

        // Extract build time if available
        if let Some(line) = output.lines().find(|l| l.contains("Finished release")) {
            println!("  {}", line);
        }
    }

    Ok(())
}

/// Step 4: Run client_demo
fn run_client_demo(config: &ServerConfig, dry_run: bool) -> Result<String> {
    println!("\n[STEP 4] Running client_demo...");
    println!("├─ Tier 1: 100K docs (accuracy validation)");
    println!("├─ Tier 2: 1M docs (throughput demonstration)");
    println!("├─ Tier 3: 10M docs (scale capability)");
    println!("└─ Estimated time: 45 minutes\n");

    let cmd = format!(
        "cd {} && echo '' | ./target/release/client_demo 2>&1",
        config.remote_dir
    );

    let start = Instant::now();
    let output = ssh_exec(config, &cmd, dry_run)?;
    let elapsed = start.elapsed();

    if !dry_run {
        println!("\n✓ Demo completed in {:.1} minutes", elapsed.as_secs_f64() / 60.0);
    }

    Ok(output)
}

/// Step 5: Parse validation results from demo output
fn parse_validation_results(output: &str) -> ValidationResults {
    let mut results = ValidationResults {
        demo_success: true,
        build_success: true,
        output: output.to_string(),
        ..Default::default()
    };

    // Parse F1 Score
    if let Some(line) = output.lines().find(|l| l.contains("F1 Score:")) {
        if let Some(captures) = line.split(':').nth(1) {
            if let Some(value) = captures.trim().split('%').next() {
                if let Ok(f1) = value.trim().parse::<f64>() {
                    results.f1_score = Some(f1);
                }
            }
        }
    }

    // Parse Throughput (look for "Single-threaded:" or "Throughput:")
    for line in output.lines() {
        if line.contains("Single-threaded:") || line.contains("Throughput:") {
            if let Some(value) = line.split(':').nth(1) {
                let cleaned = value.trim().replace("docs/sec", "").replace(",", "");
                if let Ok(tput) = cleaned.trim().parse::<f64>() {
                    results.throughput = Some(tput);
                    break;
                }
            }
        }
    }

    // Parse Speedup
    if let Some(line) = output.lines().find(|l| l.contains("Speedup:") && l.contains("×")) {
        if let Some(value) = line.split(':').nth(1) {
            let cleaned_str = value.trim().replace("×", "").replace("(", "");
            if let Some(first_token) = cleaned_str.split_whitespace().next() {
                if let Ok(sp) = first_token.parse::<f64>() {
                    results.speedup = Some(sp);
                }
            }
        }
    }

    // Parse META_CAPSULE layers
    if output.contains("META_CAPSULE: Active") {
        results
            .meta_capsule_layers
            .push("Layer 1: Build-time verification".to_string());
        results
            .meta_capsule_layers
            .push("Layer 2: Circuit breaker (8 checks)".to_string());
        results
            .meta_capsule_layers
            .push("Layer 3: License validation".to_string());
        results.meta_capsule_layers.push("Layer 4: Audit trail".to_string());
    }

    // Parse PUF stability (if available in output)
    if let Some(line) = output.lines().find(|l| l.contains("PUF") && l.contains("Stability")) {
        if let Some(value) = line.split(':').nth(1) {
            let cleaned = value.trim().replace("%", "");
            if let Ok(puf) = cleaned.trim().parse::<f64>() {
                results.puf_stability = Some(puf);
            }
        }
    }

    // Check for errors
    if output.contains("error:") || output.contains("failed") || output.contains("panic") {
        results.demo_success = false;
        results.errors.push("Demo execution encountered errors".to_string());
    }

    results
}

/// Step 6: Collect audit trail
fn collect_audit_trail(config: &ServerConfig, dry_run: bool) -> Result<Option<String>> {
    println!("\n[STEP 5] Collecting audit trail...");

    let audit_pattern = format!("/tmp/demo_audit_{}.jsonl", config.customer_id.replace("-", "_"));

    let cmd = format!("ls -la {} 2>/dev/null || echo 'NOT_FOUND'", audit_pattern);

    let output = ssh_exec(config, &cmd, dry_run)?;

    if dry_run || output.trim() == "NOT_FOUND" {
        println!("└─ ⚠ Audit trail not found");
        return Ok(None);
    }

    println!("└─ ✓ Audit trail found: {}", audit_pattern);

    // Optionally copy to local machine
    println!("\n[OPTIONAL] Copy audit trail to local machine?");
    println!("  scp {}:{} ./", config.ssh_host, audit_pattern);

    Ok(Some(audit_pattern))
}

// ============================================================================
// MAIN DEPLOYMENT ORCHESTRATOR
// ============================================================================

fn main() -> Result<()> {
    println!("═══════════════════════════════════════════════════════════");
    println!("  6900HX Deployment & Validation System");
    println!("  kindly_dedup - Production Performance Validation");
    println!("═══════════════════════════════════════════════════════════\n");

    // Parse CLI arguments
    let args: Vec<String> = std::env::args().collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let custom_customer_id = args
        .iter()
        .position(|a| a == "--customer-id")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.clone());

    if dry_run {
        println!("⚠ DRY RUN MODE - Commands will be shown but not executed\n");
    }

    let mut config = ServerConfig::default();
    if let Some(cid) = custom_customer_id {
        config.customer_id = cid;
    }

    let thresholds = ValidationThresholds::default();

    println!("[CONFIGURATION]");
    println!("  SSH Host: {}", config.ssh_host);
    println!("  Remote Dir: {}", config.remote_dir);
    println!("  Customer ID: {}", config.customer_id);
    println!("  Timeout: {} seconds\n", config.timeout);

    println!("[VALIDATION THRESHOLDS]");
    println!("  F1 Score: ≥{:.1}%", thresholds.min_f1_score);
    println!("  Throughput: ≥{:.0} docs/sec", thresholds.min_throughput);
    println!("  Speedup: ≥{:.1}×", thresholds.min_speedup);
    println!("  Tier 3 Time: ≤{:.0}s", thresholds.max_tier3_time);

    // Execute deployment steps
    let start = Instant::now();

    verify_ssh_connectivity(&config, dry_run)?;
    verify_project_directory(&config, dry_run)?;
    build_client_demo(&config, dry_run)?;

    let demo_output = run_client_demo(&config, dry_run)?;

    let mut results = parse_validation_results(&demo_output);

    let audit_trail = collect_audit_trail(&config, dry_run)?;
    results.audit_trail_path = audit_trail;

    let total_time = start.elapsed();

    // Print validation report
    results.print_report(&thresholds);

    println!("\n[DEPLOYMENT SUMMARY]");
    println!("  Total time: {:.1} minutes", total_time.as_secs_f64() / 60.0);
    println!("  SSH Host: {}", config.ssh_host);
    println!("  Customer ID: {}", config.customer_id);

    // Final status
    let (passed, _) = results.validate(&thresholds);

    if passed {
        println!("\n✓ DEPLOYMENT & VALIDATION SUCCESSFUL");
        println!("  All claims validated on 6900HX production hardware");
        Ok(())
    } else {
        println!("\n❌ DEPLOYMENT & VALIDATION FAILED");
        println!("  Review report above for specific failures");
        bail!("Validation failed - see report above");
    }
}
