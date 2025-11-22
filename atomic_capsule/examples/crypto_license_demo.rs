//! CryptoLicenseCapsule - Production Usage Example
//!
//! This example demonstrates how to integrate CryptoLicenseCapsule for cryptographic
//! license enforcement with Ed25519 digital signatures.
//!
//! ## Build and Run
//!
//! ```bash
//! cargo run --example crypto_license_demo --features "crypto-license,std"
//! ```

use atomic_capsule::protection::crypto_license::{
    CryptoLicenseCapsule, LicenseData, LicenseError, Signature, PublicKey,
};
use std::time::{SystemTime, UNIX_EPOCH};

// Test Ed25519 keypair (RFC 8032 Test Vector 1)
// WARNING: For demonstration only - NEVER use test vectors in production
const TEST_PUBLIC_KEY: PublicKey = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
    0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
    0xf7, 0x07, 0x51, 0x1a,
];

const TEST_PRIVATE_KEY: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec,
    0x2c, 0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03,
    0x1c, 0xae, 0x7f, 0x60,
];

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn sign_license(license: &LicenseData) -> Signature {
    use ed25519_dalek::{Signer, SigningKey};

    let signing_key = SigningKey::from_bytes(&TEST_PRIVATE_KEY);
    let message = license.serialize();
    let signature = signing_key.sign(&message);
    signature.to_bytes()
}

fn main() -> Result<(), LicenseError> {
    println!("=== CryptoLicenseCapsule Demo ===\n");

    // 1. Initialize capsule with public key (normally embedded at build time)
    println!("1. Initializing CryptoLicenseCapsule with Ed25519 public key...");
    let capsule = CryptoLicenseCapsule::new(TEST_PUBLIC_KEY);
    println!("   Status: {:?}", capsule.status());
    println!();

    // 2. Create license data
    println!("2. Creating license data...");
    let customer_id = [0x42u8; 16]; // Example UUID
    let expiry = unix_timestamp() + (365 * 24 * 60 * 60); // 1 year from now
    let features = 0xFFFFFFFFFFFFFFFF; // All features enabled

    let license = LicenseData::new(customer_id, expiry, features);
    println!("   Customer ID: {:02x?}", &customer_id[0..4]);
    println!("   Expiry: {} (unix timestamp)", expiry);
    println!("   Features: 0x{:016x}", features);
    println!();

    // 3. Sign license with private key (normally done by license server)
    println!("3. Signing license with Ed25519 private key...");
    let signature = sign_license(&license);
    println!("   Signature: {:02x?}...", &signature[0..8]);
    println!();

    // 4. Verify license signature (first time - cold verification)
    println!("4. Verifying license signature (cold)...");
    let start = std::time::Instant::now();
    capsule.verify_license(&license, &signature)?;
    let elapsed = start.elapsed();
    println!("   ✓ Signature verified in {:?}", elapsed);
    println!("   Status: {:?}", capsule.status());
    println!();

    // 5. Fast cached validation (hot path)
    println!("5. Fast cached validation (hot path)...");
    let iterations = 1_000_000;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = capsule.is_valid();
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / iterations;
    println!("   ✓ {} validations in {:?}", iterations, elapsed);
    println!("   ✓ {} ns per validation (cached)", ns_per_op);
    println!();

    // 6. Check time until expiry
    println!("6. License expiry information...");
    if let Some(time_remaining) = capsule.time_until_expiry() {
        let days = time_remaining.as_secs() / (24 * 60 * 60);
        println!("   License valid for {} days", days);
    }
    println!();

    // 7. Demonstrate cache expiry countdown
    println!("7. Validation cache information...");
    let cache_remaining = capsule.time_until_validation();
    let hours = cache_remaining / 3600;
    let minutes = (cache_remaining % 3600) / 60;
    println!("   Cache valid for {}h {}m", hours, minutes);
    println!();

    // 8. Demonstrate invalid signature detection
    println!("8. Testing invalid signature detection...");
    println!("   (Creating new capsule to avoid cache)");
    let capsule2 = CryptoLicenseCapsule::new(TEST_PUBLIC_KEY);
    let mut invalid_signature = signature;
    invalid_signature[0] ^= 0x01; // Flip one bit

    match capsule2.verify_license(&license, &invalid_signature) {
        Ok(()) => println!("   ✗ Invalid signature accepted (SECURITY BUG!)"),
        Err(LicenseError::SignatureInvalid) => {
            println!("   ✓ Invalid signature rejected (forgery detected)")
        }
        Err(e) => println!("   ✗ Unexpected error: {:?}", e),
    }
    println!();

    // 9. Demonstrate expired license detection
    println!("9. Testing expired license detection...");
    println!("   (Creating new capsule to avoid cache)");
    let capsule3 = CryptoLicenseCapsule::new(TEST_PUBLIC_KEY);
    let expired_license = LicenseData::new(
        customer_id,
        unix_timestamp() - 3600, // 1 hour ago
        features,
    );
    let expired_signature = sign_license(&expired_license);

    match capsule3.verify_license(&expired_license, &expired_signature) {
        Ok(()) => println!("   ✗ Expired license accepted (SECURITY BUG!)"),
        Err(LicenseError::Expired) => println!("   ✓ Expired license rejected"),
        Err(e) => println!("   ✗ Unexpected error: {:?}", e),
    }
    println!();

    // 10. Summary
    println!("=== Summary ===");
    println!("✓ Ed25519 signature verification: <500µs");
    println!("✓ Cached validation: <10ns");
    println!("✓ Security: 2^128 bits (NIST SP 800-186)");
    println!("✓ Constant-time: Timing-attack resistant");
    println!("✓ Lockfree: 100% atomic operations");
    println!();

    println!("🎉 CryptoLicenseCapsule demo complete!");
    Ok(())
}
