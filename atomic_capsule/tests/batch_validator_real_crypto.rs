//! Real Crypto Integration Tests for BatchValidatorCapsule
//!
//! **Tests**: Real Ed25519 and ECDSA signature verification with RFC test vectors
//! **Frameworks**: T28 (comprehensive), B32 (validated performance)

#![cfg(feature = "batch-crypto")]

use atomic_capsule::parallel::BatchValidatorCapsule;
use ed25519_dalek::{SecretKey, Signature, Signer, SigningKey};
use k256::ecdsa::{
    signature::Signer as EcdsaSigner,
    Signature as EcdsaSignature, SigningKey as EcdsaSigningKey,
};
use rand::rngs::OsRng;
use rand::RngCore;

// ============================================================================
// HELPER: Generate Real Ed25519 Signatures
// ============================================================================

fn generate_ed25519_signatures(count: usize) -> (Vec<Vec<u8>>, Vec<[u8; 64]>, Vec<[u8; 32]>) {
    let mut messages = Vec::with_capacity(count);
    let mut signatures = Vec::with_capacity(count);
    let mut public_keys = Vec::with_capacity(count);

    for i in 0..count {
        // Generate signing key (32 random bytes)
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        let signing_key = SigningKey::from_bytes(&secret);
        let verifying_key = signing_key.verifying_key();

        // Message
        let message = format!("Test message {}", i);
        messages.push(message.as_bytes().to_vec());

        // Sign
        let signature: Signature = signing_key.sign(message.as_bytes());
        signatures.push(signature.to_bytes());

        // Public key
        public_keys.push(verifying_key.to_bytes());
    }

    (messages, signatures, public_keys)
}

// ============================================================================
// HELPER: Generate Real ECDSA Signatures
// ============================================================================

fn generate_ecdsa_signatures(
    count: usize,
) -> (
    Vec<Vec<u8>>,
    Vec<Vec<u8>>, // Variable-length signatures
    Vec<Vec<u8>>, // Variable-length public keys
) {
    let mut messages = Vec::with_capacity(count);
    let mut signatures = Vec::with_capacity(count);
    let mut public_keys = Vec::with_capacity(count);

    for i in 0..count {
        // Generate signing key
        let signing_key = EcdsaSigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        // Message
        let message = format!("Test message {}", i);
        messages.push(message.as_bytes().to_vec());

        // Sign
        let signature: EcdsaSignature = signing_key.sign(message.as_bytes());
        signatures.push(signature.to_bytes().to_vec());

        // Public key (compressed SEC1 format, 33 bytes)
        public_keys.push(verifying_key.to_sec1_bytes().to_vec());
    }

    (messages, signatures, public_keys)
}

// ============================================================================
// TEST 1: Ed25519 Real Signatures (16 signatures)
// ============================================================================

#[test]
fn test_ed25519_real_signatures_batch_16() {
    let validator = BatchValidatorCapsule::new();

    // Generate 16 real Ed25519 signatures
    let (messages, signatures, public_keys) = generate_ed25519_signatures(16);

    // Convert to slices
    let msg_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let sig_refs: Vec<&[u8; 64]> = signatures.iter().collect();
    let key_refs: Vec<&[u8; 32]> = public_keys.iter().collect();

    // Verify batch
    let results = validator
        .verify_batch_ed25519(&msg_refs, &sig_refs, &key_refs)
        .unwrap();

    // All signatures should be valid
    assert_eq!(results.len(), 16);
    assert!(results.iter().all(|&r| r), "All signatures should be valid");

    // Check statistics
    let stats = validator.stats();
    assert_eq!(stats.verified_count, 16);
    assert_eq!(stats.failed_count, 0);
}

// ============================================================================
// TEST 2: Ed25519 Invalid Signature Detection
// ============================================================================

#[test]
fn test_ed25519_invalid_signature_detection() {
    let validator = BatchValidatorCapsule::new();

    // Generate 16 real Ed25519 signatures
    let (mut messages, mut signatures, public_keys) = generate_ed25519_signatures(16);

    // Corrupt signature at index 8
    signatures[8] = [0xFF; 64];

    // Corrupt message at index 12 (signature won't match)
    messages[12] = b"corrupted message".to_vec();

    // Convert to slices
    let msg_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let sig_refs: Vec<&[u8; 64]> = signatures.iter().collect();
    let key_refs: Vec<&[u8; 32]> = public_keys.iter().collect();

    // Verify batch
    let results = validator
        .verify_batch_ed25519(&msg_refs, &sig_refs, &key_refs)
        .unwrap();

    // Check results
    assert_eq!(results.len(), 16);
    assert!(!results[8], "Corrupted signature should be invalid");
    assert!(!results[12], "Mismatched message should be invalid");

    // Valid signatures
    for i in 0..16 {
        if i != 8 && i != 12 {
            assert!(results[i], "Valid signature at index {} should verify", i);
        }
    }

    // Check statistics
    let stats = validator.stats();
    assert_eq!(stats.verified_count, 14); // 16 - 2 invalid
    assert_eq!(stats.failed_count, 2);
}

// ============================================================================
// TEST 3: ECDSA Real Signatures (32 signatures)
// ============================================================================

#[test]
fn test_ecdsa_real_signatures_batch_32() {
    let validator = BatchValidatorCapsule::new();

    // Generate 32 real ECDSA signatures
    let (messages, signatures, public_keys) = generate_ecdsa_signatures(32);

    // Convert to slices
    let msg_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let sig_refs: Vec<&[u8]> = signatures.iter().map(|s| s.as_slice()).collect();
    let key_refs: Vec<&[u8]> = public_keys.iter().map(|k| k.as_slice()).collect();

    // Verify batch
    let results = validator
        .verify_batch_ecdsa(&msg_refs, &sig_refs, &key_refs)
        .unwrap();

    // All signatures should be valid
    assert_eq!(results.len(), 32);
    assert!(results.iter().all(|&r| r), "All ECDSA signatures should be valid");

    // Check statistics
    let stats = validator.stats();
    assert_eq!(stats.verified_count, 32);
    assert_eq!(stats.failed_count, 0);
}

// ============================================================================
// TEST 4: ECDSA Invalid Signature Detection
// ============================================================================

#[test]
fn test_ecdsa_invalid_signature_detection() {
    let validator = BatchValidatorCapsule::new();

    // Generate 32 real ECDSA signatures
    let (mut messages, mut signatures, public_keys) = generate_ecdsa_signatures(32);

    // Corrupt signature at index 10
    signatures[10] = vec![0xFF; 64];

    // Corrupt message at index 20 (signature won't match)
    messages[20] = b"corrupted ecdsa message".to_vec();

    // Convert to slices
    let msg_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let sig_refs: Vec<&[u8]> = signatures.iter().map(|s| s.as_slice()).collect();
    let key_refs: Vec<&[u8]> = public_keys.iter().map(|k| k.as_slice()).collect();

    // Verify batch
    let results = validator
        .verify_batch_ecdsa(&msg_refs, &sig_refs, &key_refs)
        .unwrap();

    // Check results
    assert_eq!(results.len(), 32);
    assert!(!results[10], "Corrupted ECDSA signature should be invalid");
    assert!(!results[20], "Mismatched ECDSA message should be invalid");

    // Valid signatures
    for i in 0..32 {
        if i != 10 && i != 20 {
            assert!(results[i], "Valid ECDSA signature at index {} should verify", i);
        }
    }

    // Check statistics
    let stats = validator.stats();
    assert_eq!(stats.verified_count, 30); // 32 - 2 invalid
    assert_eq!(stats.failed_count, 2);
}

// ============================================================================
// TEST 5: Large Batch Ed25519 (256 signatures = MAX_BATCH_SIZE)
// ============================================================================

#[test]
fn test_ed25519_max_batch_size() {
    let validator = BatchValidatorCapsule::new();

    // Generate 256 real Ed25519 signatures
    let (messages, signatures, public_keys) = generate_ed25519_signatures(256);

    // Convert to slices
    let msg_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let sig_refs: Vec<&[u8; 64]> = signatures.iter().collect();
    let key_refs: Vec<&[u8; 32]> = public_keys.iter().collect();

    // Verify batch
    let start = std::time::Instant::now();
    let results = validator
        .verify_batch_ed25519(&msg_refs, &sig_refs, &key_refs)
        .unwrap();
    let elapsed = start.elapsed();

    // All signatures should be valid
    assert_eq!(results.len(), 256);
    assert!(results.iter().all(|&r| r), "All 256 signatures should be valid");

    // Check statistics
    let stats = validator.stats();
    assert_eq!(stats.verified_count, 256);
    assert_eq!(stats.failed_count, 0);

    println!(
        "Ed25519 MAX_BATCH_SIZE (256) verification took: {:?} ({} sigs/sec)",
        elapsed,
        (256 as f64 / elapsed.as_secs_f64()) as u64
    );
}

// ============================================================================
// TEST 6: RFC 8032 Test Vectors (Ed25519)
// ============================================================================

#[test]
fn test_ed25519_rfc8032_test_vector_1() {
    let validator = BatchValidatorCapsule::new();

    // RFC 8032 Test Vector 1 (empty message)
    // https://tools.ietf.org/html/rfc8032#section-7.1
    // Public key: d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a
    // Signature: e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155
    //            5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b

    let public_key: [u8; 32] = [
        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7,
        0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
        0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25,
        0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
    ];

    let signature: [u8; 64] = [
        0xe5, 0x56, 0x43, 0x00, 0xc3, 0x60, 0xac, 0x72,
        0x90, 0x86, 0xe2, 0xcc, 0x80, 0x6e, 0x82, 0x8a,
        0x84, 0x87, 0x7f, 0x1e, 0xb8, 0xe5, 0xd9, 0x74,
        0xd8, 0x73, 0xe0, 0x65, 0x22, 0x49, 0x01, 0x55,
        0x5f, 0xb8, 0x82, 0x15, 0x90, 0xa3, 0x3b, 0xac,
        0xc6, 0x1e, 0x39, 0x70, 0x1c, 0xf9, 0xb4, 0x6b,
        0xd2, 0x5b, 0xf5, 0xf0, 0x59, 0x5b, 0xbe, 0x24,
        0x65, 0x51, 0x41, 0x43, 0x8e, 0x7a, 0x10, 0x0b,
    ];

    let message: &[u8] = b""; // Empty message

    // Verify
    let results = validator
        .verify_batch_ed25519(&[message], &[&signature], &[&public_key])
        .unwrap();

    assert!(results[0], "RFC 8032 Test Vector 1 should verify");
}

// ============================================================================
// TEST 7: Mixed Valid/Invalid Batch
// ============================================================================

#[test]
fn test_mixed_valid_invalid_batch() {
    let validator = BatchValidatorCapsule::new();

    // Generate 64 signatures: 50 valid, 14 invalid
    let (messages, mut signatures, public_keys) = generate_ed25519_signatures(64);

    // Corrupt every 4th signature (indices 3, 7, 11, ..., 63)
    for i in (3..64).step_by(4) {
        signatures[i] = [0xFF; 64];
    }

    // Convert to slices
    let msg_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let sig_refs: Vec<&[u8; 64]> = signatures.iter().collect();
    let key_refs: Vec<&[u8; 32]> = public_keys.iter().collect();

    // Verify batch
    let results = validator
        .verify_batch_ed25519(&msg_refs, &sig_refs, &key_refs)
        .unwrap();

    // Count valid/invalid
    let valid_count = results.iter().filter(|&&r| r).count();
    let invalid_count = results.iter().filter(|&&r| !r).count();

    assert_eq!(valid_count, 48); // 64 - 16 corrupted
    assert_eq!(invalid_count, 16);

    // Check statistics
    let stats = validator.stats();
    assert_eq!(stats.verified_count, 48);
    assert_eq!(stats.failed_count, 16);
}
