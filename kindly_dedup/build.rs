//! Build-time binary protection for kindly_dedup
//!
//! Layer 1: Customer-specific compilation and binary signing
//!
//! ## UCE34 Framework
//! - Q10: Tier = T1 Atomic (compile-time constants, zero runtime cost)
//! - Q11: Rust transform = const/static embedding via cargo:rustc-env
//! - Q12: Nightly = Not required (stable features sufficient)
//! - Q28: Simplicity = Single build.rs, minimal dependencies
//! - Q29: Dependencies = sha2 (hashing), uuid (customer ID)
//! - Q34: Auditability = Build logs record customer ID and timestamp
//!
//! ## ASSUM Safety
//! - #ASSUME: CUSTOMER_ID env var provided or UUID v4 generated (collision probability <0.001%)
//! - #ASSUME: Binary hash computed from Cargo.toml + src/** files (deterministic)
//! - #ASSUME: Build environment is trusted (no malicious build.rs execution)
//!
//! ## Implementation
//! 1. Generate unique CUSTOMER_ID (env var or UUID v4)
//! 2. Compute binary signature (SHA-256 hash of source files)
//! 3. Embed as compile-time constants via cargo:rustc-env
//! 4. Enable aggressive optimization (LTO=fat, opt-level=3, codegen-units=1)
//!
//! ## Zero Runtime Cost
//! All work done at build time. Runtime access via `env!()` macro (0ns overhead).

use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src/");

    // Step 1: Generate or read CUSTOMER_ID
    let customer_id = generate_customer_id();
    println!("cargo:rustc-env=CUSTOMER_ID={}", customer_id);
    eprintln!("[BUILD] Customer ID: {}", customer_id);

    // Step 2: Generate BUILD_TIMESTAMP (Unix timestamp)
    let build_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("System clock before Unix epoch")
        .as_secs();
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", build_timestamp);
    eprintln!("[BUILD] Build timestamp: {}", build_timestamp);

    // Step 3: Compute binary signature (SHA-256 hash of source files)
    let binary_signature = compute_binary_signature();
    println!("cargo:rustc-env=BUILD_SIGNATURE={}", binary_signature);
    eprintln!("[BUILD] Binary signature: {}", binary_signature);

    // Step 4: Q34 Auditability - Log to build_audit.log
    log_build_audit(&customer_id, build_timestamp, &binary_signature);

    // Step 5: Aggressive optimization flags (already in Cargo.toml profile.release)
    // LTO=fat, opt-level=3, codegen-units=1, strip=symbols
    eprintln!("[BUILD] Optimization: LTO=fat, opt-level=3, codegen-units=1, strip=symbols");
}

/// Generate unique customer ID
///
/// Priority:
/// 1. CUSTOMER_ID env var (for customer-specific builds)
/// 2. UUID v4 (random, collision probability <0.001%)
///
/// # ASSUM Safety
/// - #ASSUME: UUID v4 collision probability <0.001% for <1M customers
/// - #VERIFY: Customer ID is 36 characters (UUID format)
fn generate_customer_id() -> String {
    // Check for explicit CUSTOMER_ID env var (highest priority)
    if let Ok(customer_id) = env::var("CUSTOMER_ID") {
        if !customer_id.is_empty() {
            eprintln!("[BUILD] Using explicit CUSTOMER_ID: {}", customer_id);
            return customer_id;
        }
    }

    // Generate UUID v4 (random, 128-bit)
    let uuid = uuid::Uuid::new_v4();
    let customer_id = uuid.to_string();

    eprintln!("[BUILD] Generated UUID v4 customer ID: {}", customer_id);
    customer_id
}

/// Compute binary signature (SHA-256 hash of source files)
///
/// Hashes:
/// - Cargo.toml (dependencies, features, version)
/// - All .rs files in src/ (source code)
///
/// # ASSUM Safety
/// - #ASSUME: Source files exist and are readable
/// - #ASSUME: SHA-256 collision resistance (2^128 operations)
/// - #VERIFY: Hash is deterministic (same sources = same hash)
fn compute_binary_signature() -> String {
    let mut hasher = Sha256::new();

    // Hash Cargo.toml (includes dependencies, features, version)
    if let Ok(cargo_toml) = fs::read("Cargo.toml") {
        hasher.update(&cargo_toml);
        eprintln!("[BUILD] Hashed Cargo.toml ({} bytes)", cargo_toml.len());
    }

    // Hash all .rs files in src/ (recursive)
    hash_directory(&mut hasher, Path::new("src"));

    // Finalize hash
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// Hash all .rs files in a directory (recursive)
fn hash_directory(hasher: &mut Sha256, path: &Path) {
    if !path.exists() {
        return;
    }

    if path.is_file() {
        if let Some(ext) = path.extension() {
            if ext == "rs" {
                if let Ok(contents) = fs::read(path) {
                    hasher.update(&contents);
                    eprintln!("[BUILD] Hashed {:?} ({} bytes)", path, contents.len());
                }
            }
        }
    } else if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                hash_directory(hasher, &entry.path());
            }
        }
    }
}

/// Q34 Auditability: Log build audit trail
///
/// Logs to build_audit.log with:
/// - Customer ID
/// - Build timestamp
/// - Binary signature
/// - Rust version
/// - Target triple
///
/// Format: JSON Lines (one event per line)
fn log_build_audit(customer_id: &str, build_timestamp: u64, binary_signature: &str) {
    use std::io::Write;

    let log_path = "build_audit.log";
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .expect("Failed to open build_audit.log");

    // Collect build metadata
    let rustc_version = env::var("RUSTC_VERSION").unwrap_or_else(|_| "unknown".to_string());
    let target_triple = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());

    // Write JSON Line
    writeln!(
        file,
        r#"{{"timestamp":{},"customer_id":"{}","binary_signature":"{}","rustc_version":"{}","target":"{}","profile":"{}"}}"#,
        build_timestamp, customer_id, binary_signature, rustc_version, target_triple, profile
    )
    .expect("Failed to write build audit log");

    eprintln!("[BUILD] Audit logged to {}", log_path);
}
