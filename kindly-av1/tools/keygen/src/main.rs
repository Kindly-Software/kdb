//! Ed25519 Keypair Generator for kindly-av1 License Signing
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! # Purpose
//!
//! Generates Ed25519 keypair for offline license validation:
//! - Private key: Used by activation server to sign licenses (KEEP SECRET)
//! - Public key: Embedded in kindly-av1 binary for offline verification
//!
//! # Usage
//!
//! ```bash
//! cd /home/samuel/Primitives/kindly-av1/tools/keygen
//! cargo run --release
//! ```
//!
//! # Output
//!
//! - `../../keys/signing_key.bin` - 32-byte private key (SERVER ONLY)
//! - `../../keys/public_key.bin` - 32-byte public key (embedded in binary)
//!
//! # Security
//!
//! - Private key MUST be stored securely (HSM, encrypted disk, etc.)
//! - Private key NEVER committed to git
//! - Public key safe to distribute (embedded in binary)
//!
//! # Implementation
//!
//! Based on SOTA ed25519-dalek 2.1 API:
//! - https://docs.rs/ed25519-dalek
//! - Uses OsRng for cryptographically secure random generation
//! - SigningKey/VerifyingKey API (not deprecated Keypair)
//!
//! # Framework Compliance
//!
//! - UCE34 Q11: 100% Rust implementation
//! - Chaos: Standalone tool, no capsule needed (one-shot generation)
//! - ASSUM: OsRng is cryptographically secure (#ASSUME_SECURE_RNG)

use ed25519_dalek::{SigningKey, VerifyingKey, Signer, Verifier, Signature};
use rand::rngs::OsRng;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

/// Byzantine Royal Purple for CLI branding
const PURPLE: &str = "\x1b[38;2;155;89;182m";
/// Golden Spark for highlights
const GOLD: &str = "\x1b[38;2;241;196;15m";
/// Reset color
const RESET: &str = "\x1b[0m";

fn main() {
    println!("\n{}=== kindly-av1 Ed25519 Keygen ==={}", PURPLE, RESET);
    println!("{}[TRADE SECRET]{} License signing key generator\n", GOLD, RESET);

    // Determine output directory (relative to tool location)
    let keys_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("keys"))
        .expect("Failed to determine keys directory");

    println!("Output directory: {}\n", keys_dir.display());

    // Create keys directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&keys_dir) {
        eprintln!("{}ERROR:{} Failed to create keys directory: {}", GOLD, RESET, e);
        std::process::exit(1);
    }

    // Check if keys already exist
    let signing_key_path = keys_dir.join("signing_key.bin");
    let public_key_path = keys_dir.join("public_key.bin");

    if signing_key_path.exists() || public_key_path.exists() {
        println!("{}WARNING:{} Keys already exist!", GOLD, RESET);
        println!("  Signing key: {}", signing_key_path.display());
        println!("  Public key:  {}", public_key_path.display());
        println!("\nTo regenerate, delete existing keys first.");
        println!("{}CAUTION:{} Regenerating keys will invalidate ALL existing licenses!\n", GOLD, RESET);

        // Ask for confirmation
        print!("{}Overwrite existing keys? [y/N]:{} ", PURPLE, RESET);
        std::io::stdout().flush().unwrap();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            std::process::exit(0);
        }
        println!();
    }

    // Generate Ed25519 keypair using OsRng (cryptographically secure)
    // #ASSUME_SECURE_RNG: OsRng provides cryptographically secure randomness
    // #VERIFY: OsRng uses platform-specific CSPRNG (getrandom on Linux)
    println!("Generating Ed25519 keypair...");
    let mut csprng = OsRng;
    let signing_key: SigningKey = SigningKey::generate(&mut csprng);
    let verifying_key: VerifyingKey = signing_key.verifying_key();

    // Extract raw bytes
    let signing_key_bytes: [u8; 32] = signing_key.to_bytes();
    let verifying_key_bytes: [u8; 32] = verifying_key.to_bytes();

    // Verify the keypair works (self-test)
    println!("Verifying keypair...");
    let test_message = b"kindly-av1 license signing test";
    let test_signature: Signature = signing_key.sign(test_message);

    if verifying_key.verify(test_message, &test_signature).is_err() {
        eprintln!("{}ERROR:{} Keypair verification failed!", GOLD, RESET);
        std::process::exit(1);
    }
    println!("{}OK{} Keypair verified successfully.\n", GOLD, RESET);

    // Write signing key (PRIVATE - server only)
    println!("Writing signing key (PRIVATE)...");
    match write_key_file(&signing_key_path, &signing_key_bytes) {
        Ok(_) => println!("  {}OK{} {}", GOLD, RESET, signing_key_path.display()),
        Err(e) => {
            eprintln!("{}ERROR:{} Failed to write signing key: {}", GOLD, RESET, e);
            std::process::exit(1);
        }
    }

    // Write public key (safe to distribute)
    println!("Writing public key (DISTRIBUTABLE)...");
    match write_key_file(&public_key_path, &verifying_key_bytes) {
        Ok(_) => println!("  {}OK{} {}", GOLD, RESET, public_key_path.display()),
        Err(e) => {
            eprintln!("{}ERROR:{} Failed to write public key: {}", GOLD, RESET, e);
            std::process::exit(1);
        }
    }

    // Generate Rust const array for embedding
    let rust_const_path = keys_dir.join("public_key.rs");
    println!("Writing Rust const array...");
    match write_rust_const(&rust_const_path, &verifying_key_bytes) {
        Ok(_) => println!("  {}OK{} {}", GOLD, RESET, rust_const_path.display()),
        Err(e) => {
            eprintln!("{}ERROR:{} Failed to write Rust const: {}", GOLD, RESET, e);
            std::process::exit(1);
        }
    }

    // Print summary
    println!("\n{}=== Key Generation Complete ==={}\n", PURPLE, RESET);

    println!("{}Private Key (BASE64 - KEEP SECRET):{}", GOLD, RESET);
    println!("  {}\n", BASE64.encode(&signing_key_bytes));

    println!("{}Public Key (BASE64 - safe to share):{}", GOLD, RESET);
    println!("  {}\n", BASE64.encode(&verifying_key_bytes));

    println!("{}Public Key (Rust array - for build.rs):{}", GOLD, RESET);
    print_rust_array(&verifying_key_bytes);

    println!("\n{}Security Reminders:{}", PURPLE, RESET);
    println!("  1. {}NEVER{} commit signing_key.bin to git", GOLD, RESET);
    println!("  2. Add 'keys/signing_key.bin' to .gitignore");
    println!("  3. Store private key in secure location (HSM, encrypted disk)");
    println!("  4. Public key will be embedded by build.rs automatically");
    println!();

    // Create/update .gitignore in keys directory
    let gitignore_path = keys_dir.join(".gitignore");
    if !gitignore_path.exists() {
        let _ = fs::write(&gitignore_path, "# Private signing key - NEVER commit!\nsigning_key.bin\n");
        println!("Created {}", gitignore_path.display());
    }
}

/// Write key bytes to file atomically
fn write_key_file(path: &PathBuf, bytes: &[u8; 32]) -> std::io::Result<()> {
    // Write to temp file first
    let temp_path = path.with_extension("tmp");
    let mut file = File::create(&temp_path)?;
    file.write_all(bytes)?;
    file.sync_all()?;

    // Atomic rename
    fs::rename(&temp_path, path)?;

    Ok(())
}

/// Write Rust const array file for build.rs inclusion
fn write_rust_const(path: &PathBuf, bytes: &[u8; 32]) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    writeln!(file, "//! Auto-generated Ed25519 public key for license verification")?;
    writeln!(file, "//!")?;
    writeln!(file, "//! Generated by: kindly-av1-keygen")?;
    writeln!(file, "//! DO NOT EDIT MANUALLY - regenerate with keygen tool")?;
    writeln!(file, "//!")?;
    writeln!(file, "//! This key is embedded in the kindly-av1 binary for offline")?;
    writeln!(file, "//! license signature verification.")?;
    writeln!(file)?;
    writeln!(file, "/// Ed25519 public key for offline license verification (32 bytes)")?;
    writeln!(file, "///")?;
    writeln!(file, "/// Used to verify Ed25519 signatures on stored license files.")?;
    writeln!(file, "/// The corresponding private key is held by the activation server.")?;
    writeln!(file, "pub const ED25519_PUBLIC_KEY: [u8; 32] = [")?;

    // Write bytes in rows of 8 for readability
    for (i, byte) in bytes.iter().enumerate() {
        if i % 8 == 0 {
            write!(file, "    ")?;
        }
        write!(file, "0x{:02x}, ", byte)?;
        if i % 8 == 7 {
            writeln!(file)?;
        }
    }

    writeln!(file, "];")?;

    file.sync_all()?;
    Ok(())
}

/// Print Rust array to stdout
fn print_rust_array(bytes: &[u8; 32]) {
    println!("  [");
    for (i, byte) in bytes.iter().enumerate() {
        if i % 8 == 0 {
            print!("      ");
        }
        print!("0x{:02x}, ", byte);
        if i % 8 == 7 {
            println!();
        }
    }
    println!("  ]");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        // Test sign/verify round-trip
        let message = b"test message";
        let signature = signing_key.sign(message);
        assert!(verifying_key.verify(message, &signature).is_ok());
    }

    #[test]
    fn test_key_size() {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        assert_eq!(signing_key.to_bytes().len(), 32);
        assert_eq!(verifying_key.to_bytes().len(), 32);
    }

    #[test]
    fn test_signature_size() {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);

        let message = b"test";
        let signature = signing_key.sign(message);

        assert_eq!(signature.to_bytes().len(), 64);
    }

    #[test]
    fn test_wrong_message_fails() {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        let message = b"correct message";
        let wrong_message = b"wrong message";
        let signature = signing_key.sign(message);

        assert!(verifying_key.verify(wrong_message, &signature).is_err());
    }
}
