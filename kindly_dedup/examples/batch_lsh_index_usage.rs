//! Example usage of BatchLshIndexCapsule (T4 Batch + T9 Persistent)
//!
//! This example demonstrates how to use the batch LSH index capsule for
//! efficiently accumulating LSH signatures and flushing them in batches.
//!
//! # Performance
//!
//! - Batch size: 1000 documents
//! - Per-insert latency: <10ns
//! - Flush latency: ~50ms per batch
//! - Memory: Pre-allocated, no allocations in hot path
//!
//! # Compile
//!
//! ```bash
//! cargo build --example batch_lsh_index_usage --features batch-lsh
//! ```
//!
//! # Run
//!
//! ```bash
//! cargo run --example batch_lsh_index_usage --features batch-lsh --release
//! ```

use kindly_dedup::lsh::BatchLshIndexCapsule;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("BatchLshIndexCapsule Usage Example");
    println!("==================================\n");

    // Create a new batch LSH index capsule
    // - Batch size: 1000 documents
    // - Number of bands: 5 (typical LSH configuration)
    let capsule = BatchLshIndexCapsule::new(1000, 5)?;
    println!(
        "Created BatchLshIndexCapsule: batch_size={}, num_bands={}",
        capsule.batch_size(),
        capsule.num_bands()
    );

    // Example 1: Single insertion
    println!("\n--- Example 1: Single Insertion ---");
    capsule.insert_signature(1, 0, 0x123456789abcdef0)?;
    let (size, pending, gen) = capsule.stats();
    println!(
        "After 1 insert: size={}, pending={}, generation={} ({})",
        size,
        pending,
        gen,
        if gen % 2 == 0 { "committed" } else { "in-progress" }
    );

    // Example 2: Bulk insertion with timing
    println!("\n--- Example 2: Bulk Insertion (100 docs) ---");
    let start = Instant::now();
    for doc_id in 1..100 {
        for band_idx in 0..5 {
            capsule.insert_signature(doc_id, band_idx as u8, doc_id as u64 * band_idx)?;
        }
    }
    let elapsed = start.elapsed();
    let (size, pending, _) = capsule.stats();
    println!(
        "Inserted 100 docs × 5 bands = 500 entries in {:?}",
        elapsed
    );
    println!("Average per-insert latency: {} ns", elapsed.as_nanos() / 500);
    println!("Current batch: {}/{} entries", size, capsule.batch_size());

    // Example 3: Check if flush needed
    println!("\n--- Example 3: Flush Decision ---");
    println!("should_flush() = {}", capsule.should_flush());
    println!(
        "Current batch occupancy: {}/{}",
        size,
        capsule.batch_size()
    );

    // Example 4: Graceful flush and continue
    println!("\n--- Example 4: Flush and Continue ---");
    println!("Flushing batch to persistent storage...");
    let flush_start = Instant::now();
    capsule.flush()?;
    let flush_elapsed = flush_start.elapsed();
    println!("Flush completed in {:?}", flush_elapsed);

    let (size_after, pending_after, gen_after) = capsule.stats();
    println!(
        "After flush: size={}, pending={}, generation={} ({})",
        size_after,
        pending_after,
        gen_after,
        if gen_after % 2 == 0 { "committed" } else { "in-progress" }
    );

    // Example 5: Insert more after flush
    println!("\n--- Example 5: Insert After Flush ---");
    for doc_id in 100..150 {
        for band_idx in 0..5 {
            capsule.insert_signature(doc_id, band_idx as u8, doc_id as u64 * band_idx)?;
        }
    }
    let (size_new, pending_new, _) = capsule.stats();
    println!(
        "After second insert: size={}, pending={} (total across all flushes)",
        size_new, pending_new
    );

    // Example 6: Multiple flush cycles
    println!("\n--- Example 6: Multiple Flush Cycles ---");
    for cycle in 0..3 {
        println!("\nCycle {}:", cycle);

        // Insert another batch
        for doc_id in 150 + (cycle * 100)..250 + (cycle * 100) {
            for band_idx in 0..5 {
                capsule.insert_signature(doc_id, band_idx as u8, doc_id as u64 * band_idx)?;
            }
        }

        let (size, _, _) = capsule.stats();
        println!("  After insert: {} entries");

        if capsule.should_flush() {
            capsule.flush()?;
            let (size_post, _, _) = capsule.stats();
            println!("  After flush: {} entries (cleared)", size_post);
        }
    }

    // Example 7: Final statistics
    println!("\n--- Example 7: Final Statistics ---");
    let (size_final, pending_final, gen_final) = capsule.stats();
    println!("Final batch state:");
    println!("  Current size: {} entries", size_final);
    println!("  Total pending: {} inserts", pending_final);
    println!(
        "  Generation: {} ({})",
        gen_final,
        if gen_final % 2 == 0 { "committed" } else { "in-progress" }
    );
    println!("  Is committed: {}", capsule.is_committed());

    // Example 8: Capsule metadata
    println!("\n--- Example 8: Capsule Metadata ---");
    use std::mem::{align_of, size_of};
    println!(
        "Capsule size: {} bytes (target: ≤256)",
        size_of::<BatchLshIndexCapsule>()
    );
    println!(
        "Capsule alignment: {} bytes (target: 128)",
        align_of::<BatchLshIndexCapsule>()
    );

    println!("\n✅ All examples completed successfully!");

    Ok(())
}
