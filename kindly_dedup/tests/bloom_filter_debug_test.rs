//! Minimal test to debug Bloom filter false positive rate bug (UCE-D7)
//!
//! **Problem**: 99.9998% false positive rate on 1M corpus
//! **Expected**: <1% false positive rate
//! **Hypothesis**: Hash collision or semantic confusion

use kindly_dedup::bloom_sharded::ShardedDedupBloomFilter;

#[test]
fn test_bloom_false_positive_rate_with_unique_docs() {
    println!("\n=== BLOOM FILTER DEBUG TEST ===\n");

    let filter = ShardedDedupBloomFilter::new();

    // Insert 100 unique documents
    println!("Inserting 100 unique documents...");
    for i in 0..100 {
        let text = format!("Unique document number {} with completely distinct content xyz", i);
        filter.insert(i, &text);
    }

    // Query 100 DIFFERENT unique documents (should all be NOT found)
    println!("Querying 100 different unique documents...");
    let mut false_positives = 0;
    let mut true_negatives = 0;

    for i in 100..200 {
        let text = format!(
            "Different unique document number {} with completely distinct content abc",
            i
        );
        if filter.query(i, &text) {
            false_positives += 1;
            println!("  FALSE POSITIVE: Doc {} wrongly detected as duplicate", i);
        } else {
            true_negatives += 1;
        }
    }

    let fp_rate = false_positives as f64 / 100.0;
    println!("\n=== RESULTS ===");
    println!("False positives: {}/100 ({:.2}%)", false_positives, fp_rate * 100.0);
    println!(
        "True negatives: {}/100 ({:.2}%)",
        true_negatives,
        (1.0 - fp_rate) * 100.0
    );

    // Get Bloom filter's internal metrics
    let (checked, skipped, skip_rate) = filter.audit_metrics();
    println!("\nBloom filter internal metrics:");
    println!("  checked: {}", checked);
    println!("  skipped: {}", skipped);
    println!("  skip_rate: {:.4}%", skip_rate * 100.0);

    // Assertion: FP rate should be <1%
    assert!(
        fp_rate < 0.01,
        "FAIL: False positive rate {:.2}% exceeds 1%",
        fp_rate * 100.0
    );

    println!(
        "\n✅ PASS: False positive rate {:.2}% is within expected range (<1%)",
        fp_rate * 100.0
    );
}

#[test]
fn test_bloom_duplicate_detection() {
    println!("\n=== BLOOM DUPLICATE DETECTION TEST ===\n");

    let filter = ShardedDedupBloomFilter::new();

    // Insert 10 documents
    println!("Inserting 10 documents...");
    for i in 0..10 {
        let text = format!("Document {} content", i);
        filter.insert(i, &text);
    }

    // Query same 10 documents (should all be found as duplicates)
    println!("Querying same 10 documents...");
    let mut duplicates_found = 0;

    for i in 0..10 {
        let text = format!("Document {} content", i);
        if filter.query(i + 1000, &text) {
            // Different doc_id, same content
            duplicates_found += 1;
        } else {
            println!("  MISS: Doc {} with same content not detected as duplicate", i);
        }
    }

    let detection_rate = duplicates_found as f64 / 10.0;
    println!("\n=== RESULTS ===");
    println!(
        "Duplicates found: {}/10 ({:.2}%)",
        duplicates_found,
        detection_rate * 100.0
    );

    // Assertion: Should detect 100% of duplicates (zero false negatives)
    assert_eq!(
        duplicates_found,
        10,
        "FAIL: Bloom filter missed {} duplicates (should have ZERO false negatives)",
        10 - duplicates_found
    );

    println!("\n✅ PASS: Detected 100% of duplicates (zero false negatives)");
}

#[test]
fn test_bloom_semantic_check() {
    println!("\n=== BLOOM SEMANTIC CHECK ===\n");

    let filter = ShardedDedupBloomFilter::new();

    // Test 1: Empty filter, query unseen document
    let unseen_result = filter.query(0, "unseen document");
    println!("Empty filter, query unseen: {} (expect: false)", unseen_result);
    assert!(
        !unseen_result,
        "Empty Bloom filter should return false for unseen document"
    );

    // Test 2: Insert document, query same document
    filter.insert(0, "test document");
    let seen_result = filter.query(0, "test document");
    println!("After insert, query same: {} (expect: true)", seen_result);
    assert!(seen_result, "Bloom filter should return true for inserted document");

    // Test 3: Query different document
    let different_result = filter.query(1, "completely different content xyz");
    println!("After insert, query different: {} (expect: false)", different_result);

    // Note: This might be false OR true (false positive). We'll check the rate separately.

    println!("\n✅ PASS: Basic semantics are correct");
}
