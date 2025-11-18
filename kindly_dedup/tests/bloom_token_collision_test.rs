//! Debug test to understand token collision in Bloom filter
//!
//! Root cause investigation: Why 100% false positive rate?

use kindly_dedup::bloom_sharded::ShardedDedupBloomFilter;

#[test]
fn test_token_extraction() {
    println!("\n=== TOKEN EXTRACTION TEST ===\n");

    // Test documents
    let doc1 = "Unique document number 0 with completely distinct content xyz";
    let doc2 = "Different unique document number 100 with completely distinct content abc";

    // Extract tokens (same logic as Bloom filter)
    let prefix1: String = doc1.chars().take(100).collect();
    let tokens1: Vec<&str> = prefix1.split_whitespace().collect();

    let prefix2: String = doc2.chars().take(100).collect();
    let tokens2: Vec<&str> = prefix2.split_whitespace().collect();

    println!("Doc1 tokens: {:?}", tokens1);
    println!("Doc2 tokens: {:?}", tokens2);

    // Find common tokens
    let mut common_tokens = Vec::new();
    for t1 in &tokens1 {
        if tokens2.contains(t1) {
            common_tokens.push(*t1);
        }
    }

    println!("\nCommon tokens: {:?}", common_tokens);
    println!("Common token count: {}/{}", common_tokens.len(), tokens1.len());

    // The bug: If there are ANY common tokens, Bloom will return true!
    // Expected common tokens: "document", "number", "with", "completely", "distinct", "content"
    // That's 6 out of ~10 tokens = HIGH collision rate
}

#[test]
fn test_bloom_with_zero_common_tokens() {
    println!("\n=== ZERO COMMON TOKENS TEST ===\n");

    let filter = ShardedDedupBloomFilter::new();

    // Insert document with UNIQUE tokens
    let doc1 = "aaa bbb ccc ddd eee fff ggg hhh";
    filter.insert(0, doc1);
    println!("Inserted: {}", doc1);

    // Query document with COMPLETELY DIFFERENT tokens
    let doc2 = "xxx yyy zzz www vvv uuu ttt sss";
    let result = filter.query(1, doc2);
    println!("Queried: {} → {}", doc2, result);

    assert!(
        !result,
        "Bloom filter should return false for documents with ZERO common tokens"
    );

    println!("\n✅ PASS: Bloom correctly returned false for zero common tokens");
}

#[test]
fn test_bloom_with_one_common_token() {
    println!("\n=== ONE COMMON TOKEN TEST ===\n");

    let filter = ShardedDedupBloomFilter::new();

    // Insert document
    let doc1 = "aaa bbb ccc ddd eee fff ggg hhh";
    filter.insert(0, doc1);
    println!("Inserted: {}", doc1);

    // Query document with ONE common token
    let doc2 = "xxx yyy zzz aaa www vvv uuu ttt"; // "aaa" is common
    let result = filter.query(1, doc2);
    println!(
        "Queried: {} → {} (expected: true, one common token 'aaa')",
        doc2, result
    );

    assert!(
        result,
        "Bloom filter should return true when ANY token is common (correct behavior)"
    );

    println!("\n✅ PASS: Bloom correctly returned true for one common token");
}
