//! Real-time metrics dashboard with lockfree aggregation
//!
//! # Features
//! - Display metrics every 1 second
//! - Per-shard and cluster-wide aggregation
//! - Human-readable formatting
//! - Zero locks (100% atomic reads)
//!
//! # Example
//! ```no_run
//! use atomic_capsule::network::monitoring::{MetricsDashboard, GLOBAL_METRICS};
//!
//! // Start dashboard (spawns background thread)
//! let dashboard = MetricsDashboard::start(GLOBAL_METRICS);
//!
//! // Record metrics
//! GLOBAL_METRICS[0].record_operation(1_000_000);
//! GLOBAL_METRICS[0].record_hit();
//!
//! // Dashboard prints automatically every 1 second
//! // Stop dashboard
//! dashboard.stop();
//! ```

#![cfg(feature = "histogram")]

use crate::network::monitoring::metrics_capsule::{MetricsCapsule, MetricsSnapshot};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Global metrics array (3 shards for distributed cache)
pub static GLOBAL_METRICS: [MetricsCapsule; 3] = [
    MetricsCapsule::new(),
    MetricsCapsule::new(),
    MetricsCapsule::new(),
];

/// Real-time metrics dashboard
///
/// # Architecture
/// - Spawns background thread for display
/// - Polls metrics every 1 second
/// - 100% lockfree (atomic reads only)
/// - Human-readable table format
///
/// # Example
/// ```no_run
/// use atomic_capsule::network::monitoring::MetricsDashboard;
///
/// let dashboard = MetricsDashboard::start(&GLOBAL_METRICS);
/// // ... metrics recorded in background ...
/// dashboard.stop();
/// ```
pub struct MetricsDashboard {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

/// Cluster-wide aggregated metrics
#[derive(Debug, Clone)]
pub struct ClusterMetrics {
    pub total_throughput: f64,
    pub avg_p99_latency_us: f64,
    pub cluster_hit_ratio: f64,
    pub total_errors: u64,
    pub max_replication_lag_ms: f64,
    pub active_shards: usize,
}

impl MetricsDashboard {
    /// Start dashboard display (spawns background thread)
    ///
    /// # Performance
    /// - Polls every 1 second
    /// - <1ms display overhead per iteration
    /// - Zero blocking operations
    ///
    /// # Example
    /// ```no_run
    /// use atomic_capsule::network::monitoring::MetricsDashboard;
    ///
    /// let dashboard = MetricsDashboard::start(&GLOBAL_METRICS);
    /// // ... dashboard runs in background ...
    /// ```
    pub fn start(metrics: &'static [MetricsCapsule; 3]) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        let handle = thread::spawn(move || {
            // Initialize start time for all shards
            // (This is a workaround since we can't modify static shards)
            let start_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;

            while running_clone.load(Ordering::Relaxed) {
                // Sleep first to allow metrics to accumulate
                thread::sleep(Duration::from_secs(1));

                if !running_clone.load(Ordering::Relaxed) {
                    break;
                }

                // Display dashboard
                Self::display_dashboard(metrics, start_time);

                // Check alerts
                Self::check_alerts(metrics);
            }
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    /// Stop dashboard display
    ///
    /// # Example
    /// ```no_run
    /// let dashboard = MetricsDashboard::start(&GLOBAL_METRICS);
    /// // ... work ...
    /// dashboard.stop();
    /// ```
    pub fn stop(mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Display dashboard (internal implementation)
    fn display_dashboard(metrics: &[MetricsCapsule; 3], start_time: u64) {
        println!(
            "\n╔════════════════════════════════════════════════════════════════════════════╗"
        );
        println!("║           T8 Network Capsule Metrics Dashboard                             ║");
        println!("╚════════════════════════════════════════════════════════════════════════════╝");

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        println!("Timestamp: {}", timestamp);
        println!();

        // Per-shard metrics
        for (shard_id, shard) in metrics.iter().enumerate() {
            let snapshot = shard.snapshot();
            Self::display_shard_metrics(shard_id + 1, &snapshot, start_time);
        }

        // Cluster summary
        let cluster = Self::aggregate_cluster_metrics(metrics, start_time);
        Self::display_cluster_summary(&cluster);

        println!("╚════════════════════════════════════════════════════════════════════════════╝");
        println!();
    }

    /// Display shard-specific metrics
    fn display_shard_metrics(shard_id: usize, snapshot: &MetricsSnapshot, start_time: u64) {
        // Calculate elapsed time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let elapsed_ns = now.saturating_sub(start_time);
        let elapsed_secs = elapsed_ns as f64 / 1_000_000_000.0;

        // Calculate throughput
        let throughput = if elapsed_secs > 0.0 {
            snapshot.operations as f64 / elapsed_secs
        } else {
            0.0
        };

        println!(
            "┌─ Shard {} ─────────────────────────────────────────────────────────────────┐",
            shard_id
        );
        println!("│  Throughput:      {:>10.0} ops/sec", throughput);
        println!("│  P50 latency:     {:>10.2} µs", snapshot.p50_us());
        println!("│  P95 latency:     {:>10.2} µs", snapshot.p95_us());
        println!("│  P99 latency:     {:>10.2} µs", snapshot.p99_us());
        println!("│  P999 latency:    {:>10.2} µs", snapshot.p999_us());
        println!("│  Cache hit ratio: {:>10.1}%", snapshot.hit_ratio());
        println!("│  Error rate:      {:>10.2}%", snapshot.error_rate());
        println!(
            "│  Replication lag: {:>10.2} ms",
            snapshot.replication_lag_ms()
        );
        println!("│  Total ops:       {:>10}", snapshot.operations);
        println!("│  Errors:          {:>10}", snapshot.errors);

        // Alert indicators
        if snapshot.alert_latency {
            println!("│  ⚠️  ALERT: P99 latency > 10ms");
        }
        if snapshot.alert_errors {
            println!("│  ⚠️  ALERT: Error rate > 1%");
        }
        if snapshot.alert_hit_ratio {
            println!("│  ⚠️  ALERT: Hit ratio < 80%");
        }

        println!("└────────────────────────────────────────────────────────────────────────────┘");
        println!();
    }

    /// Aggregate cluster-wide metrics
    fn aggregate_cluster_metrics(metrics: &[MetricsCapsule; 3], start_time: u64) -> ClusterMetrics {
        let snapshots: Vec<MetricsSnapshot> = metrics.iter().map(|m| m.snapshot()).collect();

        // Calculate elapsed time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let elapsed_ns = now.saturating_sub(start_time);
        let elapsed_secs = elapsed_ns as f64 / 1_000_000_000.0;

        let total_ops: u64 = snapshots.iter().map(|s| s.operations).sum();
        let total_throughput = if elapsed_secs > 0.0 {
            total_ops as f64 / elapsed_secs
        } else {
            0.0
        };

        let avg_p99 = snapshots.iter().map(|s| s.p99_us()).sum::<f64>() / snapshots.len() as f64;

        let total_hits: u64 = snapshots.iter().map(|s| s.hits).sum();
        let total_misses: u64 = snapshots.iter().map(|s| s.misses).sum();
        let cluster_hit_ratio = if total_hits + total_misses > 0 {
            (total_hits as f64 / (total_hits + total_misses) as f64) * 100.0
        } else {
            100.0
        };

        let total_errors: u64 = snapshots.iter().map(|s| s.errors).sum();

        let max_replication_lag = snapshots
            .iter()
            .map(|s| s.replication_lag_ms())
            .fold(0.0f64, f64::max);

        let active_shards = snapshots.iter().filter(|s| s.operations > 0).count();

        ClusterMetrics {
            total_throughput,
            avg_p99_latency_us: avg_p99,
            cluster_hit_ratio,
            total_errors,
            max_replication_lag_ms: max_replication_lag,
            active_shards,
        }
    }

    /// Display cluster summary
    fn display_cluster_summary(cluster: &ClusterMetrics) {
        println!("┌─ Cluster Summary ──────────────────────────────────────────────────────────┐");
        println!(
            "│  Total throughput:  {:>10.0} ops/sec",
            cluster.total_throughput
        );
        println!(
            "│  Avg P99 latency:   {:>10.2} µs",
            cluster.avg_p99_latency_us
        );
        println!("│  Cluster hit ratio: {:>10.1}%", cluster.cluster_hit_ratio);
        println!("│  Total errors:      {:>10}", cluster.total_errors);
        println!(
            "│  Max replication lag: {:>8.2} ms",
            cluster.max_replication_lag_ms
        );
        println!("│  Active shards:     {:>10}/{}", cluster.active_shards, 3);
        println!("└────────────────────────────────────────────────────────────────────────────┘");
    }

    /// Check alerts across all shards
    fn check_alerts(metrics: &[MetricsCapsule; 3]) {
        for (shard_id, shard) in metrics.iter().enumerate() {
            shard.check_alerts();

            let snapshot = shard.snapshot();
            if snapshot.alert_latency {
                eprintln!(
                    "⚠️  [Shard {}] ALERT: P99 latency ({:.2} µs) exceeds 10ms threshold",
                    shard_id + 1,
                    snapshot.p99_us()
                );
            }
            if snapshot.alert_errors {
                eprintln!(
                    "⚠️  [Shard {}] ALERT: Error rate ({:.2}%) exceeds 1% threshold",
                    shard_id + 1,
                    snapshot.error_rate()
                );
            }
            if snapshot.alert_hit_ratio {
                eprintln!(
                    "⚠️  [Shard {}] ALERT: Hit ratio ({:.1}%) below 80% threshold",
                    shard_id + 1,
                    snapshot.hit_ratio()
                );
            }
        }
    }
}

impl Drop for MetricsDashboard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_aggregation() {
        let metrics: [MetricsCapsule; 3] = [
            MetricsCapsule::new(),
            MetricsCapsule::new(),
            MetricsCapsule::new(),
        ];

        // Record operations
        for i in 0..100 {
            metrics[0].record_operation(1_000_000 + i * 1000);
            metrics[0].record_hit();
        }
        for i in 0..50 {
            metrics[1].record_operation(2_000_000 + i * 1000);
            metrics[1].record_miss();
        }
        for i in 0..75 {
            metrics[2].record_operation(3_000_000 + i * 1000);
            metrics[2].record_hit();
        }

        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let cluster = MetricsDashboard::aggregate_cluster_metrics(&metrics, start_time);

        assert_eq!(cluster.active_shards, 3);
        assert!(cluster.total_throughput > 0.0);
    }

    #[test]
    fn test_display_formatting() {
        let metrics: [MetricsCapsule; 3] = [
            MetricsCapsule::new(),
            MetricsCapsule::new(),
            MetricsCapsule::new(),
        ];

        // Record some metrics
        metrics[0].record_operation(1_000_000);
        metrics[0].record_hit();
        metrics[1].record_operation(2_000_000);
        metrics[1].record_miss();

        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        // Test display (visual inspection in test output)
        MetricsDashboard::display_dashboard(&metrics, start_time);
    }
}
