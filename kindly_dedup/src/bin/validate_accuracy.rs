//! Validation binary for F1 score and recall measurement
//!
//! Measures duplicate detection accuracy on synthetic corpus with known ground truth.

use anyhow::Result;
use kindly_dedup::DedupPipeline;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Document {
    id: usize,
    url: String,
    text: String,
}

/// Ground truth from synthetic corpus generator
/// - IDs 0-79,999: Unique documents
/// - IDs 80,000-84,998: Exact duplicates (5 groups of 999 duplicates each + 1 original)
/// - IDs 85,000-99,998: Near-duplicates (15 groups of 999 near-dups each + 1 original)
fn get_ground_truth() -> HashMap<usize, HashSet<usize>> {
    let mut ground_truth: HashMap<usize, HashSet<usize>> = HashMap::new();

    // Exact duplicates (IDs 80,000-84,998)
    // Pattern: doc 80000 is original, 80001-80999 are copies
    for group in 0..5 {
        let base = 80_000 + group * 1000;
        let mut cluster = HashSet::new();
        for offset in 0..1000 {
            cluster.insert(base + offset);
        }
        ground_truth.insert(base, cluster);
    }

    // Near-duplicates (IDs 85,000-99,998)
    // Pattern: doc 85000 is original, 85001-85999 are near-dups
    for group in 0..15 {
        let base = 85_000 + group * 1000;
        let mut cluster = HashSet::new();
        for offset in 0..1000 {
            cluster.insert(base + offset);
        }
        ground_truth.insert(base, cluster);
    }

    ground_truth
}

fn calculate_metrics(
    predicted_clusters: &[Vec<usize>],
    ground_truth: &HashMap<usize, HashSet<usize>>,
) -> (f64, f64, f64) {
    let mut true_positives = 0usize;
    let mut false_positives = 0usize;
    let mut false_negatives = 0usize;

    // Convert predicted to pairs
    let mut predicted_pairs: HashSet<(usize, usize)> = HashSet::new();
    for cluster in predicted_clusters {
        for i in 0..cluster.len() {
            for j in i + 1..cluster.len() {
                let pair = if cluster[i] < cluster[j] {
                    (cluster[i], cluster[j])
                } else {
                    (cluster[j], cluster[i])
                };
                predicted_pairs.insert(pair);
            }
        }
    }

    // Convert ground truth to pairs
    let mut truth_pairs: HashSet<(usize, usize)> = HashSet::new();
    for (_base, cluster) in ground_truth.iter() {
        let cluster_vec: Vec<_> = cluster.iter().copied().collect();
        for i in 0..cluster_vec.len() {
            for j in i + 1..cluster_vec.len() {
                let pair = if cluster_vec[i] < cluster_vec[j] {
                    (cluster_vec[i], cluster_vec[j])
                } else {
                    (cluster_vec[j], cluster_vec[i])
                };
                truth_pairs.insert(pair);
            }
        }
    }

    // Calculate TP, FP, FN
    for pair in &predicted_pairs {
        if truth_pairs.contains(pair) {
            true_positives += 1;
        } else {
            false_positives += 1;
        }
    }

    for pair in &truth_pairs {
        if !predicted_pairs.contains(pair) {
            false_negatives += 1;
        }
    }

    // Calculate metrics
    let precision = if true_positives + false_positives > 0 {
        true_positives as f64 / (true_positives + false_positives) as f64
    } else {
        0.0
    };

    let recall = if true_positives + false_negatives > 0 {
        true_positives as f64 / (true_positives + false_negatives) as f64
    } else {
        0.0
    };

    let f1 = if precision + recall > 0.0 {
        2.0 * (precision * recall) / (precision + recall)
    } else {
        0.0
    };

    (precision, recall, f1)
}

fn main() -> Result<()> {
    println!("=== kindly_dedup Validation Suite ===\n");

    // Get test size from env or default to 10K
    let test_size: usize = std::env::var("TEST_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000);

    // Load corpus
    println!("Loading 100K synthetic corpus...");
    let start = Instant::now();
    let file = File::open("test_data/synthetic_100k.json")?;
    let all_documents: Vec<Document> = serde_json::from_reader(file)?;
    println!("Loaded {} documents in {:?}", all_documents.len(), start.elapsed());

    // Take subset for testing (to avoid OOM)
    // Strategy: Take documents from duplicate range (80K-100K) to have actual ground truth
    let documents: Vec<Document> = if test_size < all_documents.len() {
        // Take last N documents (which include duplicates starting at 80K)
        let len = all_documents.len();
        all_documents.into_iter().skip(len - test_size).collect()
    } else {
        all_documents
    };
    println!(
        "Using {} documents for validation (IDs {}-{})\n",
        documents.len(),
        documents.first().map(|d| d.id).unwrap_or(0),
        documents.last().map(|d| d.id).unwrap_or(0)
    );

    // Get ground truth
    let ground_truth = get_ground_truth();
    println!("Ground truth: {} duplicate clusters\n", ground_truth.len());

    // Test multiple thresholds
    for threshold in [0.70, 0.85, 0.95] {
        println!("--- Threshold: {:.2} ---", threshold);

        // Create pipeline
        let mut pipeline = DedupPipeline::new(documents.len());

        // Add all documents
        let start = Instant::now();
        for doc in &documents {
            pipeline.add_document(doc.id, &doc.text);
        }
        let add_time = start.elapsed();

        // Find duplicates
        let start = Instant::now();
        let clusters = pipeline.find_duplicates(threshold);
        let dedup_time = start.elapsed();

        // Calculate metrics
        let (precision, recall, f1) = calculate_metrics(&clusters, &ground_truth);

        println!("Performance:");
        println!(
            "  Add time:      {:?} ({:.2}μs/doc)",
            add_time,
            add_time.as_micros() as f64 / documents.len() as f64
        );
        println!(
            "  Dedup time:    {:?} ({:.2}μs/doc)",
            dedup_time,
            dedup_time.as_micros() as f64 / documents.len() as f64
        );
        println!(
            "  Total:         {:?} ({:.2}μs/doc)",
            add_time + dedup_time,
            (add_time + dedup_time).as_micros() as f64 / documents.len() as f64
        );

        println!("\nAccuracy:");
        println!("  Precision:     {:.2}%", precision * 100.0);
        println!("  Recall:        {:.2}%", recall * 100.0);
        println!("  F1 Score:      {:.2}%", f1 * 100.0);

        println!("\nClusters:");
        println!("  Total clusters: {}", clusters.len());
        println!(
            "  Duplicate clusters (size >1): {}",
            clusters.iter().filter(|c| c.len() > 1).count()
        );
        println!(
            "  Largest cluster: {}",
            clusters.iter().map(|c| c.len()).max().unwrap_or(0)
        );

        // Validate targets
        println!("\nTarget Validation:");
        let latency_us = (add_time + dedup_time).as_micros() as f64 / documents.len() as f64;
        let latency_check = if latency_us < 1000.0 { "PASS" } else { "FAIL" };
        let f1_check = if f1 >= 0.90 { "PASS" } else { "FAIL" };
        let recall_check = if recall >= 0.92 { "PASS" } else { "FAIL" };

        println!("  Latency <1ms/doc: {} (actual: {:.2}μs)", latency_check, latency_us);
        println!("  F1 ≥90%: {} (actual: {:.2}%)", f1_check, f1 * 100.0);
        println!("  Recall ≥92%: {} (actual: {:.2}%)", recall_check, recall * 100.0);

        println!();
    }

    Ok(())
}
