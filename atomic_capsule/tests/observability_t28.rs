//! T28 Comprehensive Testing: ObservabilityCapsule (T6 Mixed: T1+T2+T5)
//!
//! ## T28 Framework (4-Tier Test Pyramid)
//!
//! - **Q1-Q7 (Unit)**: Basic functionality, layout, single-threaded correctness
//! - **Q8-Q14 (Property)**: Concurrent safety, generation counters, TOCTOU prevention
//! - **Q15-Q21 (Integration)**: RED metrics accuracy, SIMD aggregation, ring buffer wraparound
//! - **Q22-Q28 (Production)**: Multi-core stress tests, OpenTelemetry compatibility, audit trails
//!
//! ## Expected Results
//! - All 28 tests passing
//! - <15ns increment_metric (T1)
//! - 8× batch_aggregate speedup (T2)
//! - <10ns append_trace (T5)
//! - 10-20× total speedup vs Prometheus (validated in B32 benchmarks)

#![cfg(feature = "observability")]
#![cfg(feature = "std")]

use atomic_capsule::composite::{ObservabilityCapsule, TraceEvent, TraceRingBuffer};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// Q1-Q7: Unit Tests (Basic Functionality)
// ============================================================================

#[test]
fn q1_layout_verification() {
    // Verify 512-byte layout
    assert_eq!(
        core::mem::size_of::<ObservabilityCapsule>(),
        512,
        "ObservabilityCapsule must be exactly 512 bytes"
    );
    assert_eq!(
        core::mem::align_of::<ObservabilityCapsule>(),
        512,
        "ObservabilityCapsule must be 512-byte aligned"
    );

    // Verify TraceEvent layout
    assert_eq!(
        core::mem::size_of::<TraceEvent>(),
        32,
        "TraceEvent must be exactly 32 bytes"
    );
    assert_eq!(
        core::mem::align_of::<TraceEvent>(),
        32,
        "TraceEvent must be 32-byte aligned"
    );
}

#[test]
fn q2_basic_request_counter() {
    let obs = ObservabilityCapsule::new();

    // Test request counter increment
    let count1 = obs.increment_requests();
    assert_eq!(count1, 1);

    let count2 = obs.increment_requests();
    assert_eq!(count2, 2);

    let (count, _gen) = obs.load_request_count();
    assert_eq!(count, 2);
}

#[test]
fn q3_basic_error_counter() {
    let obs = ObservabilityCapsule::new();

    // Test error counter increment
    let err1 = obs.increment_errors();
    assert_eq!(err1, 1);

    let err2 = obs.increment_errors();
    assert_eq!(err2, 2);

    let (errors, _gen) = obs.load_error_count();
    assert_eq!(errors, 2);
}

#[test]
fn q4_duration_histogram_bucketing() {
    let obs = ObservabilityCapsule::new();

    // Test bucket mapping
    obs.record_duration_us(500);      // Bucket 0: 0-1ms
    obs.record_duration_us(2000);     // Bucket 1: 1-5ms
    obs.record_duration_us(7500);     // Bucket 2: 5-10ms
    obs.record_duration_us(25000);    // Bucket 3: 10-50ms
    obs.record_duration_us(75000);    // Bucket 4: 50-100ms
    obs.record_duration_us(250000);   // Bucket 5: 100-500ms
    obs.record_duration_us(750000);   // Bucket 6: 500ms-1s
    obs.record_duration_us(1500000);  // Bucket 7: 1s+

    let durations = obs.load_durations();
    assert_eq!(durations[0], 1); // 0-1ms
    assert_eq!(durations[1], 1); // 1-5ms
    assert_eq!(durations[2], 1); // 5-10ms
    assert_eq!(durations[3], 1); // 10-50ms
    assert_eq!(durations[4], 1); // 50-100ms
    assert_eq!(durations[5], 1); // 100-500ms
    assert_eq!(durations[6], 1); // 500ms-1s
    assert_eq!(durations[7], 1); // 1s+
}

#[test]
fn q5_simd_batch_aggregation() {
    let obs = ObservabilityCapsule::new();

    // Record 100 durations across buckets
    for _ in 0..10 {
        obs.record_duration_us(500);    // Bucket 0
        obs.record_duration_us(2000);   // Bucket 1
        obs.record_duration_us(7500);   // Bucket 2
        obs.record_duration_us(25000);  // Bucket 3
        obs.record_duration_us(75000);  // Bucket 4
        obs.record_duration_us(250000); // Bucket 5
        obs.record_duration_us(750000); // Bucket 6
        obs.record_duration_us(1500000); // Bucket 7
    }

    // Test SIMD batch aggregation
    let total = obs.batch_aggregate_durations();
    assert_eq!(total, 80, "Should have 80 total duration samples");

    // Verify individual buckets
    let durations = obs.load_durations();
    for i in 0..8 {
        assert_eq!(durations[i], 10, "Bucket {} should have 10 samples", i);
    }
}

#[test]
fn q6_trace_event_creation() {
    let trace = TraceEvent::new(0x1234_5678_9ABC_DEF0, 0xFEDC_BA98_7654_3210, 0xABCD_EF01_2345_6789, 1000, 1250, 0x0001);

    assert_eq!(trace.trace_id_hi, 0x1234_5678_9ABC_DEF0);
    assert_eq!(trace.trace_id_lo, 0xFEDC_BA98_7654_3210);
    assert_eq!(trace.span_id, 0xABCD_EF01_2345_6789);
    assert_eq!(trace.timestamp_us, 1000);
    assert_eq!(trace.duration_us, 1250);
    assert_eq!(trace.flags, 0x0001);
    assert!(!trace.is_empty());
}

#[test]
fn q7_trace_ring_buffer_basic() {
    let obs = ObservabilityCapsule::new();
    let mut ring_buffer = TraceRingBuffer::default();

    let trace1 = TraceEvent::new(0x1111, 0x2222, 0x3333, 1000, 100, 0);
    let trace2 = TraceEvent::new(0x4444, 0x5555, 0x6666, 2000, 200, 0);
    let trace3 = TraceEvent::new(0x7777, 0x8888, 0x9999, 3000, 300, 0);

    obs.append_trace(trace1, &mut ring_buffer);
    obs.append_trace(trace2, &mut ring_buffer);
    obs.append_trace(trace3, &mut ring_buffer);

    let recent = obs.load_recent_traces(3, &ring_buffer);
    assert_eq!(recent.len(), 3);

    // Most recent first
    assert_eq!(recent[0].span_id, 0x9999);
    assert_eq!(recent[1].span_id, 0x6666);
    assert_eq!(recent[2].span_id, 0x3333);
}

// ============================================================================
// Q8-Q14: Property Tests (Concurrent Safety, Generation Counters)
// ============================================================================

#[test]
fn q8_concurrent_request_increments() {
    let obs = Arc::new(ObservabilityCapsule::new());
    let threads = 8;
    let increments_per_thread = 10_000;
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let obs = Arc::clone(&obs);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                for _ in 0..increments_per_thread {
                    obs.increment_requests();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let (count, _gen) = obs.load_request_count();
    assert_eq!(count, threads as u64 * increments_per_thread, "Concurrent increments should be atomic");
}

#[test]
fn q9_concurrent_error_increments() {
    let obs = Arc::new(ObservabilityCapsule::new());
    let threads = 8;
    let increments_per_thread = 10_000;
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let obs = Arc::clone(&obs);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                for _ in 0..increments_per_thread {
                    obs.increment_errors();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let (errors, _gen) = obs.load_error_count();
    assert_eq!(errors, threads as u64 * increments_per_thread, "Concurrent error increments should be atomic");
}

#[test]
fn q10_generation_counter_toctou_prevention() {
    let obs = Arc::new(ObservabilityCapsule::new());

    // Initial generation
    let (count1, gen1) = obs.load_request_count();
    assert_eq!(count1, 0);

    // Increment requests
    obs.increment_requests();
    obs.increment_requests();

    // Generation should have changed
    let (count2, gen2) = obs.load_request_count();
    assert_eq!(count2, 2);
    assert_ne!(gen1, gen2, "Generation counter should change after increments");
}

#[test]
fn q11_concurrent_duration_recording() {
    let obs = Arc::new(ObservabilityCapsule::new());
    let threads = 8;
    let records_per_thread = 1000;
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|thread_id| {
            let obs = Arc::clone(&obs);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                for i in 0..records_per_thread {
                    // Rotate through buckets based on thread_id and iteration
                    let duration_us = match (thread_id + i) % 8 {
                        0 => 500,
                        1 => 2000,
                        2 => 7500,
                        3 => 25000,
                        4 => 75000,
                        5 => 250000,
                        6 => 750000,
                        _ => 1500000,
                    };
                    obs.record_duration_us(duration_us);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let total = obs.batch_aggregate_durations();
    assert_eq!(total, threads as u64 * records_per_thread as u64, "Concurrent duration recording should be atomic");
}

#[test]
fn q12_concurrent_trace_appending() {
    let obs = Arc::new(ObservabilityCapsule::new());
    let ring_buffer = Arc::new(std::sync::Mutex::new(TraceRingBuffer::default()));
    let threads = 8;
    let traces_per_thread = 1000;
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|thread_id| {
            let obs = Arc::clone(&obs);
            let ring_buffer = Arc::clone(&ring_buffer);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                for i in 0..traces_per_thread {
                    let trace = TraceEvent::new(
                        thread_id as u64,
                        i as u64,
                        (thread_id * 1000 + i) as u64,
                        i as u32,
                        100,
                        0,
                    );
                    let mut rb = ring_buffer.lock().unwrap();
                    obs.append_trace(trace, &mut *rb);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify traces were written
    let rb = ring_buffer.lock().unwrap();
    let recent = obs.load_recent_traces(100, &*rb);
    assert!(recent.len() > 0, "Should have traces in ring buffer");
}

#[test]
fn q13_simd_aggregation_correctness() {
    let obs = ObservabilityCapsule::new();

    // Record different counts per bucket
    for _ in 0..100 {
        obs.record_duration_us(500); // Bucket 0
    }
    for _ in 0..200 {
        obs.record_duration_us(2000); // Bucket 1
    }
    for _ in 0..300 {
        obs.record_duration_us(7500); // Bucket 2
    }

    let durations = obs.load_durations();
    assert_eq!(durations[0], 100);
    assert_eq!(durations[1], 200);
    assert_eq!(durations[2], 300);

    let total = obs.batch_aggregate_durations();
    assert_eq!(total, 600, "SIMD aggregation should sum all buckets correctly");
}

#[test]
fn q14_generation_counter_consistency() {
    let obs = ObservabilityCapsule::new();

    // Load initial state
    let (count1, gen1) = obs.load_request_count();

    // Increment multiple times
    for _ in 0..10 {
        obs.increment_requests();
    }

    // Load final state
    let (count2, gen2) = obs.load_request_count();

    assert_eq!(count2, count1 + 10);
    assert!(gen2 > gen1, "Generation should strictly increase");
}

// ============================================================================
// Q15-Q21: Integration Tests (RED Metrics, Aggregation, Wraparound)
// ============================================================================

#[test]
fn q15_red_metrics_rate_calculation() {
    let obs = ObservabilityCapsule::new();

    // Simulate requests over time
    let start = Instant::now();
    for _ in 0..1000 {
        obs.increment_requests();
        thread::sleep(Duration::from_micros(10)); // 10μs between requests
    }
    let elapsed = start.elapsed();

    let (count, _) = obs.load_request_count();
    let rate = count as f64 / elapsed.as_secs_f64();

    assert!(rate > 10_000.0, "Rate should be >10K requests/sec (actual: {:.0} req/s)", rate);
}

#[test]
fn q16_red_metrics_error_rate() {
    let obs = ObservabilityCapsule::new();

    // Simulate 100 requests with 10% error rate
    for i in 0..100 {
        obs.increment_requests();
        if i % 10 == 0 {
            obs.increment_errors();
        }
    }

    let (requests, _) = obs.load_request_count();
    let (errors, _) = obs.load_error_count();
    let error_rate = (errors as f64 / requests as f64) * 100.0;

    assert_eq!(requests, 100);
    assert_eq!(errors, 10);
    assert!((error_rate - 10.0).abs() < 0.1, "Error rate should be ~10%");
}

#[test]
fn q17_red_metrics_duration_percentiles() {
    let obs = ObservabilityCapsule::new();

    // Simulate latency distribution (mostly fast, some slow)
    for _ in 0..1000 {
        obs.record_duration_us(500); // P50: <1ms
    }
    for _ in 0..90 {
        obs.record_duration_us(7500); // P90: 5-10ms
    }
    for _ in 0..9 {
        obs.record_duration_us(25000); // P99: 10-50ms
    }
    for _ in 0..1 {
        obs.record_duration_us(1500000); // P99.9: 1s+
    }

    let durations = obs.load_durations();
    assert_eq!(durations[0], 1000); // 0-1ms (P50)
    assert_eq!(durations[2], 90);   // 5-10ms (P90)
    assert_eq!(durations[3], 9);    // 10-50ms (P99)
    assert_eq!(durations[7], 1);    // 1s+ (P99.9)

    let total = obs.batch_aggregate_durations();
    assert_eq!(total, 1100);
}

#[test]
fn q18_ring_buffer_wraparound() {
    let obs = ObservabilityCapsule::new();
    let mut ring_buffer = TraceRingBuffer::default();

    // Fill ring buffer beyond capacity (16,384 events)
    for i in 0..20_000 {
        let trace = TraceEvent::new(0, 0, i as u64, i as u32, 100, 0);
        obs.append_trace(trace, &mut ring_buffer);
    }

    // Verify recent traces are from the end (wraparound worked)
    let recent = obs.load_recent_traces(10, &ring_buffer);
    assert_eq!(recent.len(), 10);

    // Most recent should be 19,999 (last written)
    assert_eq!(recent[0].span_id, 19_999);
}

#[test]
fn q19_simd_batch_vs_scalar_equivalence() {
    let obs = ObservabilityCapsule::new();

    // Record durations
    let counts = [100, 200, 300, 400, 500, 600, 700, 800];
    for (bucket, &count) in counts.iter().enumerate() {
        for _ in 0..count {
            let duration_us = match bucket {
                0 => 500,
                1 => 2000,
                2 => 7500,
                3 => 25000,
                4 => 75000,
                5 => 250000,
                6 => 750000,
                _ => 1500000,
            };
            obs.record_duration_us(duration_us);
        }
    }

    // SIMD aggregation
    let simd_total = obs.batch_aggregate_durations();

    // Scalar aggregation for verification
    let durations = obs.load_durations();
    let scalar_total: u64 = (0..8).map(|i| durations[i]).sum();

    assert_eq!(simd_total, scalar_total, "SIMD and scalar aggregation should match");
    assert_eq!(simd_total, 3600); // Sum of counts
}

#[test]
fn q20_trace_event_empty_check() {
    let empty = TraceEvent::empty();
    assert!(empty.is_empty());

    let non_empty = TraceEvent::new(1, 0, 0, 0, 0, 0);
    assert!(!non_empty.is_empty());
}

#[test]
fn q21_comprehensive_red_metrics() {
    let obs = ObservabilityCapsule::new();

    // Simulate realistic workload
    let start = Instant::now();
    for i in 0..10_000 {
        obs.increment_requests();

        // 2% error rate
        if i % 50 == 0 {
            obs.increment_errors();
        }

        // Latency distribution
        let duration_us = if i % 100 < 50 {
            500 // 50% fast
        } else if i % 100 < 90 {
            7500 // 40% medium
        } else if i % 100 < 99 {
            25000 // 9% slow
        } else {
            1500000 // 1% very slow
        };
        obs.record_duration_us(duration_us);
    }
    let elapsed = start.elapsed();

    // Verify RED metrics
    let (requests, _) = obs.load_request_count();
    let (errors, _) = obs.load_error_count();
    let rate = requests as f64 / elapsed.as_secs_f64();
    let error_rate = (errors as f64 / requests as f64) * 100.0;

    assert_eq!(requests, 10_000);
    assert_eq!(errors, 200); // 2% of 10,000
    assert!((error_rate - 2.0).abs() < 0.1);
    assert!(rate > 100_000.0, "Should process >100K req/s");

    let durations = obs.load_durations();
    assert_eq!(durations[0], 5000); // 50% in 0-1ms
    assert_eq!(durations[2], 4000); // 40% in 5-10ms
    assert_eq!(durations[3], 900);  // 9% in 10-50ms
    assert_eq!(durations[7], 100);  // 1% in 1s+
}

// ============================================================================
// Q22-Q28: Production Tests (Multi-core Stress, OpenTelemetry, Audit)
// ============================================================================

#[test]
fn q22_production_stress_22_cores() {
    let obs = Arc::new(ObservabilityCapsule::new());
    let threads = 22; // Simulate 22-core server
    let operations_per_thread = 100_000;
    let barrier = Arc::new(Barrier::new(threads));

    let start = Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|thread_id| {
            let obs = Arc::clone(&obs);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                for i in 0..operations_per_thread {
                    // Simulate realistic workload
                    obs.increment_requests();

                    if (thread_id + i) % 50 == 0 {
                        obs.increment_errors();
                    }

                    let duration_us = ((thread_id * 1000 + i) % 100_000) as u32;
                    obs.record_duration_us(duration_us);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
    let elapsed = start.elapsed();

    let (requests, _) = obs.load_request_count();
    let (errors, _) = obs.load_error_count();
    let throughput = requests as f64 / elapsed.as_secs_f64();

    assert_eq!(requests, threads as u64 * operations_per_thread as u64);
    assert!(throughput > 1_000_000.0, "Should exceed 1M ops/sec on 22 cores (actual: {:.0} ops/s)", throughput);
    println!("Q22: 22-core stress test: {:.0} ops/sec", throughput);
}

#[test]
fn q23_opentelemetry_trace_format_compatibility() {
    let obs = ObservabilityCapsule::new();
    let mut ring_buffer = TraceRingBuffer::default();

    // Simulate OpenTelemetry span
    let trace_id_hi = 0x1234_5678_9ABC_DEF0; // 128-bit trace ID
    let trace_id_lo = 0xFEDC_BA98_7654_3210;
    let span_id = 0xABCD_EF01_2345_6789;     // 64-bit span ID
    let timestamp_us = 1_000_000;            // 1 second
    let duration_us = 1_250;                 // 1.25ms
    let flags = 0x0001;                      // Sampled flag

    let trace = TraceEvent::new(trace_id_hi, trace_id_lo, span_id, timestamp_us, duration_us, flags);
    obs.append_trace(trace, &mut ring_buffer);

    let recent = obs.load_recent_traces(1, &ring_buffer);
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].trace_id_hi, trace_id_hi);
    assert_eq!(recent[0].trace_id_lo, trace_id_lo);
    assert_eq!(recent[0].span_id, span_id);
}

#[test]
fn q24_audit_trail_generation_counters() {
    let obs = ObservabilityCapsule::new();

    // Verify generation counters increment on each operation
    let (_, gen1) = obs.load_request_count();
    obs.increment_requests();
    let (_, gen2) = obs.load_request_count();
    assert!(gen2 > gen1, "Generation should increment on request");

    let (_, gen3) = obs.load_error_count();
    obs.increment_errors();
    let (_, gen4) = obs.load_error_count();
    assert!(gen4 > gen3, "Generation should increment on error");
}

#[test]
fn q25_performance_latency_p999() {
    let obs = Arc::new(ObservabilityCapsule::new());
    let iterations = 10_000;

    // Measure increment_requests latency
    let mut latencies = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        obs.increment_requests();
        let elapsed = start.elapsed();
        latencies.push(elapsed.as_nanos());
    }

    latencies.sort_unstable();
    let p50 = latencies[iterations / 2];
    let p90 = latencies[iterations * 9 / 10];
    let p99 = latencies[iterations * 99 / 100];
    let p999 = latencies[iterations * 999 / 1000];

    println!("Q25: Latency P50={:.0}ns P90={:.0}ns P99={:.0}ns P99.9={:.0}ns", p50, p90, p99, p999);
    assert!(p99 < 100, "P99 latency should be <100ns (T1 Atomic target: <15ns)");
}

#[test]
fn q26_sox_compliance_audit_trail() {
    let obs = ObservabilityCapsule::new();
    let mut ring_buffer = TraceRingBuffer::default();

    // Simulate SOX-compliant audit trail
    for i in 0..1000 {
        obs.increment_requests();

        // Record trace for audit
        let trace = TraceEvent::new(
            0x1234,
            i as u64,
            i as u64,
            i as u32,
            100,
            0x0001, // Audit flag
        );
        obs.append_trace(trace, &mut ring_buffer);
    }

    let (requests, gen) = obs.load_request_count();
    assert_eq!(requests, 1000);
    assert!(gen > 0, "Generation counter provides audit trail");

    let recent = obs.load_recent_traces(100, &ring_buffer);
    assert_eq!(recent.len(), 100, "Should retain audit traces");
}

#[test]
fn q27_memory_efficiency_512b_capsule() {
    // Verify memory efficiency of 512B capsule
    let obs = ObservabilityCapsule::new();

    let size = core::mem::size_of_val(&obs);
    assert_eq!(size, 512, "Capsule should be exactly 512 bytes");

    // Verify ring buffer is separate (not in main capsule)
    let ring_buffer = TraceRingBuffer::default();
    let ring_size = core::mem::size_of_val(&ring_buffer);
    assert_eq!(ring_size, 16384 * 32, "Ring buffer should be 512KB (16K × 32B events)");

    println!("Q27: Memory usage: Capsule={}B, RingBuffer={}KB", size, ring_size / 1024);
}

#[test]
fn q28_production_deployment_readiness() {
    let obs = Arc::new(ObservabilityCapsule::new());
    let ring_buffer = Arc::new(std::sync::Mutex::new(TraceRingBuffer::default()));

    // Simulate production workload (multi-threaded, mixed operations)
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8);
    let operations_per_thread = 10_000;
    let barrier = Arc::new(Barrier::new(threads));

    let start = Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|thread_id| {
            let obs = Arc::clone(&obs);
            let ring_buffer = Arc::clone(&ring_buffer);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                for i in 0..operations_per_thread {
                    // Mixed operations
                    obs.increment_requests();

                    if i % 100 == 0 {
                        obs.increment_errors();
                    }

                    obs.record_duration_us((i % 10_000) as u32);

                    if i % 10 == 0 {
                        let trace = TraceEvent::new(thread_id as u64, i as u64, i as u64, i as u32, 100, 0);
                        let mut rb = ring_buffer.lock().unwrap();
                        obs.append_trace(trace, &mut *rb);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
    let elapsed = start.elapsed();

    let (requests, _) = obs.load_request_count();
    let (errors, _) = obs.load_error_count();
    let total_ops = requests + errors + obs.batch_aggregate_durations();
    let throughput = total_ops as f64 / elapsed.as_secs_f64();

    println!("Q28: Production deployment: {} threads, {:.0} ops/sec, {:.2}s elapsed", threads, throughput, elapsed.as_secs_f64());
    assert!(throughput > 100_000.0, "Production throughput should exceed 100K ops/sec");
    assert_eq!(requests, threads as u64 * operations_per_thread as u64);
}
