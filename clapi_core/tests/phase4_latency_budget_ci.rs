//! T28 CI Enforcement: Latency Budget Validation (E27)
//!
//! **Purpose**: CI/CD pipeline latency budget enforcement
//! - Fails build if any capsule exceeds latency budget
//! - Enforces p50, p99, p99.9, p99.99 budgets from B32 framework
//! - Automatic regression detection in CI/CD
//!
//! **Framework Compliance**:
//! - ✅ T28 Q26: Performance regression detection
//! - ✅ B32: Fair baselines, 95% CI, statistical rigor
//! - ✅ UCE34: Latency budgets enforced in CI/CD

use clapi_core::capsules::{
    OAuthSessionCapsule, PaymentCapsule256, RateLimitCapsule,
};
use clapi_core::auth::OAuthStateCapsule;
use std::time::Instant;

// ============================================================================
// Latency Budget Definitions (B32 Framework)
// ============================================================================

struct LatencyBudget {
    operation: &'static str,
    p50_ns: u64,
    p99_ns: u64,
    p99_9_ns: u64,
    p99_99_ns: u64,
}

impl LatencyBudget {
    const OAUTH_SESSION_CREATE: LatencyBudget = LatencyBudget {
        operation: "OAuth session creation",
        p50_ns: 200,
        p99_ns: 500,
        p99_9_ns: 1_000,
        p99_99_ns: 5_000,
    };

    const OAUTH_SESSION_VALIDATE: LatencyBudget = LatencyBudget {
        operation: "OAuth session validation",
        p50_ns: 50,
        p99_ns: 100,
        p99_9_ns: 200,
        p99_99_ns: 500,
    };

    const OAUTH_PKCE_GENERATE: LatencyBudget = LatencyBudget {
        operation: "PKCE generation",
        p50_ns: 5_000,
        p99_ns: 10_000,
        p99_9_ns: 20_000,
        p99_99_ns: 50_000,
    };

    const PAYMENT_CREATE: LatencyBudget = LatencyBudget {
        operation: "Payment creation",
        p50_ns: 100,
        p99_ns: 200,
        p99_9_ns: 500,
        p99_99_ns: 1_000,
    };

    const PAYMENT_CONFIRM: LatencyBudget = LatencyBudget {
        operation: "Payment confirmation",
        p50_ns: 150,
        p99_ns: 300,
        p99_9_ns: 600,
        p99_99_ns: 1_500,
    };

    const RATELIMIT_CHECK: LatencyBudget = LatencyBudget {
        operation: "Rate limit check",
        p50_ns: 20,
        p99_ns: 40,
        p99_9_ns: 80,
        p99_99_ns: 200,
    };

    const RATELIMIT_INCREMENT: LatencyBudget = LatencyBudget {
        operation: "Rate limit increment",
        p50_ns: 30,
        p99_ns: 60,
        p99_9_ns: 120,
        p99_99_ns: 300,
    };
}

// ============================================================================
// CI Enforcement Tests - OAuth Session
// ============================================================================

#[test]
fn test_ci_latency_budget_oauth_session_create() {
    let budget = LatencyBudget::OAUTH_SESSION_CREATE;
    let iterations = 100_000;
    let mut latencies = Vec::with_capacity(iterations);

    for user_id in 0..iterations {
        let start = Instant::now();
        let _session = OAuthSessionCapsule::new(user_id as u64, user_id as u64, None);
        latencies.push(start.elapsed().as_nanos() as u64);
    }

    latencies.sort();

    let p50 = latencies[iterations / 2];
    let p99 = latencies[(iterations * 99) / 100];
    let p99_9 = latencies[(iterations * 999) / 1000];
    let p99_99 = latencies[(iterations * 9999) / 10000];

    println!(
        "{}: p50={}ns, p99={}ns, p99.9={}ns, p99.99={}ns",
        budget.operation, p50, p99, p99_9, p99_99
    );

    // CI enforcement: Fail build if budget exceeded
    assert!(
        p50 <= budget.p50_ns,
        "❌ CI FAIL: {} p50 budget exceeded: {}ns > {}ns",
        budget.operation,
        p50,
        budget.p50_ns
    );
    assert!(
        p99 <= budget.p99_ns,
        "❌ CI FAIL: {} p99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99,
        budget.p99_ns
    );
    assert!(
        p99_9 <= budget.p99_9_ns,
        "❌ CI FAIL: {} p99.9 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_9,
        budget.p99_9_ns
    );
    assert!(
        p99_99 <= budget.p99_99_ns,
        "❌ CI FAIL: {} p99.99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_99,
        budget.p99_99_ns
    );
}

#[test]
fn test_ci_latency_budget_oauth_session_validate() {
    let budget = LatencyBudget::OAUTH_SESSION_VALIDATE;
    let iterations = 100_000;
    let mut latencies = Vec::with_capacity(iterations);

    let session = OAuthSessionCapsule::new(1001, 0xABCDEF, None);
    let token_hash = 0xABCDEF;

    for _ in 0..iterations {
        let start = Instant::now();
        let _ = session.verify_token(token_hash);
        latencies.push(start.elapsed().as_nanos() as u64);
    }

    latencies.sort();

    let p50 = latencies[iterations / 2];
    let p99 = latencies[(iterations * 99) / 100];
    let p99_9 = latencies[(iterations * 999) / 1000];
    let p99_99 = latencies[(iterations * 9999) / 10000];

    println!(
        "{}: p50={}ns, p99={}ns, p99.9={}ns, p99.99={}ns",
        budget.operation, p50, p99, p99_9, p99_99
    );

    assert!(
        p50 <= budget.p50_ns,
        "❌ CI FAIL: {} p50 budget exceeded: {}ns > {}ns",
        budget.operation,
        p50,
        budget.p50_ns
    );
    assert!(
        p99 <= budget.p99_ns,
        "❌ CI FAIL: {} p99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99,
        budget.p99_ns
    );
    assert!(
        p99_9 <= budget.p99_9_ns,
        "❌ CI FAIL: {} p99.9 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_9,
        budget.p99_9_ns
    );
    assert!(
        p99_99 <= budget.p99_99_ns,
        "❌ CI FAIL: {} p99.99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_99,
        budget.p99_99_ns
    );
}

#[test]
fn test_ci_latency_budget_pkce_generate() {
    let budget = LatencyBudget::OAUTH_PKCE_GENERATE;
    let iterations = 10_000; // Fewer iterations for crypto-heavy operation
    let mut latencies = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let start = Instant::now();
        let _pkce = OAuthStateCapsule::generate_pkce();
        latencies.push(start.elapsed().as_nanos() as u64);
    }

    latencies.sort();

    let p50 = latencies[iterations / 2];
    let p99 = latencies[(iterations * 99) / 100];
    let p99_9 = latencies[(iterations * 999) / 1000];
    let p99_99 = latencies[(iterations * 9999) / 10000];

    println!(
        "{}: p50={}ns, p99={}ns, p99.9={}ns, p99.99={}ns",
        budget.operation, p50, p99, p99_9, p99_99
    );

    assert!(
        p50 <= budget.p50_ns,
        "❌ CI FAIL: {} p50 budget exceeded: {}ns > {}ns",
        budget.operation,
        p50,
        budget.p50_ns
    );
    assert!(
        p99 <= budget.p99_ns,
        "❌ CI FAIL: {} p99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99,
        budget.p99_ns
    );
    assert!(
        p99_9 <= budget.p99_9_ns,
        "❌ CI FAIL: {} p99.9 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_9,
        budget.p99_9_ns
    );
    assert!(
        p99_99 <= budget.p99_99_ns,
        "❌ CI FAIL: {} p99.99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_99,
        budget.p99_99_ns
    );
}

// ============================================================================
// CI Enforcement Tests - Payment
// ============================================================================

#[test]
fn test_ci_latency_budget_payment_create() {
    let budget = LatencyBudget::PAYMENT_CREATE;
    let iterations = 100_000;
    let mut latencies = Vec::with_capacity(iterations);

    for i in 0..iterations {
        let start = Instant::now();
        let _payment = PaymentCapsule256::new(i as u64, i as u64, 1_000_00);
        latencies.push(start.elapsed().as_nanos() as u64);
    }

    latencies.sort();

    let p50 = latencies[iterations / 2];
    let p99 = latencies[(iterations * 99) / 100];
    let p99_9 = latencies[(iterations * 999) / 1000];
    let p99_99 = latencies[(iterations * 9999) / 10000];

    println!(
        "{}: p50={}ns, p99={}ns, p99.9={}ns, p99.99={}ns",
        budget.operation, p50, p99, p99_9, p99_99
    );

    assert!(
        p50 <= budget.p50_ns,
        "❌ CI FAIL: {} p50 budget exceeded: {}ns > {}ns",
        budget.operation,
        p50,
        budget.p50_ns
    );
    assert!(
        p99 <= budget.p99_ns,
        "❌ CI FAIL: {} p99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99,
        budget.p99_ns
    );
    assert!(
        p99_9 <= budget.p99_9_ns,
        "❌ CI FAIL: {} p99.9 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_9,
        budget.p99_9_ns
    );
    assert!(
        p99_99 <= budget.p99_99_ns,
        "❌ CI FAIL: {} p99.99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_99,
        budget.p99_99_ns
    );
}

#[test]
fn test_ci_latency_budget_payment_confirm() {
    let budget = LatencyBudget::PAYMENT_CONFIRM;
    let iterations = 100_000;
    let mut latencies = Vec::with_capacity(iterations);

    for i in 0..iterations {
        let payment = PaymentCapsule256::new(i as u64, i as u64, 1_000_00);
        payment.start_processing().unwrap();

        let start = Instant::now();
        payment.confirm_payment().unwrap();
        latencies.push(start.elapsed().as_nanos() as u64);
    }

    latencies.sort();

    let p50 = latencies[iterations / 2];
    let p99 = latencies[(iterations * 99) / 100];
    let p99_9 = latencies[(iterations * 999) / 1000];
    let p99_99 = latencies[(iterations * 9999) / 10000];

    println!(
        "{}: p50={}ns, p99={}ns, p99.9={}ns, p99.99={}ns",
        budget.operation, p50, p99, p99_9, p99_99
    );

    assert!(
        p50 <= budget.p50_ns,
        "❌ CI FAIL: {} p50 budget exceeded: {}ns > {}ns",
        budget.operation,
        p50,
        budget.p50_ns
    );
    assert!(
        p99 <= budget.p99_ns,
        "❌ CI FAIL: {} p99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99,
        budget.p99_ns
    );
    assert!(
        p99_9 <= budget.p99_9_ns,
        "❌ CI FAIL: {} p99.9 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_9,
        budget.p99_9_ns
    );
    assert!(
        p99_99 <= budget.p99_99_ns,
        "❌ CI FAIL: {} p99.99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_99,
        budget.p99_99_ns
    );
}

// ============================================================================
// CI Enforcement Tests - Rate Limiting
// ============================================================================

#[test]
fn test_ci_latency_budget_ratelimit_check() {
    let budget = LatencyBudget::RATELIMIT_CHECK;
    let iterations = 100_000;
    let mut latencies = Vec::with_capacity(iterations);

    let limiter = RateLimitCapsule::with_quota(1_000_000);

    for _ in 0..iterations {
        let start = Instant::now();
        let _ = limiter.check_rate_limit();
        latencies.push(start.elapsed().as_nanos() as u64);
    }

    latencies.sort();

    let p50 = latencies[iterations / 2];
    let p99 = latencies[(iterations * 99) / 100];
    let p99_9 = latencies[(iterations * 999) / 1000];
    let p99_99 = latencies[(iterations * 9999) / 10000];

    println!(
        "{}: p50={}ns, p99={}ns, p99.9={}ns, p99.99={}ns",
        budget.operation, p50, p99, p99_9, p99_99
    );

    assert!(
        p50 <= budget.p50_ns,
        "❌ CI FAIL: {} p50 budget exceeded: {}ns > {}ns",
        budget.operation,
        p50,
        budget.p50_ns
    );
    assert!(
        p99 <= budget.p99_ns,
        "❌ CI FAIL: {} p99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99,
        budget.p99_ns
    );
    assert!(
        p99_9 <= budget.p99_9_ns,
        "❌ CI FAIL: {} p99.9 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_9,
        budget.p99_9_ns
    );
    assert!(
        p99_99 <= budget.p99_99_ns,
        "❌ CI FAIL: {} p99.99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_99,
        budget.p99_99_ns
    );
}

#[test]
fn test_ci_latency_budget_ratelimit_increment() {
    let budget = LatencyBudget::RATELIMIT_INCREMENT;
    let iterations = 100_000;
    let mut latencies = Vec::with_capacity(iterations);

    let limiter = RateLimitCapsule::with_quota(1_000_000);

    for _ in 0..iterations {
        let start = Instant::now();
        let _ = limiter.increment_request();
        latencies.push(start.elapsed().as_nanos() as u64);
    }

    latencies.sort();

    let p50 = latencies[iterations / 2];
    let p99 = latencies[(iterations * 99) / 100];
    let p99_9 = latencies[(iterations * 999) / 1000];
    let p99_99 = latencies[(iterations * 9999) / 10000];

    println!(
        "{}: p50={}ns, p99={}ns, p99.9={}ns, p99.99={}ns",
        budget.operation, p50, p99, p99_9, p99_99
    );

    assert!(
        p50 <= budget.p50_ns,
        "❌ CI FAIL: {} p50 budget exceeded: {}ns > {}ns",
        budget.operation,
        p50,
        budget.p50_ns
    );
    assert!(
        p99 <= budget.p99_ns,
        "❌ CI FAIL: {} p99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99,
        budget.p99_ns
    );
    assert!(
        p99_9 <= budget.p99_9_ns,
        "❌ CI FAIL: {} p99.9 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_9,
        budget.p99_9_ns
    );
    assert!(
        p99_99 <= budget.p99_99_ns,
        "❌ CI FAIL: {} p99.99 budget exceeded: {}ns > {}ns",
        budget.operation,
        p99_99,
        budget.p99_99_ns
    );
}

// ============================================================================
// CI Statistical Validation (B32 Framework)
// ============================================================================

#[test]
fn test_ci_statistical_rigor_validation() {
    // Validate statistical rigor: >1000 iterations, 95% CI

    let iterations = 100_000;
    assert!(
        iterations >= 1000,
        "CI FAIL: Insufficient iterations for statistical rigor: {} < 1000",
        iterations
    );

    // Calculate 95% confidence interval
    let mut latencies = Vec::with_capacity(iterations);

    for i in 0..iterations {
        let start = Instant::now();
        let _session = OAuthSessionCapsule::new(i as u64, i as u64, None);
        latencies.push(start.elapsed().as_nanos() as f64);
    }

    let mean: f64 = latencies.iter().sum::<f64>() / iterations as f64;
    let variance: f64 = latencies
        .iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f64>()
        / (iterations - 1) as f64;
    let stddev = variance.sqrt();
    let stderr = stddev / (iterations as f64).sqrt();
    let ci_95 = 1.96 * stderr; // 95% confidence interval

    println!(
        "Statistical validation: mean={:.2}ns, stddev={:.2}ns, 95% CI=±{:.2}ns",
        mean, stddev, ci_95
    );

    // CI should be tight (low variance)
    assert!(
        ci_95 / mean < 0.05,
        "CI FAIL: 95% confidence interval too wide: {:.2}% > 5%",
        (ci_95 / mean) * 100.0
    );
}
