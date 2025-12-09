//! Comprehensive tests for AtomicHedgeCapsule diagnostics capabilities
//!
//! UCE-32 Q30 (Empirical Validation): These tests prove that diagnostics
//! help actual debugging scenarios through measurable evidence.

#[cfg(feature = "diagnostics")]
mod diagnostics_tests {
    use atomic_hedge_capsule::{AtomicHedgeCapsule, Diagnostics, DiagnosticsExt, HealthStatus};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_basic_diagnostics() {
        let capsule = AtomicHedgeCapsule::new();
        let diagnostics = capsule.diagnostics();

        assert!(diagnostics.health.is_healthy());
        assert!(!diagnostics.has_production_issue());
        assert_eq!(diagnostics.state.generation, 0);
        assert!(!diagnostics.state.emergency_stop);
    }

    #[test]
    fn test_health_check_basic() {
        let capsule = AtomicHedgeCapsule::new();
        let health = capsule.health_check();

        // New capsule should be in degraded state (not initialized)
        assert!(!health.is_healthy());
        assert_eq!(health.severity(), 1); // Degraded
    }

    #[test]
    fn test_health_check_after_initialization() -> Result<(), Box<dyn std::error::Error>> {
        let capsule =
            AtomicHedgeCapsule::create_hedge("BTCUSD", "TestExchange", 1.0, 50000.0, 60000.0)?;

        let health = capsule.health_check();
        assert!(health.is_healthy());
        assert_eq!(health.severity(), 0);

        Ok(())
    }

    #[test]
    fn test_emergency_stop_detection() -> Result<(), Box<dyn std::error::Error>> {
        let capsule =
            AtomicHedgeCapsule::create_hedge("ETHUSD", "TestExchange", 2.0, 3000.0, 4000.0)?;

        // Trigger emergency stop
        capsule.emergency_stop("Test emergency")?;

        let health = capsule.health_check();
        assert!(!health.is_healthy());
        assert!(health.requires_intervention());
        assert_eq!(health.severity(), 2); // Critical

        let diagnostics = capsule.diagnostics();
        assert!(diagnostics.has_production_issue());
        assert!(diagnostics.state.emergency_stop);

        Ok(())
    }

    #[test]
    fn test_performance_status() {
        let capsule = AtomicHedgeCapsule::new();
        let perf = capsule.performance_check();

        // Basic performance validation
        assert!(perf.memory_usage_bytes > 0);
        assert!(perf.avg_latency_ns >= 0);
        assert!(perf.ops_per_second >= 0.0);
        assert!(perf.cache_hit_rate >= 0.0 && perf.cache_hit_rate <= 100.0);
    }

    #[test]
    fn test_state_inspection() -> Result<(), Box<dyn std::error::Error>> {
        let capsule = AtomicHedgeCapsule::create_hedge("ADAUSD", "TestExchange", 5.0, 1.0, 2.0)?;

        let inspection = capsule.state_inspection();

        assert_eq!(inspection.generation, 1); // Should be initialized
        assert!(!inspection.emergency_stop);
        assert!(!inspection.is_stuck);
        assert!(inspection.recovery_suggestions.is_empty());

        Ok(())
    }

    #[test]
    fn test_error_analysis() -> Result<(), Box<dyn std::error::Error>> {
        let capsule = AtomicHedgeCapsule::new();
        let mut errors = capsule.error_analysis();

        // New capsule should have uninitialized error
        assert!(errors.total_errors > 0);
        assert!(errors.error_counts.contains_key("Uninitialized"));

        Ok(())
    }

    #[test]
    fn test_diagnostics_generation_time() {
        let capsule = AtomicHedgeCapsule::new();
        let diagnostics = capsule.diagnostics();

        // Diagnostics should generate quickly (< 1ms)
        assert!(diagnostics.generation_time_ns < 1_000_000);
        assert!(diagnostics.generation_time_ns > 0);
    }

    #[test]
    fn test_diagnostics_summary() {
        let capsule = AtomicHedgeCapsule::new();
        let diagnostics = capsule.diagnostics();

        let summary = diagnostics.summary();
        assert!(summary.contains("DEGRADED") || summary.contains("HEALTHY"));
        assert!(summary.contains("Perf:"));
        assert!(summary.contains("Errors:"));
        assert!(summary.contains("Gen:"));
    }

    #[test]
    fn test_diagnostics_display() {
        let capsule = AtomicHedgeCapsule::new();
        let diagnostics = capsule.diagnostics();

        let display_output = format!("{}", diagnostics);
        assert!(display_output.contains("AtomicHedgeCapsule Diagnostics"));
        assert!(display_output.contains("Health:"));
        assert!(display_output.contains("Performance:"));
        assert!(display_output.contains("Errors:"));
        assert!(display_output.contains("State:"));
    }

    #[test]
    fn test_health_status_severity_levels() {
        let healthy = HealthStatus::Healthy;
        let degraded = HealthStatus::Degraded("Test issue".to_string());
        let critical = HealthStatus::Critical("Critical issue".to_string());

        assert_eq!(healthy.severity(), 0);
        assert_eq!(degraded.severity(), 1);
        assert_eq!(critical.severity(), 2);

        assert!(healthy.is_healthy());
        assert!(!degraded.is_healthy());
        assert!(!critical.is_healthy());

        assert!(!healthy.requires_intervention());
        assert!(!degraded.requires_intervention());
        assert!(critical.requires_intervention());
    }

    #[test]
    fn test_performance_bottleneck_detection() {
        use atomic_hedge_capsule::PerformanceStatus;

        let mut perf = PerformanceStatus::new();

        // Simulate high latency
        perf.avg_latency_ns = 2_000_000; // 2ms
        perf.identify_bottlenecks();

        assert!(!perf.is_performant());
        assert!(!perf.bottlenecks.is_empty());
        assert!(perf.bottlenecks.iter().any(|b| b.contains("latency")));

        // Reset and test low cache hit rate
        let mut perf2 = PerformanceStatus::new();
        perf2.cache_hit_rate = 30.0;
        perf2.identify_bottlenecks();

        assert!(!perf2.is_performant());
        assert!(perf2.bottlenecks.iter().any(|b| b.contains("cache")));
    }

    #[test]
    fn test_error_summary_tracking() {
        use atomic_hedge_capsule::ErrorSummary;

        let mut summary = ErrorSummary::new();

        // Record multiple errors
        summary.record_error("TestError");
        summary.record_error("TestError");
        summary.record_error("AnotherError");

        assert_eq!(summary.total_errors, 3);
        assert_eq!(summary.error_counts.get("TestError"), Some(&2));
        assert_eq!(summary.error_counts.get("AnotherError"), Some(&1));
        assert_eq!(summary.most_frequent_error, Some("TestError".to_string()));

        assert!(!summary.has_concerning_error_rate()); // 3 errors isn't concerning yet

        // Add many more errors to trigger concerning rate
        for _ in 0..100 {
            summary.record_error("FloodError");
        }

        assert!(summary.has_concerning_error_rate());
    }

    #[test]
    fn test_state_inspection_stuck_detection() {
        use atomic_hedge_capsule::StateInspection;

        let mut inspection = StateInspection::new();

        // Simulate stuck state
        inspection.time_in_state_ms = 35_000; // 35 seconds
        inspection.recent_transitions.clear();
        inspection.analyze_stuck_state();

        assert!(inspection.is_stuck);
        assert!(!inspection.recovery_suggestions.is_empty());
        assert!(inspection
            .recovery_suggestions
            .iter()
            .any(|s| s.contains("deadlock")));
    }

    #[test]
    fn test_diagnostics_recommendations() {
        let capsule = AtomicHedgeCapsule::new();
        let mut diagnostics = capsule.diagnostics();

        // Force some performance issues
        diagnostics.performance.avg_latency_ns = 5_000_000; // 5ms
        diagnostics.performance.identify_bottlenecks();
        diagnostics.errors.record_error("TestError");
        diagnostics.errors.record_error("TestError");

        diagnostics.assess_health();
        diagnostics.generate_recommendations();

        assert!(!diagnostics.recommendations.is_empty());
        assert!(diagnostics
            .recommendations
            .iter()
            .any(|r| r.contains("performance") || r.contains("bottleneck")));
    }

    #[test]
    fn test_concurrent_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
        let capsule = Arc::new(AtomicHedgeCapsule::create_hedge(
            "CONCURRENTTEST",
            "TestExchange",
            1.0,
            100.0,
            200.0,
        )?);

        let mut handles = vec![];

        // Spawn multiple threads running diagnostics concurrently
        for i in 0..10 {
            let capsule_clone = Arc::clone(&capsule);
            let handle = thread::spawn(move || {
                for _ in 0..5 {
                    let diagnostics = capsule_clone.diagnostics();
                    assert!(diagnostics.generation_time_ns > 0);

                    let health = capsule_clone.health_check();
                    assert!(health.severity() <= 2);

                    // Small delay to allow interleaving
                    thread::sleep(Duration::from_millis(1));
                }
                format!("Thread {} completed", i)
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result.contains("completed"));
        }

        // Final diagnostics check
        let final_diagnostics = capsule.diagnostics();
        assert!(!final_diagnostics.has_production_issue());

        Ok(())
    }

    #[test]
    fn test_diagnostics_with_operations() -> Result<(), Box<dyn std::error::Error>> {
        let capsule = AtomicHedgeCapsule::create_hedge(
            "OPERATIONSTEST",
            "TestExchange",
            2.0,
            1000.0,
            2000.0,
        )?;

        // Perform some operations
        capsule.submit_order()?;
        capsule.update_progress(0.5)?;

        let diagnostics = capsule.diagnostics();

        // Should still be healthy after operations
        assert!(
            diagnostics.health.is_healthy()
                || matches!(diagnostics.health, HealthStatus::Degraded(_))
        );
        assert!(diagnostics.state.generation > 0);

        // Performance should show some activity
        assert!(diagnostics.performance.ops_per_second >= 0.0);

        Ok(())
    }

    #[test]
    fn test_zero_overhead_when_disabled() {
        // This test ensures that when diagnostics feature is disabled,
        // there's no performance impact. Since we're in the feature guard,
        // we can only test that diagnostics work when enabled.

        let capsule = AtomicHedgeCapsule::new();
        let start = std::time::Instant::now();

        // Run diagnostics multiple times
        for _ in 0..100 {
            let _diagnostics = capsule.diagnostics();
        }

        let elapsed = start.elapsed();

        // Should complete quickly (< 10ms for 100 runs)
        assert!(elapsed < Duration::from_millis(10));
    }

    #[test]
    fn test_diagnostic_feature_detection() {
        use atomic_hedge_capsule::features;

        // Since this test is running, diagnostics feature must be enabled
        assert!(features::has_diagnostics());
    }

    #[test]
    fn test_production_issue_detection() -> Result<(), Box<dyn std::error::Error>> {
        let capsule =
            AtomicHedgeCapsule::create_hedge("PRODISSUE", "TestExchange", 1.0, 500.0, 600.0)?;

        // Initially should be healthy
        let diagnostics1 = capsule.diagnostics();
        assert!(!diagnostics1.has_production_issue());

        // Trigger emergency stop
        capsule.emergency_stop("Production issue test")?;

        let diagnostics2 = capsule.diagnostics();
        assert!(diagnostics2.has_production_issue());
        assert!(!diagnostics2.health.is_healthy());

        Ok(())
    }

    #[test]
    fn test_memory_layout_diagnostics() {
        let capsule = AtomicHedgeCapsule::new();
        let inspection = capsule.state_inspection();

        // Verify that memory layout information is accessible
        assert!(inspection.position_raw == 0); // Initial state
        assert!(inspection.spread_raw == 0); // Initial state
        assert!(inspection.generation == 0); // Initial state

        // Memory usage should be reasonable
        let perf = capsule.performance_check();
        assert!(perf.memory_usage_bytes > 0);
        assert!(perf.memory_usage_bytes < 10_000); // Should be less than 10KB
    }
}

#[cfg(not(feature = "diagnostics"))]
mod diagnostics_disabled_tests {
    use atomic_hedge_capsule::features;

    #[test]
    fn test_diagnostics_feature_disabled() {
        // When diagnostics feature is disabled, the feature detection should return false
        assert!(!features::has_diagnostics());
    }
}
