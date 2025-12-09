// Minimal reproduction of the duplicate token bug

use std::collections::HashSet;

/// Simulate ExactJaccardComputer (HashSet-based)
fn exact_jaccard(text1: &str, text2: &str) -> f64 {
    let tokens1: HashSet<String> = text1
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .collect();
    let tokens2: HashSet<String> = text2
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .collect();

    if tokens1.is_empty() && tokens2.is_empty() {
        return 1.0;
    }

    let intersection = tokens1.intersection(&tokens2).count();
    let union = tokens1.union(&tokens2).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Simulate SimdJaccardComputer (sorted merge on token IDs)
fn simd_jaccard(text1: &str, text2: &str) -> f64 {
    // Encode tokens to IDs (NO deduplication!)
    let mut dictionary = std::collections::HashMap::new();
    let mut next_id = 0u32;

    let encode = |token: &str, dict: &mut std::collections::HashMap<String, u32>, next: &mut u32| -> u32 {
        if let Some(&id) = dict.get(token) {
            id
        } else {
            let id = *next;
            dict.insert(token.to_string(), id);
            *next += 1;
            id
        }
    };

    let mut tokens1: Vec<u32> = text1
        .split_whitespace()
        .map(|s| encode(&s.to_lowercase(), &mut dictionary, &mut next_id))
        .collect();

    let mut tokens2: Vec<u32> = text2
        .split_whitespace()
        .map(|s| encode(&s.to_lowercase(), &mut dictionary, &mut next_id))
        .collect();

    tokens1.sort_unstable();
    tokens2.sort_unstable();

    if tokens1.is_empty() && tokens2.is_empty() {
        return 1.0;
    }

    // Sorted merge for intersection count
    let mut i = 0;
    let mut j = 0;
    let mut intersection = 0;

    while i < tokens1.len() && j < tokens2.len() {
        if tokens1[i] == tokens2[j] {
            intersection += 1;
            i += 1;
            j += 1;
        } else if tokens1[i] < tokens2[j] {
            i += 1;
        } else {
            j += 1;
        }
    }

    let union = tokens1.len() + tokens2.len() - intersection;

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

fn main() {
    // Test case 1: Document with repeated tokens
    let doc1 = "the quick brown fox the";
    let doc2 = "the lazy dog";

    let exact = exact_jaccard(doc1, doc2);
    let simd = simd_jaccard(doc1, doc2);

    println!("Test 1: Repeated tokens");
    println!("  Doc1: '{}'", doc1);
    println!("  Doc2: '{}'", doc2);
    println!("  Exact Jaccard (HashSet): {:.4}", exact);
    println!("  SIMD Jaccard (sorted merge): {:.4}", simd);
    println!("  Match: {}", if (exact - simd).abs() < 0.001 { "✓" } else { "✗ BUG!" });
    println!();

    // Test case 2: No repeated tokens
    let doc3 = "the quick brown fox";
    let doc4 = "the lazy dog";

    let exact2 = exact_jaccard(doc3, doc4);
    let simd2 = simd_jaccard(doc3, doc4);

    println!("Test 2: No repeated tokens");
    println!("  Doc3: '{}'", doc3);
    println!("  Doc4: '{}'", doc4);
    println!("  Exact Jaccard (HashSet): {:.4}", exact2);
    println!("  SIMD Jaccard (sorted merge): {:.4}", simd2);
    println!("  Match: {}", if (exact2 - simd2).abs() < 0.001 { "✓" } else { "✗ BUG!" });
    println!();

    // Test case 3: Many repeated tokens
    let doc5 = "the the the quick quick brown";
    let doc6 = "the quick lazy";

    let exact3 = exact_jaccard(doc5, doc6);
    let simd3 = simd_jaccard(doc5, doc6);

    println!("Test 3: Many repeated tokens");
    println!("  Doc5: '{}'", doc5);
    println!("  Doc6: '{}'", doc6);
    println!("  Exact Jaccard (HashSet): {:.4}", exact3);
    println!("  SIMD Jaccard (sorted merge): {:.4}", simd3);
    println!("  Match: {}", if (exact3 - simd3).abs() < 0.001 { "✓" } else { "✗ BUG!" });
}
