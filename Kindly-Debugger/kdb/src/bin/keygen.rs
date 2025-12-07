//! Ed25519 Keypair Generator for KDB License Signing
//!
//! Generates a cryptographically secure Ed25519 keypair for license signing.
//!
//! # Usage
//! ```bash
//! cargo run --bin keygen --features license-signing
//! ```
//!
//! # Output
//! - Private key: Hex-encoded (32 bytes) - STORE SECURELY, NEVER EMBED
//! - Public key: Rust array format - EMBED in src/ptrace/license.rs
//!
//! # Security
//! - Private key MUST be stored in secure offline storage (HSM, air-gapped machine)
//! - Private key NEVER committed to version control
//! - Public key safe to embed in binary (verification only)
//!
//! # TRADE SECRET
//! Generated keys are trade secrets. Handle according to trade secret protocols.

use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::RngCore;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    println!("=================================================================");
    println!("  KDB Ed25519 Keypair Generator");
    println!("  TRADE SECRET - Handle with care");
    println!("=================================================================\n");

    // Generate cryptographically secure random bytes
    let mut secret_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut secret_bytes);

    // Create signing key (private) and verifying key (public)
    let signing_key = SigningKey::from_bytes(&secret_bytes);
    let verifying_key: VerifyingKey = signing_key.verifying_key();

    // Get raw bytes
    let private_bytes: [u8; 32] = signing_key.to_bytes();
    let public_bytes: [u8; 32] = verifying_key.to_bytes();

    // Format private key as hex (for secure storage)
    let private_hex: String = private_bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    // Format public key as Rust array (for embedding)
    let public_rust = format_as_rust_array(&public_bytes, "KDB_PUBLIC_KEY_BYTES");

    // Print results
    println!("PRIVATE KEY (STORE SECURELY - NEVER COMMIT):");
    println!("---------------------------------------------");
    println!("{}", private_hex);
    println!();

    println!("PUBLIC KEY (Rust format for embedding):");
    println!("----------------------------------------");
    println!("{}", public_rust);
    println!();

    // Save to files
    let kdb_dir = Path::new("/home/samuel/Primitives/kdb");
    let keys_dir = kdb_dir.join("keys");

    // Create keys directory if it doesn't exist
    if !keys_dir.exists() {
        fs::create_dir_all(&keys_dir).expect("Failed to create keys directory");
        println!("Created keys directory: {}", keys_dir.display());
    }

    // Save private key (hex format)
    let private_key_path = keys_dir.join("kdb_private_key.hex");
    let mut private_file = fs::File::create(&private_key_path)
        .expect("Failed to create private key file");
    writeln!(private_file, "# KDB Ed25519 Private Key (TRADE SECRET - NEVER COMMIT)").unwrap();
    writeln!(private_file, "# Generated: {}", chrono_timestamp()).unwrap();
    writeln!(private_file, "# Format: Hex-encoded 32 bytes").unwrap();
    writeln!(private_file, "#").unwrap();
    writeln!(private_file, "# SECURITY: Store in HSM or air-gapped machine").unwrap();
    writeln!(private_file, "# SECURITY: Delete this file after secure storage").unwrap();
    writeln!(private_file, "#").unwrap();
    writeln!(private_file, "{}", private_hex).unwrap();
    println!("Private key saved: {}", private_key_path.display());

    // Save public key (Rust format)
    let public_key_path = keys_dir.join("kdb_public_key.rs");
    let mut public_file = fs::File::create(&public_key_path)
        .expect("Failed to create public key file");
    writeln!(public_file, "// KDB Ed25519 Public Key for License Verification").unwrap();
    writeln!(public_file, "// Generated: {}", chrono_timestamp()).unwrap();
    writeln!(public_file, "// Copy this into src/ptrace/license.rs").unwrap();
    writeln!(public_file, "//").unwrap();
    writeln!(public_file, "// SECURITY: This is the PUBLIC key only.").unwrap();
    writeln!(public_file, "// Safe to embed in binary for verification.").unwrap();
    writeln!(public_file, "").unwrap();
    writeln!(public_file, "{}", public_rust).unwrap();
    println!("Public key saved: {}", public_key_path.display());

    // Create .gitignore for keys directory
    let gitignore_path = keys_dir.join(".gitignore");
    let mut gitignore = fs::File::create(&gitignore_path)
        .expect("Failed to create .gitignore");
    writeln!(gitignore, "# NEVER commit private keys").unwrap();
    writeln!(gitignore, "*.hex").unwrap();
    writeln!(gitignore, "*_private_*").unwrap();
    writeln!(gitignore, "# Public key can be committed (but embedded version is in license.rs)").unwrap();
    writeln!(gitignore, "# *.rs").unwrap();
    println!("Created .gitignore: {}", gitignore_path.display());

    println!();
    println!("=================================================================");
    println!("  NEXT STEPS:");
    println!("=================================================================");
    println!("1. Copy public key array from {} into", public_key_path.display());
    println!("   src/ptrace/license.rs (replace KDB_PUBLIC_KEY_BYTES)");
    println!();
    println!("2. SECURELY STORE private key from {}", private_key_path.display());
    println!("   Options: HSM, encrypted vault, air-gapped machine");
    println!();
    println!("3. DELETE {} after secure storage", private_key_path.display());
    println!();
    println!("4. NEVER commit private key to version control");
    println!("=================================================================");

    // Verification test
    println!();
    println!("Verification Test:");
    println!("------------------");

    // Sign a test message
    use ed25519_dalek::Signer;
    let test_message = b"KDB-LICENSE-V1:PRO:1234567890:ABCD1234";
    let signature = signing_key.sign(test_message);

    // Verify with public key
    use ed25519_dalek::Verifier;
    match verifying_key.verify(test_message, &signature) {
        Ok(()) => println!("SUCCESS: Keypair verification passed"),
        Err(e) => println!("ERROR: Keypair verification failed: {}", e),
    }
}

/// Format byte array as Rust const declaration
fn format_as_rust_array(bytes: &[u8; 32], name: &str) -> String {
    let mut result = String::new();
    result.push_str(&format!(
        "const {}: [u8; {}] = [\n",
        name,
        bytes.len()
    ));

    // Format as 8 bytes per line with comments
    for (i, chunk) in bytes.chunks(8).enumerate() {
        result.push_str("    ");
        for (j, byte) in chunk.iter().enumerate() {
            result.push_str(&format!("0x{:02x}", byte));
            if i * 8 + j < 31 {
                result.push_str(", ");
            }
        }
        result.push_str(&format!(" // bytes {}-{}\n", i * 8, i * 8 + chunk.len() - 1));
    }

    result.push_str("];");
    result
}

/// Get current timestamp as ISO 8601 string
fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();

    // Simple ISO 8601 format (approximate)
    let days_since_epoch = secs / 86400;
    let years = 1970 + (days_since_epoch / 365);
    let remaining_days = days_since_epoch % 365;
    let months = remaining_days / 30 + 1;
    let days = remaining_days % 30 + 1;

    let day_secs = secs % 86400;
    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        years, months, days, hours, minutes, seconds
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_as_rust_array() {
        let bytes = [0u8; 32];
        let result = format_as_rust_array(&bytes, "TEST");
        assert!(result.contains("const TEST: [u8; 32]"));
        assert!(result.contains("0x00"));
    }

    #[test]
    fn test_keypair_generation() {
        let mut secret_bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut secret_bytes);

        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();

        // Test sign and verify
        use ed25519_dalek::{Signer, Verifier};
        let message = b"test message";
        let signature = signing_key.sign(message);
        assert!(verifying_key.verify(message, &signature).is_ok());
    }
}
