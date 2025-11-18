use kindly_dedup::{generate_synthetic_corpus, StreamingDedupPipeline};

fn main() {
    println!("Generating 10K corpus...");
    let corpus = generate_synthetic_corpus(10_000);
    let documents: Vec<(usize, String)> = corpus.iter().map(|doc| (doc.id, doc.text.clone())).collect();

    println!("Testing T5 with 10K documents...");
    let mut pipeline = StreamingDedupPipeline::new(10_000, 16).unwrap();

    let start = std::time::Instant::now();
    pipeline.add_documents(documents).unwrap();
    let add_time = start.elapsed();

    let start_find = std::time::Instant::now();
    let clusters = pipeline.find_duplicates(0.85).unwrap();
    let find_time = start_find.elapsed();

    let metrics = pipeline.metrics();
    let total_time = add_time + find_time;
    let throughput = 10_000.0 / total_time.as_secs_f64();

    println!("\n=== T5 Streaming 10K Results ===");
    println!("Add phase: {:.3}s", add_time.as_secs_f64());
    println!("Find phase: {:.3}s", find_time.as_secs_f64());
    println!("Total: {:.3}s", total_time.as_secs_f64());
    println!("Throughput: {:.0} docs/sec", throughput);
    println!("Ingested: {}", metrics.documents_ingested);
    println!("Tokenized: {}", metrics.documents_tokenized);
    println!("Skipped (Bloom): {}", metrics.documents_skipped);
    println!("Signatures: {}", metrics.signatures_computed);
    println!("Clusters: {}", clusters.len());
    println!(
        "Panics: tok={}, min={}, lsh={}, ver={}",
        metrics.tokenization_panics, metrics.minhash_panics, metrics.lsh_panics, metrics.verification_panics
    );
}
