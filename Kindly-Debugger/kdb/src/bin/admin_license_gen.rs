/// Admin License Generator
///
/// **Tier**: T0 Auditable (admin tooling, zero performance requirements)
/// **Purpose**: Generate Enterprise license for admin/testing
/// **Security**: Requires access to signing.key (TRADE SECRET)
///
/// # Usage
/// ```bash
/// cargo run --bin admin_license_gen --features license-signing
/// ```
///
/// # Output
/// Valid Enterprise license key for unlimited sessions

use ed25519_dalek::{SigningKey, Signer, VerifyingKey};
use std::time::{SystemTime, UNIX_EPOCH};

/// Admin License Generator Capsule (T0 Auditable)
///
/// Zero-dependency license generation for admin use.
/// Signs with Ed25519 private key from kdb-signup/keys/signing.key
///
/// **Alignment**: N/A (stateless tool)
/// **Size**: N/A (stateless tool)
/// **Latency**: <1ms (Ed25519 signing)
#[repr(C)]
pub struct AdminLicenseGenCapsule;

impl AdminLicenseGenCapsule {
    /// Generate Enterprise license for admin use
    ///
    /// Format: `KDB-ENTERPRISE-{timestamp}-{org_hash}-{signature_hex}`
    ///
    /// # Parameters
    /// - `signing_key_hex`: 64-char hex Ed25519 private key
    /// - `org_name`: Organization name (e.g., "Kindly Software Admin")
    ///
    /// # Returns
    /// Valid Enterprise license key
    ///
    /// #ASSUME_KEY_VALID: signing_key_hex is valid 32-byte Ed25519 key
    /// #VERIFY_KEY: Test with public key validation before use
    pub fn generate(signing_key_hex: &str, org_name: &str) -> Result<String, String> {
        // 1. Parse signing key from hex
        let signing_key_bytes = hex_to_bytes(signing_key_hex)
            .ok_or_else(|| "Invalid signing key hex".to_string())?;

        if signing_key_bytes.len() != 32 {
            return Err(format!(
                "Invalid key length: expected 32 bytes, got {}",
                signing_key_bytes.len()
            ));
        }

        let mut key_array = [0u8; 32];
        key_array.copy_from_slice(&signing_key_bytes);

        let signing_key = SigningKey::from_bytes(&key_array);
        let verifying_key = signing_key.verifying_key();

        // 2. Generate timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // 3. Hash organization name (FNV-1a)
        let org_hash = fnv1a_hash(org_name.as_bytes());

        // 4. Build payload to sign
        let payload = format!("ENTERPRISE:{}:{}:{}", timestamp, org_hash, org_name);

        // 5. Sign with Ed25519
        let signature = signing_key.sign(payload.as_bytes());
        let signature_hex = bytes_to_hex(signature.to_bytes());

        // 6. Format license key
        let license_key = format!(
            "KDB-ENTERPRISE-{}-{:016x}-{}",
            timestamp, org_hash, signature_hex
        );

        // 7. Verify signature (self-test)
        let verification_payload = format!("ENTERPRISE:{}:{}:{}", timestamp, org_hash, org_name);
        verifying_key
            .verify_strict(verification_payload.as_bytes(), &signature)
            .map_err(|_| "Self-verification failed".to_string())?;

        Ok(license_key)
    }
}

/// FNV-1a hash (64-bit)
///
/// **Tier**: T0 Auditable (0ns compile-time possible, <5ns runtime)
///
/// #ASSUME_SMALL_INPUT: org_name is <1KB
fn fnv1a_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Convert hex string to bytes
///
/// #ASSUME_HEX_VALID: Input is valid hex (0-9a-fA-F)
fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if hex.len() % 2 != 0 {
        return None;
    }

    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16).ok()?;
        bytes.push(byte);
    }
    Some(bytes)
}

/// Convert bytes to hex string
fn bytes_to_hex<const N: usize>(bytes: [u8; N]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn main() {
    // Read signing key from kdb-signup
    let signing_key_path =
        "/home/samuel/Primitives/Kindly-Debugger/kdb-signup/keys/signing.key";

    let key_contents = std::fs::read_to_string(signing_key_path)
        .expect("Failed to read signing.key");

    // Extract hex key (skip comment lines)
    let signing_key_hex = key_contents
        .lines()
        .find(|line| !line.starts_with('#') && !line.trim().is_empty())
        .expect("No hex key found in signing.key");

    // Generate Enterprise license for admin
    let org_name = "Kindly Software Admin";

    match AdminLicenseGenCapsule::generate(signing_key_hex, org_name) {
        Ok(license_key) => {
            println!("=================================================================");
            println!("  KDB Admin Enterprise License");
            println!("=================================================================");
            println!();
            println!("License Key:");
            println!("{}", license_key);
            println!();
            println!("Tier: Enterprise (Unlimited sessions)");
            println!("Organization: {}", org_name);
            println!("Valid Until: Never (admin license)");
            println!();
            println!("=================================================================");
            println!("  Setup Instructions");
            println!("=================================================================");
            println!();
            println!("1. Add to ~/.claude.json:");
            println!("   {{");
            println!("     \"mcpServers\": {{");
            println!("       \"kdb\": {{");
            println!("         \"transport\": {{");
            println!("           \"type\": \"sse\",");
            println!("           \"url\": \"https://mcp.kindly.software/sse\",");
            println!("           \"headers\": {{");
            println!("             \"X-License-Key\": \"{}\"", license_key);
            println!("           }}");
            println!("         }}");
            println!("       }}");
            println!("     }}");
            println!("   }}");
            println!();
            println!("2. Restart Claude Code");
            println!();
            println!("3. Run /mcp to test connection");
            println!();
            println!("=================================================================");
        }
        Err(e) => {
            eprintln!("ERROR: Failed to generate license: {}", e);
            std::process::exit(1);
        }
    }
}
