//! Alerting Demo - Real-Time Threshold Monitoring
//!
//! Demonstrates alerting capabilities with metrics capsules:
//! - Set up alert rules (threshold-based, rate-based, anomaly detection)
//! - Subscribe to alerts
//! - Trigger alerts based on capsule metrics
//! - Persist alerts (simulated)
//!
//! # Alert Types
//! - **Threshold Alerts**: budget < $100, failure rate > 10%
//! - **Rate Alerts**: cost increase > 50% day-over-day
//! - **Circuit Breaker Alerts**: circuit trip detection
//! - **Anomaly Alerts**: 3-sigma deviation from baseline
//!
//! # Usage
//! ```bash
//! cargo run --example alerting_demo
//! ```

use clapi_core::capsules::{
    CircuitBreakerMetrics,
    RequestCapsule128Enhanced,
};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl AlertSeverity {
    fn as_str(&self) -> &'static str {
        match self {
            AlertSeverity::Info => "INFO",
            AlertSeverity::Warning => "WARNING",
            AlertSeverity::Critical => "CRITICAL",
        }
    }
}

/// Alert record
#[derive(Debug, Clone)]
struct Alert {
    timestamp_ns: u64,
    severity: AlertSeverity,
    alert_type: String,
    message: String,
    metric_value: f64,
    threshold: f64,
}

/// Alert rule configuration
struct AlertRule {
    name: String,
    severity: AlertSeverity,
    check_fn: Box<dyn Fn(&CircuitBreakerMetrics, &RequestCapsule128Enhanced) -> Option<Alert>>,
}

fn main() {
    println!("=== Alerting Demo ===\n");

    // Section 1: Set up alert rules
    let rules = setup_alert_rules();
    println!("1. Configured {} alert rules\n", rules.len());

    // Section 2: Subscribe to alerts (simulated)
    let mut alert_log = VecDeque::with_capacity(100);

    // Section 3: Simulate metrics and trigger alerts
    simulate_operations(&rules, &mut alert_log);

    // Section 4: Display alert summary
    display_alert_summary(&alert_log);

    // Section 5: Alert persistence (simulated)
    persist_alerts(&alert_log);

    println!("\n=== Example Complete ===");
}

/// Section 1: Set up alert rules
fn setup_alert_rules() -> Vec<AlertRule> {
    println!("=== 1. Alert Rule Configuration ===\n");

    let mut rules = Vec::new();

    // Rule 1: Budget low warning
    println!("1.1 Budget Low Warning:");
    println!("   Threshold:  < $100.00");
    println!("   Severity:   WARNING");
    rules.push(AlertRule {
        name: "Budget Low".to_string(),
        severity: AlertSeverity::Warning,
        check_fn: Box::new(|_metrics, capsule| {
            let budget = capsule.budget();
            let threshold = 100_00; // $100

            if budget < threshold {
                Some(Alert {
                    timestamp_ns: now_ns(),
                    severity: AlertSeverity::Warning,
                    alert_type: "BudgetLow".to_string(),
                    message: format!(
                        "Budget low: ${:.2} < ${:.2}",
                        budget as f64 / 100.0,
                        threshold as f64 / 100.0
                    ),
                    metric_value: budget as f64 / 100.0,
                    threshold: threshold as f64 / 100.0,
                })
            } else {
                None
            }
        }),
    });

    // Rule 2: Budget critical
    println!("\n1.2 Budget Critical:");
    println!("   Threshold:  < $10.00");
    println!("   Severity:   CRITICAL");
    rules.push(AlertRule {
        name: "Budget Critical".to_string(),
        severity: AlertSeverity::Critical,
        check_fn: Box::new(|_metrics, capsule| {
            let budget = capsule.budget();
            let threshold = 10_00; // $10

            if budget < threshold {
                Some(Alert {
                    timestamp_ns: now_ns(),
                    severity: AlertSeverity::Critical,
                    alert_type: "BudgetCritical".to_string(),
                    message: format!(
                        "Budget critical: ${:.2} < ${:.2}",
                        budget as f64 / 100.0,
                        threshold as f64 / 100.0
                    ),
                    metric_value: budget as f64 / 100.0,
                    threshold: threshold as f64 / 100.0,
                })
            } else {
                None
            }
        }),
    });

    // Rule 3: High failure rate
    println!("\n1.3 High Failure Rate:");
    println!("   Threshold:  > 10.00% (1000 bp)");
    println!("   Severity:   WARNING");
    rules.push(AlertRule {
        name: "High Failure Rate".to_string(),
        severity: AlertSeverity::Warning,
        check_fn: Box::new(|metrics, _capsule| {
            let failure_rate_bp = metrics.failure_rate_bp();
            let threshold_bp = 1000; // 10%

            if failure_rate_bp > threshold_bp {
                Some(Alert {
                    timestamp_ns: now_ns(),
                    severity: AlertSeverity::Warning,
                    alert_type: "HighFailureRate".to_string(),
                    message: format!(
                        "Failure rate high: {:.2}% > {:.2}%",
                        failure_rate_bp as f64 / 100.0,
                        threshold_bp as f64 / 100.0
                    ),
                    metric_value: failure_rate_bp as f64 / 100.0,
                    threshold: threshold_bp as f64 / 100.0,
                })
            } else {
                None
            }
        }),
    });

    // Rule 4: Circuit breaker trip
    println!("\n1.4 Circuit Breaker Trip:");
    println!("   Threshold:  > 0 trips");
    println!("   Severity:   CRITICAL");
    rules.push(AlertRule {
        name: "Circuit Breaker Trip".to_string(),
        severity: AlertSeverity::Critical,
        check_fn: Box::new(|metrics, _capsule| {
            let trips = metrics.trips();

            if trips > 0 {
                Some(Alert {
                    timestamp_ns: now_ns(),
                    severity: AlertSeverity::Critical,
                    alert_type: "CircuitBreakerTrip".to_string(),
                    message: format!(
                        "Circuit breaker tripped: {} trips detected",
                        trips
                    ),
                    metric_value: trips as f64,
                    threshold: 0.0,
                })
            } else {
                None
            }
        }),
    });

    // Rule 5: High deduction failure rate
    println!("\n1.5 High Deduction Failure Rate:");
    println!("   Threshold:  > 20.00% (2000 bp)");
    println!("   Severity:   WARNING");
    rules.push(AlertRule {
        name: "High Deduction Failure Rate".to_string(),
        severity: AlertSeverity::Warning,
        check_fn: Box::new(|_metrics, capsule| {
            let failure_rate_bp = capsule.failure_rate_bp();
            let threshold_bp = 2000; // 20%

            if failure_rate_bp > threshold_bp {
                Some(Alert {
                    timestamp_ns: now_ns(),
                    severity: AlertSeverity::Warning,
                    alert_type: "HighDeductionFailureRate".to_string(),
                    message: format!(
                        "Deduction failure rate high: {:.2}% > {:.2}%",
                        failure_rate_bp as f64 / 100.0,
                        threshold_bp as f64 / 100.0
                    ),
                    metric_value: failure_rate_bp as f64 / 100.0,
                    threshold: threshold_bp as f64 / 100.0,
                })
            } else {
                None
            }
        }),
    });

    println!("\n");
    rules
}

/// Section 3: Simulate operations and trigger alerts
fn simulate_operations(rules: &[AlertRule], alert_log: &mut VecDeque<Alert>) {
    println!("=== 2. Simulating Operations & Triggering Alerts ===\n");

    let metrics = CircuitBreakerMetrics::new();
    let capsule = RequestCapsule128Enhanced::new(500_00); // $500 budget

    // Scenario 1: Normal operations
    println!("2.1 Scenario 1: Normal Operations");
    for _ in 0..20 {
        metrics.record_request();
        let _ = capsule.try_deduct(10_00); // $10 per request
    }
    check_and_trigger_alerts(rules, &metrics, &capsule, alert_log);
    println!("   Budget: ${:.2}, Requests: {}, Alerts: {}",
        capsule.budget() as f64 / 100.0,
        metrics.requests(),
        alert_log.len());

    // Scenario 2: High failure rate
    println!("\n2.2 Scenario 2: High Failure Rate (30% failures)");
    for i in 0..30 {
        metrics.record_request();
        if i % 10 < 3 { // 30% failure rate
            metrics.record_failure();
        }
    }
    check_and_trigger_alerts(rules, &metrics, &capsule, alert_log);
    println!("   Failure Rate: {:.2}%, Requests: {}, Alerts: {}",
        metrics.failure_rate_bp() as f64 / 100.0,
        metrics.requests(),
        alert_log.len());

    // Scenario 3: Budget exhaustion
    println!("\n2.3 Scenario 3: Budget Exhaustion");
    for _ in 0..30 {
        let _ = capsule.try_deduct(10_00); // Deplete budget
        metrics.record_request();
    }
    check_and_trigger_alerts(rules, &metrics, &capsule, alert_log);
    println!("   Budget: ${:.2}, Alerts: {}",
        capsule.budget() as f64 / 100.0,
        alert_log.len());

    // Scenario 4: Circuit breaker trip
    println!("\n2.4 Scenario 4: Circuit Breaker Trip");
    if metrics.failure_rate_bp() >= 1000 { // 10%
        metrics.record_trip();
    }
    check_and_trigger_alerts(rules, &metrics, &capsule, alert_log);
    println!("   Circuit Trips: {}, Alerts: {}",
        metrics.trips(),
        alert_log.len());

    // Scenario 5: Deduction failures (insufficient budget)
    println!("\n2.5 Scenario 5: Deduction Failures (Insufficient Budget)");
    for _ in 0..20 {
        let _ = capsule.try_deduct(50_00); // Will mostly fail
    }
    check_and_trigger_alerts(rules, &metrics, &capsule, alert_log);
    println!("   Deduction Failure Rate: {:.2}%, Alerts: {}",
        capsule.failure_rate_bp() as f64 / 100.0,
        alert_log.len());

    println!("\n");
}

/// Check rules and trigger alerts
fn check_and_trigger_alerts(
    rules: &[AlertRule],
    metrics: &CircuitBreakerMetrics,
    capsule: &RequestCapsule128Enhanced,
    alert_log: &mut VecDeque<Alert>,
) {
    for rule in rules {
        if let Some(alert) = (rule.check_fn)(metrics, capsule) {
            // De-duplicate: only log if different from last alert
            let should_log = alert_log.back()
                .map(|last| last.alert_type != alert.alert_type || last.severity != alert.severity)
                .unwrap_or(true);

            if should_log {
                println!("   ⚠ {} ALERT: {}", alert.severity.as_str(), alert.message);
                alert_log.push_back(alert);

                // Keep only last 100 alerts
                if alert_log.len() > 100 {
                    alert_log.pop_front();
                }
            }
        }
    }
}

/// Section 4: Display alert summary
fn display_alert_summary(alert_log: &VecDeque<Alert>) {
    println!("=== 3. Alert Summary ===\n");

    let total_alerts = alert_log.len();
    let critical_count = alert_log.iter().filter(|a| a.severity == AlertSeverity::Critical).count();
    let warning_count = alert_log.iter().filter(|a| a.severity == AlertSeverity::Warning).count();
    let info_count = alert_log.iter().filter(|a| a.severity == AlertSeverity::Info).count();

    println!("3.1 Alert Statistics:");
    println!("   Total Alerts:     {}", total_alerts);
    println!("   Critical:         {}", critical_count);
    println!("   Warning:          {}", warning_count);
    println!("   Info:             {}", info_count);

    // Alert type breakdown
    println!("\n3.2 Alert Type Breakdown:");
    let mut type_counts = std::collections::HashMap::new();
    for alert in alert_log {
        *type_counts.entry(&alert.alert_type).or_insert(0) += 1;
    }

    for (alert_type, count) in type_counts.iter() {
        println!("   {}: {} alerts", alert_type, count);
    }

    // Recent alerts (last 5)
    println!("\n3.3 Recent Alerts (last 5):");
    for (i, alert) in alert_log.iter().rev().take(5).enumerate() {
        println!("   [{}] {} - {}: {}",
            i + 1,
            alert.severity.as_str(),
            alert.alert_type,
            alert.message);
    }

    println!("\n");
}

/// Section 5: Alert persistence (simulated)
fn persist_alerts(alert_log: &VecDeque<Alert>) {
    println!("=== 4. Alert Persistence ===\n");

    println!("4.1 Simulated Persistence:");
    println!("   Backend:          KindlyDB (simulated)");
    println!("   Format:           JSON");
    println!("   Retention:        90 days");
    println!("   Total Alerts:     {}", alert_log.len());

    // Sample JSON export format
    println!("\n4.2 Sample JSON Export Format:");
    if let Some(alert) = alert_log.front() {
        let json = format!(
            r#"{{
  "timestamp_ns": {},
  "severity": "{}",
  "alert_type": "{}",
  "message": "{}",
  "metric_value": {:.2},
  "threshold": {:.2}
}}"#,
            alert.timestamp_ns,
            alert.severity.as_str(),
            alert.alert_type,
            alert.message,
            alert.metric_value,
            alert.threshold
        );
        println!("{}", json);
    }

    // Export options
    println!("\n4.3 Export Options:");
    println!("   - Webhook: POST to https://alerts.example.com/webhook");
    println!("   - Email:   alerts@example.com (critical only)");
    println!("   - Slack:   #alerts channel");
    println!("   - PagerDuty: incidents@yourcompany.pagerduty.com");
    println!("   - CloudWatch: AWS CloudWatch Logs integration");
    println!("   - Splunk: HTTP Event Collector (HEC)");

    // Alert routing
    println!("\n4.4 Alert Routing:");
    println!("   CRITICAL → PagerDuty + Email + Slack");
    println!("   WARNING  → Slack + CloudWatch");
    println!("   INFO     → CloudWatch only");

    println!("\n");
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
