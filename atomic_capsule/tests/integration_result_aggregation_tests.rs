//! # Integration Tests: Result Aggregation Pipeline
//!
//! **End-to-end integration tests for WorkStealingQueue + LockfreeResultAggregator + ParallelBatchProcessor**
//!
//! ## Test Coverage
//!
//! - Full pipeline: parallel processing → result aggregation → verification
//! - Cross-component integration: queue + aggregator + processor
//! - Real-world scenarios: deduplication, map-reduce, batch analytics
//! - Performance validation: throughput, latency, scalability
//! - Error handling: graceful degradation, recovery

use atomic_capsule::parallel::{
    LockfreeResultAggregator, ParallelBatchProcessor, WorkStealingQueue,
};
use std::collections::HashSet;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

// ============================================================================
// END-TO-END INTEGRATION TESTS
// ============================================================================

#[test]
fn integration_full_pipeline_simple() {
    // Simulate parallel processing with result aggregation

    // Step 1: Create processor
    let processor: ParallelBatchProcessor<u64, _, u64> =
        ParallelBatchProcessor::new(8, 32, |x: &u64| -> u64 { *x * 2 }).unwrap();

    // Step 2: Process items
    let items: Vec<u64> = (0..1000).collect();
    let results = processor.process(items.clone()).unwrap();

    // Step 3: Aggregate results
    let aggregator = LockfreeResultAggregator::new();
    for (i, result) in results.iter().enumerate() {
        aggregator.insert(items[i], *result);
    }

    // Step 4: Verify aggregation
    let aggregated = aggregator.merge();
    assert_eq!(aggregated.len(), 1000);

    for (key, values) in aggregated.iter() {
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], key * 2);
    }
}

#[test]
fn integration_parallel_deduplication_simulation() {
    // Simulate deduplication: multiple workers find duplicate candidates

    let aggregator = Arc::new(LockfreeResultAggregator::new());

    // Simulate 8 workers, each finding duplicates
    let mut handles = vec![];

    for worker_id in 0..8 {
        let agg = Arc::clone(&aggregator);
        let handle = thread::spawn(move || {
            // Each worker processes 1000 documents
            for doc_id in 0..1000 {
                // Simulate finding 1-3 duplicate candidates
                for candidate_offset in 0..=(doc_id % 3) {
                    let candidate_id = (doc_id + candidate_offset + worker_id * 1000) as u64;
                    agg.insert(doc_id as u64, candidate_id);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all results aggregated
    let results = aggregator.merge();

    // Each worker processed 1000 docs, but with overlap
    // Total unique doc IDs should be 1000 (0-999)
    assert_eq!(results.len(), 1000);

    // Each doc should have multiple candidates (from different workers)
    for (_doc_id, candidates) in results.iter() {
        assert!(candidates.len() > 0);
    }
}

#[test]
fn integration_map_reduce_pattern() {
    // Map phase: parallel processing
    let processor: ParallelBatchProcessor<u64, _, u64> =
        ParallelBatchProcessor::new(8, 32, |x: &u64| -> u64 { x % 10 }).unwrap();

    let items: Vec<u64> = (0..10_000).collect();
    let mapped = processor.process(items.clone()).unwrap();

    // Reduce phase: aggregate by key
    let aggregator = LockfreeResultAggregator::new();

    for (i, &key) in mapped.iter().enumerate() {
        aggregator.insert(key, items[i]);
    }

    let reduced = aggregator.merge();

    // Should have 10 buckets (0-9)
    assert_eq!(reduced.len(), 10);

    // Each bucket should have ~1000 items
    for (key, values) in reduced.iter() {
        assert!(
            values.len() >= 900 && values.len() <= 1100,
            "Bucket {} has {} items (expected ~1000)",
            key,
            values.len()
        );

        // All values in bucket should map to this key
        for &value in values {
            assert_eq!(value % 10, *key);
        }
    }
}

#[test]
fn integration_work_stealing_with_aggregation() {
    // Test work-stealing queue feeding into aggregator

    let queue: WorkStealingQueue<u64> = WorkStealingQueue::new(1024);
    let aggregator = Arc::new(LockfreeResultAggregator::new());

    // Push items into queue
    for i in 0..1000 {
        queue.push(i).unwrap();
    }

    let queue_arc = Arc::new(queue);

    // Spawn workers that steal from queue and aggregate results
    let mut handles = vec![];

    for worker_id in 0..8 {
        let q = Arc::clone(&queue_arc);
        let agg = Arc::clone(&aggregator);

        let handle = thread::spawn(move || {
            let mut processed = 0;

            // Steal items and process
            while let Some(item) = q.steal() {
                let result = item * 2;
                agg.insert(item, result);
                processed += 1;
            }

            processed
        });

        handles.push(handle);
    }

    // Collect processed counts
    let mut total_processed = 0;
    for handle in handles {
        total_processed += handle.join().unwrap();
    }

    // Verify all items processed
    assert_eq!(total_processed, 1000);

    // Verify aggregation
    let results = aggregator.merge();
    assert_eq!(results.len(), 1000);

    for (key, values) in results.iter() {
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], key * 2);
    }
}

#[test]
fn integration_batch_analytics_pipeline() {
    // Simulate analytics: batch processing → aggregation → summary

    // Phase 1: Batch processing (calculate metrics)
    let processor: ParallelBatchProcessor<u64, _, (u64, u64)> =
        ParallelBatchProcessor::new(8, 64, |x: &u64| -> (u64, u64) {
            // Simulate metric calculation
            let bucket = x / 100; // Group into buckets of 100
            let value = x % 100;
            (bucket, value)
        })
        .unwrap();

    let items: Vec<u64> = (0..10_000).collect();
    let metrics = processor.process(items).unwrap();

    // Phase 2: Aggregate metrics by bucket
    let aggregator = LockfreeResultAggregator::new();

    for (bucket, value) in metrics {
        aggregator.insert(bucket, value);
    }

    let aggregated = aggregator.merge();

    // Phase 3: Verify aggregation
    assert_eq!(aggregated.len(), 100); // 100 buckets

    for (bucket, values) in aggregated.iter() {
        // Each bucket should have 100 values
        assert_eq!(
            values.len(),
            100,
            "Bucket {} has {} values",
            bucket,
            values.len()
        );

        // Values should be 0-99
        let value_set: HashSet<_> = values.iter().cloned().collect();
        assert_eq!(value_set.len(), 100);
    }
}

#[test]
fn integration_error_recovery_queue_full() {
    // Test graceful handling of queue full condition

    let processor: ParallelBatchProcessor<u64, _, u64> =
        ParallelBatchProcessor::new(1, 1, |x: &u64| -> u64 { *x }).unwrap();

    // Try to process more than queue capacity
    let items: Vec<u64> = (0..5000).collect();

    let result = processor.process(items);

    // Should get error, not panic
    assert!(result.is_err());
}

#[test]
fn integration_concurrent_pipelines() {
    // Multiple independent pipelines running concurrently

    let barrier = Arc::new(Barrier::new(4));
    let mut handles = vec![];

    for pipeline_id in 0..4 {
        let b = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            b.wait();

            // Independent pipeline
            let processor: ParallelBatchProcessor<u64, _, u64> =
                ParallelBatchProcessor::new(4, 16, |x: &u64| -> u64 { x * pipeline_id }).unwrap();

            let items: Vec<u64> = (0..100).collect();
            let results = processor.process(items.clone()).unwrap();

            // Aggregate
            let aggregator = LockfreeResultAggregator::new();
            for (i, result) in results.iter().enumerate() {
                aggregator.insert(items[i], *result);
            }

            aggregator.merge().len()
        });

        handles.push(handle);
    }

    // All pipelines should complete
    for handle in handles {
        let len = handle.join().unwrap();
        assert_eq!(len, 100);
    }
}

#[test]
fn integration_performance_throughput() {
    // Measure end-to-end throughput

    let start = std::time::Instant::now();

    // Process 100K items
    let processor: ParallelBatchProcessor<u64, _, u64> =
        ParallelBatchProcessor::new(16, 128, |x: &u64| -> u64 { x.wrapping_mul(17) }).unwrap();

    let items: Vec<u64> = (0..100_000).collect();
    let results = processor.process(items.clone()).unwrap();

    // Aggregate results
    let aggregator = LockfreeResultAggregator::new();
    for (i, result) in results.iter().enumerate() {
        let bucket = items[i] / 1000;
        aggregator.insert(bucket, *result);
    }

    let _aggregated = aggregator.merge();

    let elapsed = start.elapsed();
    let items_per_sec = 100_000.0 / elapsed.as_secs_f64();

    println!(
        "End-to-end throughput: {:.2} M items/sec",
        items_per_sec / 1_000_000.0
    );

    // Should process at least 1M items/sec
    assert!(items_per_sec > 1_000_000.0);
}

#[test]
fn integration_deterministic_results() {
    // Verify deterministic results across multiple runs

    for run in 0..5 {
        let processor: ParallelBatchProcessor<u64, _, u64> =
            ParallelBatchProcessor::new(8, 32, |x: &u64| -> u64 { x * 2 }).unwrap();

        let items: Vec<u64> = (0..1000).collect();
        let results = processor.process(items.clone()).unwrap();

        // Results should be consistent
        for (i, result) in results.iter().enumerate() {
            assert_eq!(*result, items[i] * 2, "Run {} failed at index {}", run, i);
        }
    }
}

#[test]
fn integration_memory_efficiency() {
    // Test with large dataset to verify memory efficiency

    let processor: ParallelBatchProcessor<u64, _, u64> =
        ParallelBatchProcessor::new(8, 128, |x: &u64| -> u64 { *x }).unwrap();

    // Process 1M items
    let items: Vec<u64> = (0..1_000_000).collect();
    let results = processor.process(items.clone()).unwrap();

    // Aggregate into 1000 buckets
    let aggregator = LockfreeResultAggregator::new();
    for (i, result) in results.iter().enumerate() {
        let bucket = items[i] / 1000;
        aggregator.insert(bucket, *result);
    }

    let aggregated = aggregator.merge();

    // Verify all items processed
    assert_eq!(results.len(), 1_000_000);
    assert_eq!(aggregated.len(), 1000);

    // Each bucket should have 1000 items
    for values in aggregated.values() {
        assert_eq!(values.len(), 1000);
    }
}

// ============================================================================
// CROSS-COMPONENT INTEGRATION TESTS
// ============================================================================

#[test]
fn cross_component_queue_to_aggregator() {
    // Direct integration: queue → aggregator

    let queue: WorkStealingQueue<(u64, u64)> = WorkStealingQueue::new(1024);
    let aggregator = Arc::new(LockfreeResultAggregator::new());

    // Push key-value pairs into queue
    for i in 0..1000 {
        queue.push((i / 10, i)).unwrap();
    }

    let queue_arc = Arc::new(queue);

    // Single worker drains queue and aggregates
    let q = Arc::clone(&queue_arc);
    let agg = Arc::clone(&aggregator);

    let handle = thread::spawn(move || {
        while let Some((key, value)) = q.pop() {
            agg.insert(key, value);
        }
    });

    handle.join().unwrap();

    // Verify aggregation
    let results = aggregator.merge();
    assert_eq!(results.len(), 100); // 100 buckets (0-99)

    for values in results.values() {
        assert_eq!(values.len(), 10); // 10 items per bucket
    }
}

#[test]
fn cross_component_processor_to_queue() {
    // Integration: processor → queue (less common, but valid)

    let processor: ParallelBatchProcessor<u64, _, u64> =
        ParallelBatchProcessor::new(8, 32, |x: &u64| -> u64 { *x * 2 }).unwrap();

    let items: Vec<u64> = (0..100).collect();
    let results = processor.process(items).unwrap();

    // Push results into queue
    let queue: WorkStealingQueue<u64> = WorkStealingQueue::new(1024);
    for result in results {
        queue.push(result).unwrap();
    }

    // Drain queue
    let mut drained = Vec::new();
    while let Some(item) = queue.pop() {
        drained.push(item);
    }

    // Verify all items present (order may differ due to LIFO)
    drained.sort();
    let expected: Vec<u64> = (0..100).map(|x| x * 2).collect();
    assert_eq!(drained, expected);
}

// ============================================================================
// REAL-WORLD SCENARIO TESTS
// ============================================================================

#[test]
fn scenario_document_deduplication() {
    // Realistic deduplication scenario

    // Step 1: Generate documents with some duplicates
    let mut documents = Vec::new();
    for i in 0..1000 {
        documents.push(i);
    }
    // Add duplicates
    for i in 0..100 {
        documents.push(i); // Duplicate first 100
    }

    // Step 2: Parallel MinHash simulation
    let processor: ParallelBatchProcessor<u64, _, (u64, u64)> =
        ParallelBatchProcessor::new(8, 32, |doc: &u64| -> (u64, u64) {
            // Simulate MinHash signature (hash % 100)
            let signature = doc % 100;
            (*doc, signature)
        })
        .unwrap();

    let signatures = processor.process(documents.clone()).unwrap();

    // Step 3: Aggregate by signature
    let aggregator = LockfreeResultAggregator::new();
    for (doc_id, signature) in signatures {
        aggregator.insert(signature, doc_id);
    }

    let candidates = aggregator.merge();

    // Step 4: Verify duplicate detection
    // Duplicates should be in same signature bucket
    for (signature, docs) in candidates.iter() {
        // Each signature should have docs that match
        for &doc in docs {
            assert_eq!(doc % 100, *signature);
        }
    }
}

#[test]
fn scenario_log_aggregation() {
    // Simulate log aggregation pipeline

    // Step 1: Generate log entries
    let logs: Vec<u64> = (0..10_000).collect();

    // Step 2: Parallel processing (extract log level)
    let processor: ParallelBatchProcessor<u64, _, (u64, u64)> =
        ParallelBatchProcessor::new(8, 64, |log: &u64| -> (u64, u64) {
            let level = log % 5; // 5 log levels
            (*log, level)
        })
        .unwrap();

    let processed = processor.process(logs).unwrap();

    // Step 3: Aggregate by log level
    let aggregator = LockfreeResultAggregator::new();
    for (log_id, level) in processed {
        aggregator.insert(level, log_id);
    }

    let aggregated = aggregator.merge();

    // Step 4: Verify aggregation
    assert_eq!(aggregated.len(), 5); // 5 log levels

    // Each level should have ~2000 logs
    for values in aggregated.values() {
        assert!(values.len() >= 1900 && values.len() <= 2100);
    }
}

#[test]
fn scenario_batch_analytics() {
    // Simulate batch analytics (e.g., click stream)

    // Step 1: Generate events
    let events: Vec<u64> = (0..100_000).collect();

    // Step 2: Parallel event processing
    let processor: ParallelBatchProcessor<u64, _, (u64, u64)> =
        ParallelBatchProcessor::new(16, 128, |event: &u64| -> (u64, u64) {
            let user_id = event / 100; // 1000 users
            let action = event % 10; // 10 action types
            (user_id, action)
        })
        .unwrap();

    let processed = processor.process(events).unwrap();

    // Step 3: Aggregate by user
    let aggregator = LockfreeResultAggregator::new();
    for (user_id, action) in processed {
        aggregator.insert(user_id, action);
    }

    let user_actions = aggregator.merge();

    // Step 4: Verify aggregation
    assert_eq!(user_actions.len(), 1000); // 1000 users

    // Each user should have 100 actions
    for values in user_actions.values() {
        assert_eq!(values.len(), 100);
    }
}

// ============================================================================
// ERROR HANDLING AND EDGE CASES
// ============================================================================

#[test]
fn error_handling_graceful_degradation() {
    // Test graceful handling of resource limits

    let processor: ParallelBatchProcessor<u64, _, u64> =
        ParallelBatchProcessor::new(1, 1, |x: &u64| -> u64 { *x }).unwrap();

    // Try to exceed capacity
    let items: Vec<u64> = (0..5000).collect();
    let result = processor.process(items);

    // Should return error, not panic
    assert!(result.is_err());
}

#[test]
fn edge_case_empty_pipeline() {
    // Test empty input through entire pipeline

    let processor: ParallelBatchProcessor<u64, _, u64> =
        ParallelBatchProcessor::new(8, 32, |x: &u64| -> u64 { *x }).unwrap();

    let items: Vec<u64> = vec![];
    let results = processor.process(items).unwrap();

    let aggregator = LockfreeResultAggregator::new();
    for result in results {
        aggregator.insert(result, result);
    }

    let aggregated = aggregator.merge();

    // Should handle empty gracefully
    assert_eq!(aggregated.len(), 0);
}

#[test]
fn edge_case_single_item_pipeline() {
    // Test single item through entire pipeline

    let processor: ParallelBatchProcessor<u64, _, u64> =
        ParallelBatchProcessor::new(8, 32, |x: &u64| -> u64 { *x * 2 }).unwrap();

    let items = vec![42u64];
    let results = processor.process(items).unwrap();

    let aggregator = LockfreeResultAggregator::new();
    for result in results {
        aggregator.insert(result, result);
    }

    let aggregated = aggregator.merge();

    assert_eq!(aggregated.len(), 1);
    assert_eq!(aggregated[&84], vec![84]);
}
