//! Integration tests for automatic CRC generation
//!
//! **Phase 5: Auto CRC & Integrity Expert**
//!
//! Tests:
//! 1. Auto-generated CRC32 computation
//! 2. Hash chain verification with prev_hash
//! 3. Zero boilerplate validation
//! 4. Q34 Auditability compliance

use atomic_capsule::fixed_point::Q16_16;
use atomic_capsule_derive_serialize::CapsuleSerialize;

// Test 1: Basic auto-CRC generation
#[derive(CapsuleSerialize)]
#[capsule_serialize(auto_crc = true)]
#[repr(C, align(256))]
struct PaymentCapsule {
    amount: Q16_16,
    fee: Q16_16,
}

#[test]
fn test_auto_crc_compute_checksum() {
    let payment = PaymentCapsule {
        amount: Q16_16::from_f64(100.0),
        fee: Q16_16::from_f64(2.50),
    };

    // Auto-generated compute_checksum() method
    let checksum = payment.compute_checksum();
    assert_ne!(checksum, 0, "CRC32 checksum should be non-zero");

    // Determinism: Same values → same checksum
    let payment2 = PaymentCapsule {
        amount: Q16_16::from_f64(100.0),
        fee: Q16_16::from_f64(2.50),
    };
    assert_eq!(
        checksum,
        payment2.compute_checksum(),
        "CRC32 must be deterministic"
    );
}

#[test]
fn test_auto_crc_verify_integrity() {
    let payment = PaymentCapsule {
        amount: Q16_16::from_f64(50.0),
        fee: Q16_16::from_f64(1.25),
    };

    // Auto-generated verify_integrity() method
    assert!(
        payment.verify_integrity(),
        "Integrity check should pass for valid capsule"
    );
}

#[test]
fn test_auto_crc_different_values() {
    let payment1 = PaymentCapsule {
        amount: Q16_16::from_f64(100.0),
        fee: Q16_16::from_f64(2.50),
    };

    let payment2 = PaymentCapsule {
        amount: Q16_16::from_f64(200.0), // Different amount
        fee: Q16_16::from_f64(2.50),
    };

    // Different values → different checksums
    assert_ne!(
        payment1.compute_checksum(),
        payment2.compute_checksum(),
        "Different values must produce different CRC32"
    );
}

// Test 2: Hash chain verification with prev_hash
#[derive(CapsuleSerialize)]
#[capsule_serialize(auto_crc = true)]
#[repr(C, align(256))]
struct AuditCapsule {
    amount: Q16_16,
    fee: Q16_16,

    #[capsule_serialize(prev_hash)]
    prev_hash: u64,
}

#[test]
fn test_hash_chain_verify() {
    // First capsule in chain
    let capsule1 = AuditCapsule {
        amount: Q16_16::from_f64(100.0),
        fee: Q16_16::from_f64(2.50),
        prev_hash: 0, // Genesis block
    };

    // Second capsule references first
    let capsule2 = AuditCapsule {
        amount: Q16_16::from_f64(200.0),
        fee: Q16_16::from_f64(5.00),
        prev_hash: capsule1.compute_hash(), // Hash chain link
    };

    // Auto-generated verify_chain() method
    assert!(
        capsule2.verify_chain(&capsule1),
        "Hash chain should be valid"
    );
}

#[test]
fn test_hash_chain_broken() {
    let capsule1 = AuditCapsule {
        amount: Q16_16::from_f64(100.0),
        fee: Q16_16::from_f64(2.50),
        prev_hash: 0,
    };

    // Capsule with invalid prev_hash (tampering detected)
    let capsule2 = AuditCapsule {
        amount: Q16_16::from_f64(200.0),
        fee: Q16_16::from_f64(5.00),
        prev_hash: 0xDEADBEEF, // Invalid hash
    };

    // Hash chain should be broken
    assert!(
        !capsule2.verify_chain(&capsule1),
        "Broken hash chain should be detected"
    );
}

#[test]
fn test_hash_chain_multi_link() {
    // Create a 3-capsule chain
    let capsule1 = AuditCapsule {
        amount: Q16_16::from_f64(100.0),
        fee: Q16_16::from_f64(2.50),
        prev_hash: 0,
    };

    let capsule2 = AuditCapsule {
        amount: Q16_16::from_f64(200.0),
        fee: Q16_16::from_f64(5.00),
        prev_hash: capsule1.compute_hash(),
    };

    let capsule3 = AuditCapsule {
        amount: Q16_16::from_f64(300.0),
        fee: Q16_16::from_f64(7.50),
        prev_hash: capsule2.compute_hash(),
    };

    // Verify each link
    assert!(capsule2.verify_chain(&capsule1), "Link 1->2 should be valid");
    assert!(capsule3.verify_chain(&capsule2), "Link 2->3 should be valid");
}

// Test 3: Zero boilerplate validation
#[test]
fn test_zero_boilerplate() {
    // No manual CRC code needed - everything auto-generated!
    let payment = PaymentCapsule {
        amount: Q16_16::from_f64(999.99),
        fee: Q16_16::from_f64(19.99),
    };

    // Auto-generated methods available:
    let _checksum = payment.compute_checksum();
    let _integrity = payment.verify_integrity();
    let _hash = payment.compute_hash();

    // Success! Zero boilerplate achieved
}

// Test 4: Q34 Auditability compliance
#[test]
fn test_q34_auditability() {
    // Create audit trail
    let mut chain: Vec<AuditCapsule> = Vec::new();

    // Genesis block
    chain.push(AuditCapsule {
        amount: Q16_16::from_f64(100.0),
        fee: Q16_16::from_f64(2.50),
        prev_hash: 0,
    });

    // Add 10 transactions
    for i in 1..=10 {
        let prev_hash = chain[i - 1].compute_hash();
        chain.push(AuditCapsule {
            amount: Q16_16::from_f64(100.0 * (i + 1) as f64),
            fee: Q16_16::from_f64(2.50 * (i + 1) as f64),
            prev_hash,
        });
    }

    // Verify entire chain
    for i in 1..chain.len() {
        assert!(
            chain[i].verify_chain(&chain[i - 1]),
            "Audit trail link {} should be valid",
            i
        );
    }

    // Q34 Compliance:
    // ✅ Hash chain integrity: All links verified
    // ✅ Tamper detection: verify_chain() catches invalid links
    // ✅ Reproducibility: Same data → same hashes
    // ✅ Auditability: Complete transaction history preserved
}

// Test 5: CRC32 collision resistance (basic)
#[test]
fn test_crc32_collision_resistance() {
    let mut checksums = std::collections::HashSet::new();

    // Generate 1000 different payments
    for i in 0..1000 {
        let payment = PaymentCapsule {
            amount: Q16_16::from_f64((i as f64) * 1.23),
            fee: Q16_16::from_f64((i as f64) * 0.05),
        };
        let checksum = payment.compute_checksum();

        // No collisions expected for different values
        assert!(
            checksums.insert(checksum),
            "CRC32 collision detected at iteration {}",
            i
        );
    }

    assert_eq!(checksums.len(), 1000, "All 1000 checksums should be unique");
}
