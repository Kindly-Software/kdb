//! Minimal heap corruption test for T5 Streaming Pipeline

use kindly_dedup::{generate_synthetic_corpus, StreamingDedupPipeline};

#[test]
#[ignore] // Run manually: cargo test --test heap_corruption_test --features benchmarking -- --ignored
fn test_progressive_sizes() {
    let sizes = vec![1000, 5000, 10000, 50000, 100000, 500000, 1000000];

    for size in sizes {
        eprintln!("\n=== Testing {} documents ===", size);

        eprintln!("  [1/3] Generating corpus...");
        let corpus = generate_synthetic_corpus(size);

        eprintln!("  [2/3] Converting to (id, text) format...");
        let docs: Vec<(usize, String)> = corpus.iter().map(|d| (d.id, d.text.clone())).collect();

        eprintln!("  [3/3] Creating pipeline and adding documents...");
        let mut pipeline = StreamingDedupPipeline::new(size, 16).unwrap();

        match pipeline.add_documents(docs) {
            Ok(_) => {
                let metrics = pipeline.metrics();
                eprintln!(
                    "  ✅ SUCCESS: {} docs (tokenized: {}, skipped: {})",
                    size, metrics.documents_tokenized, metrics.documents_skipped
                );
            }
            Err(e) => {
                panic!("❌ FAILED at {} documents: {:?}", size, e);
            }
        }
    }

    eprintln!("\n✅ All tests passed!");
}
