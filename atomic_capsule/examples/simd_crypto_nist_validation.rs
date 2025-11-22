//! # NIST Crypto Integration Validation
//!
//! **Validates real cryptographic libraries integration in SimdCryptoCapsule**
//!
//! This example demonstrates:
//! - AES-256-GCM encryption/decryption (aes-gcm crate)
//! - SHA3-256 hashing (sha3 crate)
//! - PBKDF2-HMAC-SHA256 key derivation (pbkdf2 crate)
//!
//! Run with: cargo run --example simd_crypto_nist_validation --features simd-crypto

#![cfg(feature = "simd-crypto")]

use atomic_capsule::primitives::SimdCryptoCapsule;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SimdCryptoCapsule NIST Crypto Integration Validation ===\n");

    let mut capsule = SimdCryptoCapsule::new();

    // Test 1: AES-256-GCM Encryption/Decryption
    println!("Test 1: AES-256-GCM Encryption/Decryption");
    let key = [1u8; 32];
    let iv = [2u8; 12];
    let plaintext = b"Hello, World! This is a test message for AES-256-GCM encryption.";
    let mut ciphertext = vec![0u8; plaintext.len()];
    let mut tag = [0u8; 16];

    // Encrypt
    capsule.aes256_gcm_encrypt(&key, &iv, plaintext, &mut ciphertext, &mut tag)?;
    println!("  ✓ Encrypted {} bytes", plaintext.len());
    println!("  ✓ Tag (16 bytes): {:02x?}", &tag[..4]);

    // Decrypt
    let mut decrypted = vec![0u8; ciphertext.len()];
    capsule.aes256_gcm_decrypt(&key, &iv, &ciphertext, &tag, &mut decrypted)?;
    println!("  ✓ Decrypted {} bytes", decrypted.len());

    // Verify round-trip
    assert_eq!(&decrypted[..plaintext.len()], plaintext);
    println!("  ✓ Round-trip successful: plaintext == decrypted\n");

    // Test 2: SHA3-256 Hashing
    println!("Test 2: SHA3-256 Hashing");
    let data = b"message to hash with SHA3-256";
    let mut hash1 = [0u8; 32];
    let mut hash2 = [0u8; 32];

    capsule.sha3_256_hash(data, &mut hash1)?;
    capsule.sha3_256_hash(data, &mut hash2)?;

    println!("  ✓ Hash (32 bytes): {:02x?}...", &hash1[..8]);
    assert_eq!(hash1, hash2);
    println!("  ✓ Deterministic: hash1 == hash2\n");

    // Test avalanche effect (1-bit change → major hash change)
    let data_modified = b"nessage to hash with SHA3-256"; // 'm' → 'n'
    let mut hash_modified = [0u8; 32];
    capsule.sha3_256_hash(data_modified, &mut hash_modified)?;

    let diff_bits: u32 = hash1.iter()
        .zip(hash_modified.iter())
        .map(|(a, b)| (a ^ b).count_ones())
        .sum();
    println!("  ✓ Avalanche effect: {} bits differ (expected ~128)", diff_bits);
    assert!(diff_bits >= 100, "Avalanche effect must change ≥100 bits");

    // Test 3: PBKDF2-HMAC-SHA256 Key Derivation
    println!("\nTest 3: PBKDF2-HMAC-SHA256 Key Derivation");
    let password = b"user_password";
    let salt = [42u8; 16];
    let iterations = 10_000; // Lower for demo (production: 100K+)
    let mut derived_key1 = [0u8; 32];
    let mut derived_key2 = [0u8; 32];

    capsule.pbkdf2_derive_key(password, &salt, iterations, &mut derived_key1)?;
    capsule.pbkdf2_derive_key(password, &salt, iterations, &mut derived_key2)?;

    println!("  ✓ Derived key (32 bytes): {:02x?}...", &derived_key1[..8]);
    assert_eq!(derived_key1, derived_key2);
    println!("  ✓ Deterministic: key1 == key2");

    // Different salt → different key
    let salt_different = [99u8; 16];
    let mut derived_key_different = [0u8; 32];
    capsule.pbkdf2_derive_key(password, &salt_different, iterations, &mut derived_key_different)?;
    assert_ne!(derived_key1, derived_key_different);
    println!("  ✓ Salt sensitivity: different salts → different keys\n");

    // Test 4: Capsule Metrics
    println!("Test 4: Capsule Metrics");
    println!("  ✓ Operations: {}", capsule.operation_count());
    println!("  ✓ Bytes processed: {}", capsule.bytes_processed());
    println!("  ✓ Errors: {}", capsule.error_count());

    assert!(capsule.operation_count() >= 8, "Should have ≥8 operations");
    assert_eq!(capsule.error_count(), 0, "Should have zero errors");

    println!("\n=== All NIST Crypto Integration Tests PASSED ✓ ===");

    Ok(())
}
