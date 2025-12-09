//! Manual test/demo for WeightAuditCapsule

use atomic_capsule::primitives::inference::weight_audit::{WeightAuditCapsule, fnv1a_hash};

fn main() {
    println!("Testing WeightAuditCapsule...\n");

    // Test 1: Size and alignment
    println!("Test 1: Capsule size and alignment");
    let size = std::mem::size_of::<WeightAuditCapsule>();
    let align = std::mem::align_of::<WeightAuditCapsule>();
    println!("  Size: {} bytes (expected: 128)", size);
    println!("  Alignment: {} bytes (expected: 128)", align);
    assert_eq!(size, 128, "Size must be 128 bytes");
    assert_eq!(align, 128, "Alignment must be 128 bytes");
    println!("  ✓ PASSED\n");

    // Test 2: FNV-1a hash correctness
    println!("Test 2: FNV-1a hash correctness");
    let data1 = b"hello world";
    let hash1 = fnv1a_hash(data1);
    let hash1_repeat = fnv1a_hash(data1);
    assert_eq!(hash1, hash1_repeat, "Hash must be deterministic");
    let data2 = b"hello world!";
    let hash2 = fnv1a_hash(data2);
    assert_ne!(hash1, hash2, "Different data must produce different hash");
    println!("  Hash1: {:016x}", hash1);
    println!("  Hash2: {:016x}", hash2);
    println!("  ✓ PASSED\n");

    // Test 3: Block verification success
    println!("Test 3: Block verification (success)");
    let mut audit = WeightAuditCapsule::new();
    let block0_data = b"block 0 data";
    let block1_data = b"block 1 data";
    let expected_hashes = vec![
        fnv1a_hash(block0_data),
        fnv1a_hash(block1_data),
    ];
    audit.set_expected_hashes(&expected_hashes).unwrap();
    assert!(audit.verify_block(0, block0_data).unwrap());
    assert!(audit.verify_block(1, block1_data).unwrap());
    println!("  ✓ PASSED\n");

    // Test 4: Block verification failure
    println!("Test 4: Block verification (failure)");
    let wrong_data = b"wrong data";
    let result = audit.verify_block(0, wrong_data);
    assert!(result.is_err(), "Wrong data should fail verification");
    println!("  Error: {}", result.unwrap_err());
    println!("  ✓ PASSED\n");

    // Test 5: Chain hash accumulation
    println!("Test 5: Chain hash accumulation");
    let audit2 = WeightAuditCapsule::new();
    let initial_hash = audit2.get_chain_hash();
    let block0_hash = fnv1a_hash(b"block 0");
    let chain1 = audit2.update_chain_hash(block0_hash);
    let block1_hash = fnv1a_hash(b"block 1");
    let chain2 = audit2.update_chain_hash(block1_hash);
    assert_ne!(chain1, initial_hash);
    assert_ne!(chain2, chain1);
    println!("  Initial: {:016x}", initial_hash);
    println!("  After block 0: {:016x}", chain1);
    println!("  After block 1: {:016x}", chain2);
    println!("  ✓ PASSED\n");

    // Test 6: Verification bitmap
    println!("Test 6: Verification bitmap tracking");
    let mut audit3 = WeightAuditCapsule::new();
    let hashes = vec![1u64, 2u64, 3u64];
    audit3.set_expected_hashes(&hashes).unwrap();
    assert!(!audit3.is_verified(0));
    audit3.mark_verified(0).unwrap();
    assert!(audit3.is_verified(0));
    assert!(!audit3.is_verified(1));
    assert_eq!(audit3.verified_count(), 1);
    audit3.mark_verified(2).unwrap();
    assert!(audit3.is_verified(2));
    assert_eq!(audit3.verified_count(), 2);
    println!("  Verified count: {}", audit3.verified_count());
    println!("  ✓ PASSED\n");

    // Test 7: Merkle root verification
    println!("Test 7: Merkle root verification");
    let mut audit4 = WeightAuditCapsule::new();
    let merkle_root: u128 = 0x123456789ABCDEF0_FEDCBA9876543210;
    audit4.set_merkle_root(merkle_root);
    assert!(audit4.verify_merkle_root(merkle_root));
    assert!(!audit4.verify_merkle_root(merkle_root + 1));
    println!("  Merkle root: {:032x}", merkle_root);
    println!("  ✓ PASSED\n");

    // Test 8: Metrics
    println!("Test 8: Audit metrics");
    let metrics = audit3.metrics();
    println!("  Verified count: {}", metrics.verified_count);
    println!("  Total count: {}", metrics.total_count);
    println!("  Chain hash: {:016x}", metrics.chain_hash);
    println!("  Phase: {}", metrics.phase);
    println!("  Generation: {}", metrics.generation);
    println!("  ✓ PASSED\n");

    // Test 9: Snapshot
    println!("Test 9: Capsule snapshot");
    let snapshot = audit3.snapshot();
    println!("  State: {:016x}", snapshot.state);
    println!("  Chain hash: {:016x}", snapshot.chain_hash);
    println!("  Verification bitmap: {:064b}", snapshot.verification_bitmap);
    println!("  ✓ PASSED\n");

    println!("✅ All 9 T28 tests passed!");
}
