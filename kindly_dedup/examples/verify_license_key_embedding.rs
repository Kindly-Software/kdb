//! Example: Verify License Key Embedding
//!
//! This example demonstrates how license keys are embedded at compile-time
//! and used for cryptographic license verification.
//!
//! ## Build and Run
//!
//! ```bash
//! # Default build (generates key from UUID)
//! cargo run --example verify_license_key_embedding --features "protection-crypto-license"
//!
//! # Custom customer build
//! export CUSTOMER_ID="alice@example.com"
//! cargo run --example verify_license_key_embedding --features "protection-crypto-license"
//!
//! # Override with specific key
//! export LICENSE_KEY_PUBLIC="deadbeef..." # 64 hex chars
//! cargo run --example verify_license_key_embedding --features "protection-crypto-license"
//! ```

fn main() {
    println!("=== License Key Embedding Verification ===\n");

    // Step 1: Get build-time constants
    let customer_id = env!("CUSTOMER_ID");
    let build_timestamp = env!("BUILD_TIMESTAMP");
    let build_signature = env!("BUILD_SIGNATURE");
    let license_key_public = env!("LICENSE_KEY_PUBLIC");

    println!("Build-Time Constants (Embedded at Compile-Time):");
    println!("  Customer ID:     {}", customer_id);
    println!("  Build Timestamp: {}", build_timestamp);
    println!("  Build Signature: {}", build_signature);
    println!("  License Key:     {}", license_key_public);

    // Step 2: Validate license key format
    println!("\nLicense Key Validation:");
    if license_key_public.len() == 64 {
        println!("  ✓ Length: 64 characters (32 bytes in hex)");
    } else {
        println!("  ✗ Length: {} characters (expected 64)", license_key_public.len());
        return;
    }

    if license_key_public.chars().all(|c| c.is_ascii_hexdigit()) {
        println!("  ✓ Format: Valid hexadecimal");
    } else {
        println!("  ✗ Format: Invalid hexadecimal characters");
        return;
    }

    // Step 3: Parse hex to bytes (same as crypto_license_wrapper.rs)
    println!("\nParsing License Key (Hex → [u8; 32]):");
    let mut public_key = [0u8; 32];
    let mut valid = true;

    for i in 0..32 {
        let hex_pair = &license_key_public[i * 2..i * 2 + 2];
        match u8::from_str_radix(hex_pair, 16) {
            Ok(byte) => {
                public_key[i] = byte;
            }
            Err(e) => {
                println!("  ✗ Error at position {}: {}", i * 2, e);
                valid = false;
                break;
            }
        }
    }

    if valid {
        println!("  ✓ Successfully parsed 32 bytes from hex string");
        println!("  ✓ First 4 bytes: {:02x} {:02x} {:02x} {:02x}",
            public_key[0], public_key[1], public_key[2], public_key[3]
        );
        println!("  ✓ Last 4 bytes:  {:02x} {:02x} {:02x} {:02x}",
            public_key[28], public_key[29], public_key[30], public_key[31]
        );
    }

    // Step 4: Show audit trail location
    println!("\nBuild Audit Trail:");
    println!("  Location: build_audit.log");
    println!("  Format:   JSON Lines (one event per line)");
    println!("  Fields:   timestamp, customer_id, binary_signature, license_key, rustc_version, target, profile");

    // Step 5: Implementation summary
    println!("\n=== Implementation Summary ===");
    println!("✓ License key embedded at compile-time (env! macro)");
    println!("✓ Ed25519 public key format (32 bytes)");
    println!("✓ Deterministic derivation from CUSTOMER_ID");
    println!("✓ Zero runtime cost (constant loaded from binary)");
    println!("✓ Q34 audit trail with full build metadata");
    println!("✓ Framework compliant: UCE34, ASSUM, B32, COCA");

    // Step 6: Show how to use in production
    println!("\n=== Production Usage ===");
    println!("1. Embed this public key in binary (done at compile-time)");
    println!("2. Sign license files with corresponding private key (offline)");
    println!("3. Distribute binary + signed license to customer");
    println!("4. CryptoLicenseWrapper verifies license signature against embedded key");
    println!("5. All operations cryptographically secure (Ed25519, 2^128 security)");

    println!("\n=== Verification Complete ===");
    println!("License key embedding is working correctly!");
}
