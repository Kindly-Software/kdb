//! Timeline Aggregation Demo
//!
//! Demonstrates TimelineAggregationCapsule (T4 Batch tier) usage for
//! efficient event aggregation and analytics.

use clapi_core::capsules::{TimelineAggregationCapsule, BucketGranularity};
use clapi_core::proxy::TimelineBridge;
use std::sync::Arc;

/// Example 1: Direct capsule usage (sync)
fn example_direct_capsule() {
    println!("\n=== Example 1: Direct Capsule Usage ===\n");

    // Create timeline: 1000 epoch seconds, minute buckets, 1000 capacity
    let timeline = TimelineAggregationCapsule::new(
        1000,
        BucketGranularity::Minute,
        1000,
    );

    // Append events
    for i in 0..100 {
        let timestamp = 1000 + i * 30; // Every 30 seconds
        timeline.append(timestamp).unwrap();
    }

    println!("Total events: {}", timeline.total_events());
    println!("Current head: {}", timeline.head());

    // Query first bucket
    match timeline.query_bucket(0) {
        Ok(snapshot) => {
            println!("\nBucket 0:");
            println!("  Time range: {} - {}", snapshot.start_ts, snapshot.end_ts);
            println!("  Event count: {}", snapshot.event_count);
            println!("  Status: {:?}", snapshot.status);
        }
        Err(e) => println!("Query error: {}", e),
    }

    // Flush bucket (compute hash)
    match timeline.flush_bucket(0) {
        Ok(hash) => println!("\nBucket 0 flushed with hash: 0x{:x}", hash),
        Err(e) => println!("Flush error: {}", e),
    }
}

/// Example 2: Async bridge usage
#[tokio::main]
async fn example_async_bridge() {
    println!("\n=== Example 2: Async Bridge Usage ===\n");

    // Create async bridge
    let bridge = Arc::new(TimelineBridge::new(
        1000,
        BucketGranularity::Hour,
        100,
    ));

    // Spawn concurrent appenders
    let mut handles = vec![];
    for thread_id in 0..4 {
        let bridge = Arc::clone(&bridge);
        handles.push(tokio::spawn(async move {
            for i in 0..25 {
                let timestamp = 1000 + thread_id * 3600 + i * 60; // Different hours
                bridge.append_event(timestamp).await.unwrap();
            }
        }));
    }

    // Wait for all appenders
    for handle in handles {
        handle.await.unwrap();
    }

    // Wait for worker to process
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    println!("Total events: {}", bridge.total_events());
    println!("Error count: {}", bridge.error_count());

    // Query range
    match bridge.query_range(1000, 20000).await {
        Ok(snapshots) => {
            println!("\nBuckets in range [1000, 20000):");
            for (i, snapshot) in snapshots.iter().enumerate() {
                println!("  Bucket {}: {} events", i, snapshot.event_count);
            }
        }
        Err(e) => println!("Query error: {}", e),
    }

    // Flush all buckets
    bridge.flush_all().await.unwrap();
    println!("\nLast flushed bucket: {}", bridge.last_flushed());
}

/// Example 3: Analytics - Event rate calculation
#[tokio::main]
async fn example_analytics() {
    println!("\n=== Example 3: Analytics (Event Rate) ===\n");

    let bridge = Arc::new(TimelineBridge::new(
        1000,
        BucketGranularity::Minute,
        100,
    ));

    // Simulate varying event rates
    let rates = vec![10, 20, 50, 100, 200, 150, 80, 40, 20, 10]; // Events per bucket
    for (bucket_idx, &rate) in rates.iter().enumerate() {
        let base_ts = 1000 + bucket_idx as u64 * 60;
        for _ in 0..rate {
            bridge.append_event(base_ts).await.unwrap();
        }
    }

    // Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Calculate statistics
    let snapshots = bridge.query_range(1000, 1600).await.unwrap();
    let total_events: u64 = snapshots.iter().map(|s| s.event_count).sum();
    let avg_rate = total_events as f64 / snapshots.len() as f64;
    let max_rate = snapshots.iter().map(|s| s.event_count).max().unwrap();
    let min_rate = snapshots.iter().map(|s| s.event_count).min().unwrap();

    println!("Timeline Statistics:");
    println!("  Total events: {}", total_events);
    println!("  Average rate: {:.2} events/bucket", avg_rate);
    println!("  Peak rate: {} events/bucket", max_rate);
    println!("  Min rate: {} events/bucket", min_rate);

    // Detect anomalies (rate > 2× average)
    let anomaly_threshold = avg_rate * 2.0;
    println!("\nAnomalies (rate > {:.2} events/bucket):", anomaly_threshold);
    for (i, snapshot) in snapshots.iter().enumerate() {
        if snapshot.event_count as f64 > anomaly_threshold {
            println!("  Bucket {}: {} events ({}× avg)", i, snapshot.event_count,
                     snapshot.event_count as f64 / avg_rate);
        }
    }
}

/// Example 4: Hash chain validation
fn example_hash_chain() {
    println!("\n=== Example 4: Hash Chain Validation ===\n");

    let timeline = TimelineAggregationCapsule::new(
        1000,
        BucketGranularity::Minute,
        10,
    );

    // Append events to multiple buckets
    for i in 0..5 {
        let timestamp = 1000 + i * 60;
        timeline.append(timestamp).unwrap();
    }

    // Flush buckets and build hash chain
    let mut hashes = vec![];
    for i in 0..5 {
        match timeline.flush_bucket(i) {
            Ok(hash) => {
                hashes.push(hash);
                println!("Bucket {} hash: 0x{:x}", i, hash);
            }
            Err(e) => println!("Flush error: {}", e),
        }
    }

    println!("\nHash chain length: {}", hashes.len());
    println!("Hash chain provides tamper detection:");
    println!("  - Each bucket hash depends on previous bucket");
    println!("  - Modification invalidates all subsequent hashes");
    println!("  - Enables audit trail reconstruction");
}

fn main() {
    // Run examples
    example_direct_capsule();
    example_async_bridge();
    example_analytics();
    example_hash_chain();

    println!("\n=== Timeline Aggregation Demo Complete ===\n");
}
