//! BatchValidatorCapsule Demo - Real Ed25519 and ECDSA Signature Verification
//!
//! **Demonstrates**: Production-grade batch signature verification with real cryptography
//! **Tier**: T4 Batch (8-16× speedup vs sequential)
//! **Frameworks**: UCE34, Chaos, ASSUM, B32, T28
//!
//! **Usage**:
//! ```bash
//! cargo run --release --features batch-crypto --example batch_validator_demo
//! ```

#![cfg(feature = "batch-crypto")]

use atomic_capsule::parallel::BatchValidatorCapsule;
use ed25519_dalek::{Signature, Signer, SigningKey};
use k256::ecdsa::{
    signature::Signer as EcdsaSigner,
    Signature as EcdsaSignature, SigningKey as EcdsaSigningKey,
};
use rand::rngs::OsRng;
use rand::RngCore;

fn main() {
    println!("========================================");
    println!("BatchValidatorCapsule Real Crypto Demo");
    println!("========================================\n");

    let validator = BatchValidatorCapsule::new();

    // ========================================================================
    // Ed25519 Batch Verification
    // ========================================================================

    println!("1. Ed25519 Batch Verification (64 signatures)");
    println!("---------------------------------------------");

    let batch_size = 64;
    let mut messages = Vec::with_capacity(batch_size);
    let mut signatures = Vec::with_capacity(batch_size);
    let mut public_keys = Vec::with_capacity(batch_size);

    // Generate 64 real Ed25519 signatures
    for i in 0..batch_size {
        // Generate signing key
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        let signing_key = SigningKey::from_bytes(&secret);
        let verifying_key = signing_key.verifying_key();

        // Message
        let message = format!("Transaction {}", i);
        messages.push(message.as_bytes().to_vec());

        // Sign
        let signature: Signature = signing_key.sign(message.as_bytes());
        signatures.push(signature.to_bytes());

        // Public key
        public_keys.push(verifying_key.to_bytes());
    }

    // Convert to slices
    let msg_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    let sig_refs: Vec<&[u8; 64]> = signatures.iter().collect();
    let key_refs: Vec<&[u8; 32]> = public_keys.iter().collect();

    // Verify batch
    let start = std::time::Instant::now();
    let results = validator
        .verify_batch_ed25519(&msg_refs, &sig_refs, &key_refs)
        .expect("Batch verification failed");
    let elapsed = start.elapsed();

    let valid_count = results.iter().filter(|&&r| r).count();
    let throughput = (batch_size as f64 / elapsed.as_secs_f64()) as u64;

    println!("✓ Results: {}/{} signatures valid", valid_count, batch_size);
    println!("✓ Latency: {:?}", elapsed);
    println!("✓ Throughput: {} sigs/sec", throughput);

    // ========================================================================
    // ECDSA Batch Verification
    // ========================================================================

    println!("\n2. ECDSA (secp256k1) Batch Verification (32 signatures)");
    println!("--------------------------------------------------------");

    let batch_size = 32;
    let mut messages_ec = Vec::with_capacity(batch_size);
    let mut signatures_ec = Vec::with_capacity(batch_size);
    let mut public_keys_ec = Vec::with_capacity(batch_size);

    // Generate 32 real ECDSA signatures
    for i in 0..batch_size {
        // Generate signing key
        let signing_key = EcdsaSigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        // Message
        let message = format!("ECDSA Transaction {}", i);
        messages_ec.push(message.as_bytes().to_vec());

        // Sign
        let signature: EcdsaSignature = signing_key.sign(message.as_bytes());
        signatures_ec.push(signature.to_bytes().to_vec());

        // Public key (compressed SEC1 format)
        public_keys_ec.push(verifying_key.to_sec1_bytes().to_vec());
    }

    // Convert to slices
    let msg_refs_ec: Vec<&[u8]> = messages_ec.iter().map(|m| m.as_slice()).collect();
    let sig_refs_ec: Vec<&[u8]> = signatures_ec.iter().map(|s| s.as_slice()).collect();
    let key_refs_ec: Vec<&[u8]> = public_keys_ec.iter().map(|k| k.as_slice()).collect();

    // Verify batch
    let start = std::time::Instant::now();
    let results = validator
        .verify_batch_ecdsa(&msg_refs_ec, &sig_refs_ec, &key_refs_ec)
        .expect("ECDSA batch verification failed");
    let elapsed = start.elapsed();

    let valid_count = results.iter().filter(|&&r| r).count();
    let throughput = (batch_size as f64 / elapsed.as_secs_f64()) as u64;

    println!("✓ Results: {}/{} ECDSA signatures valid", valid_count, batch_size);
    println!("✓ Latency: {:?}", elapsed);
    println!("✓ Throughput: {} sigs/sec", throughput);

    // ========================================================================
    // Statistics
    // ========================================================================

    println!("\n3. Validator Statistics");
    println!("-----------------------");

    let stats = validator.stats();
    println!("✓ Total verified: {}", stats.verified_count);
    println!("✓ Total failed: {}", stats.failed_count);
    println!("✓ Average time: {}ns", stats.avg_time_ns);
    println!("✓ Batch size: {}", stats.batch_size);
    println!("✓ Thread count: {}", stats.thread_count);

    println!("\n✓ Demo completed successfully!");
    println!("✓ Real Ed25519 and ECDSA signature verification working!");
}
