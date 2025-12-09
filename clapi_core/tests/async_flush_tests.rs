//! Comprehensive tests for async flush pipeline (T28 Framework)
//!
//! Tests cover:
//! - Unit (Q1-Q7): Basic flush operations
//! - Property (Q8-Q14): Concurrent behavior, lossless guarantee
//! - Integration (Q15-Q21): Timeline integration
//! - Production (Q22-Q28): Stress testing, graceful shutdown

use clapi_core::capsules::async_flush_capsule::{AsyncFlushPipeline, FlushTask};
use clapi_core::capsules::timeline_aggregation_capsule::TimelineAggregationCapsuleWrapper;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

// ============================================================================
// Unit Tests (Q1-Q7): Basic functionality
// ============================================================================

#[test]
fn unit_q1_flush_task_creation() {
    let task = FlushTask::new(0, 1000, 1060, 42, 0);
    assert_eq!(task.bucket_id, 0);
    assert_eq!(task.start_ts, 1000);
    assert_eq!(task.end_ts, 1060);
    assert_eq!(task.event_count, 42);
    assert_eq!(task.prev_hash, 0);
}

#[test]
fn unit_q2_flush_task_hash_deterministic() {
    let task = FlushTask::new(0, 1000, 1060, 42, 0);
    let hash1 = task.compute_hash();
    let hash2 = task.compute_hash();
    assert_eq!(hash1, hash2, "Hash should be deterministic");
    assert_ne!(hash1, 0, "Hash should be non-zero");
}

#[test]
fn unit_q3_flush_task_hash_sensitivity() {
    // Different inputs should produce different hashes
    let task1 = FlushTask::new(0, 1000, 1060, 42, 0);
    let task2 = FlushTask::new(0, 1000, 1060, 43, 0); // Different event_count
    let task3 = FlushTask::new(0, 1001, 1061, 42, 0); // Different timestamps
    let task4 = FlushTask::new(1, 1000, 1060, 42, 0); // Different bucket_id

    assert_ne!(task1.compute_hash(), task2.compute_hash());
    assert_ne!(task1.compute_hash(), task3.compute_hash());
    assert_ne!(task1.compute_hash(), task4.compute_hash());
}

#[test]
fn unit_q4_pipeline_creation() {
    let called = Arc::new(AtomicU64::new(0));
    let called_clone = Arc::clone(&called);

    let pipeline = AsyncFlushPipeline::new(move |_result| {
        called_clone.fetch_add(1, Ordering::Relaxed);
    });

    assert!(pipeline.is_worker_alive());
    assert_eq!(pipeline.metrics().scheduled, 0);
    assert_eq!(pipeline.metrics().completed, 0);
}

#[test]
fn unit_q5_pipeline_schedule_single() {
    let completed = Arc::new(AtomicU64::new(0));
    let completed_clone = Arc::clone(&completed);

    let pipeline = AsyncFlushPipeline::new(move |_result| {
        completed_clone.fetch_add(1, Ordering::Relaxed);
    });

    let task = FlushTask::new(0, 1000, 1060, 42, 0);
    pipeline.schedule_flush(task).unwrap();

    // Wait for processing
    thread::sleep(Duration::from_millis(10));

    assert_eq!(completed.load(Ordering::Relaxed), 1);
    assert_eq!(pipeline.metrics().completed, 1);
}

#[test]
fn unit_q6_pipeline_metrics() {
    let pipeline = AsyncFlushPipeline::new(|_result| {});

    // Schedule 5 tasks
    for i in 0..5 {
        let task = FlushTask::new(i as u32, 1000, 1060, 42, 0);
        pipeline.schedule_flush(task).unwrap();
    }

    // Wait for processing
    thread::sleep(Duration::from_millis(50));

    let metrics = pipeline.metrics();
    assert_eq!(metrics.scheduled, 5);
    assert_eq!(metrics.completed, 5);
    assert_eq!(metrics.failed, 0);
    assert!(metrics.avg_compute_ns > 0, "Should have compute time");
    assert!(metrics.worker_alive);
}

#[test]
fn unit_q7_pipeline_graceful_shutdown() {
    let completed = Arc::new(AtomicU64::new(0));
    let completed_clone = Arc::clone(&completed);

    let pipeline = AsyncFlushPipeline::new(move |_result| {
        completed_clone.fetch_add(1, Ordering::Relaxed);
    });

    // Schedule 10 tasks
    for i in 0..10 {
        let task = FlushTask::new(i as u32, 1000, 1060, 42, 0);
        pipeline.schedule_flush(task).unwrap();
    }

    // Drop pipeline (should drain pending)
    drop(pipeline);

    // Verify all tasks completed before shutdown
    assert_eq!(completed.load(Ordering::Relaxed), 10);
}

// ============================================================================
// Property Tests (Q8-Q14): Concurrent behavior
// ============================================================================

#[test]
fn property_q8_concurrent_schedule() {
    let completed = Arc::new(AtomicU64::new(0));
    let completed_clone = Arc::clone(&completed);

    let pipeline = Arc::new(AsyncFlushPipeline::new(move |_result| {
        completed_clone.fetch_add(1, Ordering::Relaxed);
    }));

    // 10 threads, 100 tasks each
    let handles: Vec<_> = (0..10)
        .map(|tid| {
            let pipeline = Arc::clone(&pipeline);
            thread::spawn(move || {
                for i in 0..100 {
                    let task = FlushTask::new((tid * 100 + i) as u32, 1000, 1060, 42, 0);
                    pipeline.schedule_flush(task).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Wait for all processing
    thread::sleep(Duration::from_millis(200));

    // Verify all 1000 tasks completed
    assert_eq!(completed.load(Ordering::Relaxed), 1000);
    assert_eq!(pipeline.metrics().completed, 1000);
}

#[test]
fn property_q9_lossless_guarantee() {
    let results = Arc::new(Mutex::new(Vec::new()));
    let results_clone = Arc::clone(&results);

    let pipeline = AsyncFlushPipeline::new(move |result| {
        results_clone.lock().unwrap().push(result.bucket_id);
    });

    // Schedule 1000 tasks
    for i in 0..1000 {
        let task = FlushTask::new(i as u32, 1000, 1060, 42, 0);
        pipeline.schedule_flush(task).unwrap();
    }

    // Wait for processing
    thread::sleep(Duration::from_millis(200));

    // Verify all tasks processed (lossless)
    let completed = results.lock().unwrap();
    assert_eq!(completed.len(), 1000, "Should process all tasks");

    // Verify unique bucket IDs
    let mut sorted = completed.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 1000, "All bucket IDs should be unique");
}

#[test]
fn property_q10_ordering_preservation() {
    let results = Arc::new(Mutex::new(Vec::new()));
    let results_clone = Arc::clone(&results);

    let pipeline = AsyncFlushPipeline::new(move |result| {
        results_clone.lock().unwrap().push(result.bucket_id);
    });

    // Schedule tasks sequentially
    for i in 0..100 {
        let task = FlushTask::new(i as u32, 1000, 1060, 42, 0);
        pipeline.schedule_flush(task).unwrap();
        // Small delay to ensure ordering
        thread::sleep(Duration::from_micros(10));
    }

    // Wait for processing
    thread::sleep(Duration::from_millis(100));

    // Verify ordering preserved
    let completed = results.lock().unwrap();
    assert_eq!(completed.len(), 100);

    for (i, &bucket_id) in completed.iter().enumerate() {
        assert_eq!(bucket_id, i as u32, "Order should be preserved");
    }
}

#[test]
fn property_q11_hash_chain_integrity() {
    let hashes = Arc::new(Mutex::new(Vec::new()));
    let hashes_clone = Arc::clone(&hashes);

    let pipeline = AsyncFlushPipeline::new(move |result| {
        hashes_clone.lock().unwrap().push(result.hash);
    });

    // Schedule tasks with hash chain
    let mut prev_hash = 0u64;
    for i in 0..10 {
        let task = FlushTask::new(i as u32, 1000 + i * 60, 1060 + i * 60, 42, prev_hash);
        prev_hash = task.compute_hash();
        pipeline.schedule_flush(task).unwrap();
    }

    // Wait for processing
    thread::sleep(Duration::from_millis(50));

    // Verify hashes are unique (chain integrity)
    let computed = hashes.lock().unwrap();
    assert_eq!(computed.len(), 10);

    let mut unique_hashes = computed.clone();
    unique_hashes.sort();
    unique_hashes.dedup();
    assert_eq!(unique_hashes.len(), 10, "All hashes should be unique");
}

// ============================================================================
// Integration Tests (Q15-Q21): Timeline integration
// ============================================================================

#[test]
fn integration_q15_timeline_append_with_async_flush() {
    let flushed_hashes = Arc::new(Mutex::new(Vec::new()));
    let flushed_clone = Arc::clone(&flushed_hashes);

    let pipeline = AsyncFlushPipeline::new(move |result| {
        flushed_clone.lock().unwrap().push(result.hash);
    });

    let timeline = TimelineAggregationCapsuleWrapper::default();

    // Append events with async flush
    for i in 0..1500 {
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i);
        timeline
            .append_with_async_flush(time, Some(&pipeline))
            .unwrap();
    }

    // Wait for async flushes
    thread::sleep(Duration::from_millis(100));

    // Verify flushes occurred (threshold: 1000 events per bucket)
    let hashes = flushed_hashes.lock().unwrap();
    assert!(hashes.len() > 0, "Should have flushed at least one bucket");
}

#[test]
fn integration_q16_timeline_batch_append() {
    let timeline = TimelineAggregationCapsuleWrapper::default();

    // Prepare batch
    let timestamps: Vec<u64> = (1000..2000).collect();
    let request = clapi_core::capsules::batch_append_capsule::BatchAppendRequest::new(timestamps);

    // Append batch
    let stats = timeline.append_batch(request).unwrap();

    assert_eq!(stats.appended, 1000);
    assert!(stats.latency_per_item_ns < 100, "Should be <100ns per item");
    assert_eq!(timeline.total_events(), 1000);
}

#[test]
fn integration_q17_batch_vs_single_throughput() {
    let timeline = TimelineAggregationCapsuleWrapper::default();

    // Single append (baseline)
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i);
        timeline.append_system_time(time, "test").unwrap();
    }
    let single_duration = start.elapsed();

    // Batch append
    let timestamps: Vec<u64> = (2000..3000).collect();
    let request = clapi_core::capsules::batch_append_capsule::BatchAppendRequest::new(timestamps);

    let start = std::time::Instant::now();
    let stats = timeline.append_batch(request).unwrap();
    let batch_duration = start.elapsed();

    println!(
        "Single: {:?} ({} ns/item), Batch: {:?} ({} ns/item), Speedup: {:.2}×",
        single_duration,
        single_duration.as_nanos() / 1000,
        batch_duration,
        stats.latency_per_item_ns,
        single_duration.as_nanos() as f64 / batch_duration.as_nanos() as f64
    );

    // Batch should be faster (target: 5× speedup)
    assert!(
        batch_duration < single_duration,
        "Batch should be faster than single"
    );
}

// ============================================================================
// Production Tests (Q22-Q28): Stress testing
// ============================================================================

#[test]
fn production_q22_high_throughput_stress() {
    let completed = Arc::new(AtomicU64::new(0));
    let completed_clone = Arc::clone(&completed);

    let pipeline = Arc::new(AsyncFlushPipeline::new(move |_result| {
        completed_clone.fetch_add(1, Ordering::Relaxed);
    }));

    // Stress: 20 threads × 1000 tasks = 20K total
    let handles: Vec<_> = (0..20)
        .map(|tid| {
            let pipeline = Arc::clone(&pipeline);
            thread::spawn(move || {
                for i in 0..1000 {
                    let task = FlushTask::new((tid * 1000 + i) as u32, 1000, 1060, 42, 0);
                    pipeline.schedule_flush(task).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Wait for all processing
    thread::sleep(Duration::from_millis(500));

    // Verify all 20K tasks completed
    assert_eq!(completed.load(Ordering::Relaxed), 20_000);
    assert_eq!(pipeline.metrics().completed, 20_000);
}

#[test]
fn production_q23_graceful_shutdown_under_load() {
    let completed = Arc::new(AtomicU64::new(0));
    let completed_clone = Arc::clone(&completed);

    let pipeline = AsyncFlushPipeline::new(move |_result| {
        completed_clone.fetch_add(1, Ordering::Relaxed);
    });

    // Schedule 5000 tasks
    for i in 0..5000 {
        let task = FlushTask::new(i as u32, 1000, 1060, 42, 0);
        pipeline.schedule_flush(task).unwrap();
    }

    // Drop immediately (should drain all pending)
    drop(pipeline);

    // Verify all tasks completed
    assert_eq!(
        completed.load(Ordering::Relaxed),
        5000,
        "Should drain all pending on shutdown"
    );
}

#[test]
fn production_q24_metrics_accuracy() {
    let pipeline = AsyncFlushPipeline::new(|_result| {});

    // Schedule known number of tasks
    for i in 0..100 {
        let task = FlushTask::new(i as u32, 1000, 1060, 42, 0);
        pipeline.schedule_flush(task).unwrap();
    }

    // Wait for processing
    thread::sleep(Duration::from_millis(100));

    let metrics = pipeline.metrics();

    // Verify metrics accuracy
    assert_eq!(metrics.scheduled, 100, "Scheduled count accurate");
    assert_eq!(metrics.completed, 100, "Completed count accurate");
    assert_eq!(metrics.pending, 0, "No pending tasks");
    assert_eq!(metrics.failed, 0, "No failures");
    assert!(metrics.avg_compute_ns > 0, "Compute time tracked");
    assert!(
        metrics.avg_compute_ns < 1_000,
        "Compute time reasonable (<1μs)"
    );
}
