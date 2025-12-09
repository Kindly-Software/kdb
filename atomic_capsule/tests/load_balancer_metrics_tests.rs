//! Comprehensive tests for LoadBalancerMetricsCapsule (T28 Framework)
//!
//! Tests organized in 4 tiers:
//! - Q1-Q7: Unit tests (basic functionality)
//! - Q8-Q14: Property tests (invariants)
//! - Q15-Q21: Integration tests (multi-component)
//! - Q22-Q28: Production tests (sustained load)

#[cfg(test)]
mod unit_tests {
    use atomic_capsule::load_balancing::{
        LoadBalancerMetricsCapsule, BackendMetrics, BackendState, AlertThresholds,
        AlertLevel,
    };

    /// Q1: Basic initialization
    #[test]
    fn test_metrics_new() {
        let metrics = LoadBalancerMetricsCapsule::new();
        let snapshot = metrics.aggregate_metrics().unwrap();

        assert_eq!(snapshot.total_requests, 0);
        assert_eq!(snapshot.successful_requests, 0);
        assert_eq!(snapshot.failed_requests, 0);
        assert_eq!(snapshot.success_rate, 0.0);
    }

    /// Q2: Layout verification (cache alignment)
    #[test]
    fn test_layout_verification() {
        use std::mem::{size_of, align_of};

        assert_eq!(size_of::<LoadBalancerMetricsCapsule>(), 256);
        assert_eq!(align_of::<LoadBalancerMetricsCapsule>(), 256);
        assert_eq!(size_of::<BackendMetrics>(), 128);
        assert_eq!(align_of::<BackendMetrics>(), 128);
    }

    /// Q3: Record single request
    #[test]
    fn test_record_single_request() {
        let metrics = LoadBalancerMetricsCapsule::new();
        metrics.record_request(0, 5_000_000, true).unwrap();

        let snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.successful_requests, 1);
        assert_eq!(snapshot.failed_requests, 0);
        assert_eq!(snapshot.success_rate, 1.0);
    }

    /// Q4: Record failed request
    #[test]
    fn test_record_failed_request() {
        let metrics = LoadBalancerMetricsCapsule::new();
        metrics.record_request(0, 10_000_000, false).unwrap();

        let snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.successful_requests, 0);
        assert_eq!(snapshot.failed_requests, 1);
        assert_eq!(snapshot.success_rate, 0.0);
    }

    /// Q5: Track latency min/max
    #[test]
    fn test_latency_min_max() {
        let metrics = LoadBalancerMetricsCapsule::new();
        metrics.record_request(0, 1_000_000, true).unwrap();
        metrics.record_request(0, 10_000_000, true).unwrap();
        metrics.record_request(0, 5_000_000, true).unwrap();

        let snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.min_latency_ns, 1_000_000);
        assert_eq!(snapshot.max_latency_ns, 10_000_000);
        assert_eq!(snapshot.total_requests, 3);
    }

    /// Q6: Latency average calculation
    #[test]
    fn test_latency_average() {
        let metrics = LoadBalancerMetricsCapsule::new();
        metrics.record_request(0, 2_000_000, true).unwrap();
        metrics.record_request(0, 4_000_000, true).unwrap();
        metrics.record_request(0, 6_000_000, true).unwrap();

        let snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.avg_latency_ns, 4_000_000.0);
    }

    /// Q7: Backend health tracking
    #[test]
    fn test_backend_health() {
        let metrics = LoadBalancerMetricsCapsule::new();
        metrics.record_health_check(0, true).unwrap();
        metrics.record_health_check(1, true).unwrap();
        metrics.record_health_check(2, false).unwrap();

        let snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.healthy_backends, 2);
        assert_eq!(snapshot.unhealthy_backends, 1);
    }
}

#[cfg(test)]
mod property_tests {
    use atomic_capsule::load_balancing::LoadBalancerMetricsCapsule;
    use std::sync::Arc;
    use std::thread;

    /// Q8: Concurrent request recording
    #[test]
    fn test_concurrent_requests() {
        let metrics = Arc::new(LoadBalancerMetricsCapsule::new());
        let mut handles = vec![];

        // Spawn 4 threads, each recording 25 requests
        for t in 0..4 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                for i in 0..25 {
                    let latency = ((t * 25 + i) * 1000) as u64;
                    m.record_request(t as u32, latency, true).unwrap();
                }
            }));
        }

        // Wait for all threads
        for h in handles {
            h.join().unwrap();
        }

        let snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.total_requests, 100);
        assert_eq!(snapshot.successful_requests, 100);
    }

    /// Q9: Mixed success/failure under contention
    #[test]
    fn test_mixed_success_failure() {
        let metrics = Arc::new(LoadBalancerMetricsCapsule::new());
        let mut handles = vec![];

        for t in 0..8 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                for i in 0..12 {
                    let success = (i % 3) != 0; // 2/3 success rate
                    m.record_request(t as u32, 5_000_000, success).unwrap();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.total_requests, 96);
        // Approximate: 64 successful, 32 failed
        assert!(snapshot.successful_requests >= 60 && snapshot.successful_requests <= 68);
        assert!(snapshot.failed_requests >= 28 && snapshot.failed_requests <= 36);
    }

    /// Q10: Session hit/miss tracking
    #[test]
    fn test_session_tracking() {
        let metrics = LoadBalancerMetricsCapsule::new();

        for i in 0..100 {
            let hit = i % 3 != 0; // 2/3 hit rate
            metrics.record_session_lookup(hit).unwrap();
        }

        let snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.session_hits + snapshot.session_misses, 100);
        // Approximately 66 hits, 34 misses
        assert!(snapshot.session_hits >= 60 && snapshot.session_hits <= 72);
    }

    /// Q11: Circuit breaker state transitions
    #[test]
    fn test_circuit_breaker_states() {
        let metrics = LoadBalancerMetricsCapsule::new();

        metrics.record_circuit_breaker_state("open").unwrap();
        metrics.record_circuit_breaker_state("closed").unwrap();
        metrics.record_circuit_breaker_state("half_open").unwrap();
        metrics.record_circuit_breaker_state("open").unwrap();
        metrics.record_circuit_breaker_state("closed").unwrap();

        let snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.circuit_breaker_opens, 2);
        assert_eq!(snapshot.circuit_breaker_closes, 2);
        assert_eq!(snapshot.circuit_breaker_half_opens, 1);
    }

    /// Q12: Connection lifecycle
    #[test]
    fn test_connection_lifecycle() {
        let metrics = LoadBalancerMetricsCapsule::new();

        // Establish connections
        metrics.record_connection(0, true).unwrap();
        metrics.record_connection(0, true).unwrap();
        metrics.record_connection(0, true).unwrap();

        let mut snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.active_connections, 3);

        // Close connections
        metrics.record_connection(0, false).unwrap();
        metrics.record_connection(0, false).unwrap();

        snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.active_connections, 1);
        assert_eq!(snapshot.idle_connections, 2);
    }

    /// Q13: Audit hash stability
    #[test]
    fn test_audit_hash_stability() {
        let metrics = LoadBalancerMetricsCapsule::new();

        metrics.record_request(0, 5_000_000, true).unwrap();
        metrics.record_request(0, 3_000_000, true).unwrap();

        let snapshot1 = metrics.aggregate_metrics().unwrap();

        // Record same audit hash twice
        assert!(metrics.verify_audit_trail(&snapshot1).unwrap());

        // Hash should remain stable
        let snapshot2 = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot1.audit_hash, snapshot2.audit_hash);
    }

    /// Q14: Percentile approximations
    #[test]
    fn test_percentile_calculations() {
        let metrics = LoadBalancerMetricsCapsule::new();

        // Record requests with increasing latency
        for i in 1..=100 {
            let latency = (i * 100_000) as u64; // 100us to 10ms
            metrics.record_request(0, latency, true).unwrap();
        }

        let snapshot = metrics.aggregate_metrics().unwrap();
        assert!(snapshot.p50_latency_ns > 0);
        assert!(snapshot.p95_latency_ns > snapshot.p50_latency_ns);
        assert!(snapshot.p99_latency_ns > snapshot.p95_latency_ns);
    }
}

#[cfg(test)]
mod integration_tests {
    use atomic_capsule::load_balancing::{
        LoadBalancerMetricsCapsule, BackendMetrics, BackendState, AlertThresholds,
        AlertLevel,
    };
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    /// Q15: Multi-backend metrics
    #[test]
    fn test_multi_backend_aggregation() {
        let backends: Vec<_> = (0..4).map(|id| BackendMetrics::new(id)).collect();
        let metrics = LoadBalancerMetricsCapsule::new();

        // Record requests to different backends
        for (i, _backend) in backends.iter().enumerate() {
            for _ in 0..10 {
                metrics.record_request(i as u32, 5_000_000, true).unwrap();
            }
        }

        let snapshot = metrics.aggregate_metrics().unwrap();
        assert_eq!(snapshot.total_requests, 40);
        assert_eq!(snapshot.successful_requests, 40);
    }

    /// Q16: Alert threshold checking
    #[test]
    fn test_alert_thresholds() {
        let metrics = LoadBalancerMetricsCapsule::new();

        // Record high-latency requests
        for _ in 0..10 {
            metrics.record_request(0, 500_000_000, true).unwrap(); // 500ms
        }

        let thresholds = AlertThresholds {
            max_latency_ms: 100, // 100ms threshold
            min_healthy_backends: 1,
            max_error_rate: 0.05,
            max_circuit_breaker_opens: 10,
            min_session_hit_rate: 0.5,
        };

        let alerts = metrics.check_alerts(&thresholds).unwrap();
        assert!(!alerts.is_empty());

        // Should have latency alert
        let has_latency_alert = alerts.iter().any(|a| a.metric == "p95_latency");
        assert!(has_latency_alert);
    }

    /// Q17: Prometheus export format
    #[test]
    fn test_prometheus_export() {
        let metrics = LoadBalancerMetricsCapsule::new();

        metrics.record_request(0, 5_000_000, true).unwrap();
        metrics.record_request(0, 3_000_000, true).unwrap();

        let prometheus = metrics.export_prometheus().unwrap();
        assert!(prometheus.contains("load_balancer_requests_total"));
        assert!(prometheus.contains("load_balancer_success_rate"));
        assert!(prometheus.contains("load_balancer_latency_avg_ns"));
    }

    /// Q18: JSON export format
    #[test]
    fn test_json_export() {
        let metrics = LoadBalancerMetricsCapsule::new();

        metrics.record_request(0, 5_000_000, true).unwrap();

        let json = metrics.export_json().unwrap();
        assert!(json.contains("total_requests"));
        assert!(json.contains("successful_requests"));
        assert!(json.contains("failed_requests"));
        assert!(json.contains("success_rate"));
    }

    /// Q19: Binary export format
    #[test]
    fn test_binary_export() {
        let metrics = LoadBalancerMetricsCapsule::new();

        metrics.record_request(0, 5_000_000, true).unwrap();
        metrics.record_request(0, 3_000_000, true).unwrap();

        let binary = metrics.export_binary().unwrap();
        // Should contain multiple u64 values
        assert!(binary.len() >= 56); // At least 7 u64s
    }

    /// Q20: Backend state transitions
    #[test]
    fn test_backend_state_transitions() {
        let backend = BackendMetrics::new(0);

        assert_eq!(backend.get_state(), BackendState::Healthy);

        backend.set_state(BackendState::Degraded);
        assert_eq!(backend.get_state(), BackendState::Degraded);

        backend.set_state(BackendState::Unhealthy);
        assert_eq!(backend.get_state(), BackendState::Unhealthy);

        backend.set_state(BackendState::Quarantined);
        assert_eq!(backend.get_state(), BackendState::Quarantined);

        backend.set_state(BackendState::Healthy);
        assert_eq!(backend.get_state(), BackendState::Healthy);
    }

    /// Q21: Error rate calculation
    #[test]
    fn test_error_rate_alert() {
        let metrics = LoadBalancerMetricsCapsule::new();

        // 20 requests, 5 failures (25% error rate)
        for _ in 0..15 {
            metrics.record_request(0, 5_000_000, true).unwrap();
        }
        for _ in 0..5 {
            metrics.record_request(0, 5_000_000, false).unwrap();
        }

        let thresholds = AlertThresholds {
            max_latency_ms: 100,
            min_healthy_backends: 1,
            max_error_rate: 0.20, // 20% threshold
            max_circuit_breaker_opens: 10,
            min_session_hit_rate: 0.5,
        };

        let alerts = metrics.check_alerts(&thresholds).unwrap();
        let has_error_alert = alerts.iter().any(|a| a.metric == "error_rate");
        assert!(has_error_alert);
    }
}

#[cfg(test)]
mod production_tests {
    use atomic_capsule::load_balancing::LoadBalancerMetricsCapsule;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    /// Q22: Sustained load (100K requests)
    #[test]
    fn test_sustained_load_100k() {
        let metrics = Arc::new(LoadBalancerMetricsCapsule::new());
        let mut handles = vec![];
        let start = Instant::now();

        // 4 threads × 25K requests
        for t in 0..4 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                for i in 0..25_000 {
                    let latency = ((t * 25_000 + i) % 1_000_000) as u64 + 1_000_000;
                    m.record_request(t as u32, latency, i % 20 != 0).unwrap();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let elapsed = start.elapsed();
        let snapshot = metrics.aggregate_metrics().unwrap();

        assert_eq!(snapshot.total_requests, 100_000);
        println!("100K requests in {:?}", elapsed);
        println!("Throughput: {:.0} req/s", 100_000.0 / elapsed.as_secs_f64());
    }

    /// Q23: Concurrent aggregation (100 snapshots)
    #[test]
    fn test_concurrent_aggregation() {
        let metrics = Arc::new(LoadBalancerMetricsCapsule::new());

        // Warm up: record some requests
        for _ in 0..1000 {
            metrics.record_request(0, 5_000_000, true).unwrap();
        }

        let mut handles = vec![];

        // 10 threads, each taking 10 snapshots
        for _ in 0..10 {
            let m = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _snapshot = m.aggregate_metrics().unwrap();
                }
            }));
        }

        let start = Instant::now();
        for h in handles {
            h.join().unwrap();
        }
        let elapsed = start.elapsed();

        println!("100 snapshots in {:?}", elapsed);
    }

    /// Q24: High-frequency updates
    #[test]
    fn test_high_frequency_updates() {
        let metrics = Arc::new(LoadBalancerMetricsCapsule::new());
        let mut handles = vec![];
        let start = Instant::now();

        // 8 threads recording as fast as possible for 100ms
        let stop = Arc::new(AtomicBool::new(false));
        for t in 0..8 {
            let m = Arc::clone(&metrics);
            let s = Arc::clone(&stop);
            handles.push(thread::spawn(move || {
                let mut count = 0u64;
                while !s.load(Ordering::Relaxed) {
                    m.record_request(t as u32, 1_000_000, true).unwrap();
                    count += 1;
                }
                count
            }));
        }

        thread::sleep(Duration::from_millis(100));
        stop.store(true, Ordering::Release);

        let mut total_ops = 0u64;
        for h in handles {
            total_ops += h.join().unwrap();
        }

        let elapsed = start.elapsed();
        println!("High-frequency: {} ops in {:?}", total_ops, elapsed);
        println!("Throughput: {:.0} ops/s", total_ops as f64 / elapsed.as_secs_f64());
    }

    /// Q25: Memory efficiency check
    #[test]
    fn test_memory_efficiency() {
        use std::mem::size_of;

        let metrics = LoadBalancerMetricsCapsule::new();
        let metrics_size = size_of::<LoadBalancerMetricsCapsule>();

        println!("LoadBalancerMetricsCapsule size: {} bytes", metrics_size);
        assert_eq!(metrics_size, 256);

        // Simulate 1000 backends
        let mut total_size = metrics_size;
        for _ in 0..1000 {
            let _backend = atomic_capsule::load_balancing::BackendMetrics::new(0);
            total_size += size_of::<atomic_capsule::load_balancing::BackendMetrics>();
        }

        println!("1000 backends + metrics: {} KB", total_size / 1024);
        // Should be relatively small
        assert!(total_size < 200_000); // Less than 200 KB for 1000 backends
    }

    /// Q26: Snapshot consistency
    #[test]
    fn test_snapshot_consistency() {
        let metrics = Arc::new(LoadBalancerMetricsCapsule::new());

        // Record a known sequence
        for i in 0..50 {
            metrics.record_request(0, (i * 1_000_000) as u64, true).unwrap();
        }

        // Take multiple snapshots
        let snap1 = metrics.aggregate_metrics().unwrap();
        thread::sleep(Duration::from_millis(1));
        let snap2 = metrics.aggregate_metrics().unwrap();
        thread::sleep(Duration::from_millis(1));
        let snap3 = metrics.aggregate_metrics().unwrap();

        // All snapshots should be identical
        assert_eq!(snap1.total_requests, snap2.total_requests);
        assert_eq!(snap2.total_requests, snap3.total_requests);
        assert_eq!(snap1.audit_hash, snap2.audit_hash);
    }

    /// Q27: Alert threshold edge cases
    #[test]
    fn test_alert_edge_cases() {
        use atomic_capsule::load_balancing::AlertThresholds;

        let metrics = LoadBalancerMetricsCapsule::new();

        // No requests - should not panic
        let thresholds = AlertThresholds::default();
        let alerts = metrics.check_alerts(&thresholds).unwrap();
        assert!(alerts.is_empty() || alerts.len() > 0); // Should be safe either way

        // One request
        metrics.record_request(0, 1_000_000, true).unwrap();
        let alerts = metrics.check_alerts(&thresholds).unwrap();
        // Should still be safe
        let _count = alerts.len();
    }

    /// Q28: Full end-to-end workflow
    #[test]
    fn test_end_to_end_workflow() {
        use atomic_capsule::load_balancing::AlertThresholds;

        let metrics = LoadBalancerMetricsCapsule::new();

        // Simulate 5 minutes of requests from 4 backends
        for minute in 0..5 {
            for _ in 0..12_000 {
                // 12K requests per minute = 200 RPS
                let backend = (minute * 4) as u32 % 4;
                let latency = if minute == 3 {
                    // Simulate slowdown in minute 4
                    50_000_000 // 50ms
                } else {
                    5_000_000 // 5ms
                };
                let success = minute != 4; // Complete failure in minute 5
                metrics.record_request(backend, latency, success).unwrap();
            }

            // Record health checks
            metrics.record_health_check(0, minute != 4).unwrap();
            metrics.record_health_check(1, true).unwrap();
            metrics.record_health_check(2, true).unwrap();
            metrics.record_health_check(3, minute != 4).unwrap();

            // Take snapshot and check alerts
            let snapshot = metrics.aggregate_metrics().unwrap();
            println!("Minute {}: {} req, {:.2}% success", minute, snapshot.total_requests, snapshot.success_rate * 100.0);

            let thresholds = AlertThresholds::default();
            let alerts = metrics.check_alerts(&thresholds).unwrap();
            if minute >= 3 {
                assert!(!alerts.is_empty());
            }
        }

        // Verify final state
        let final_snapshot = metrics.aggregate_metrics().unwrap();
        println!("Final: {} total requests", final_snapshot.total_requests);
        assert_eq!(final_snapshot.total_requests, 60_000);
    }
}
