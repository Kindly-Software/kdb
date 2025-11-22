//! Validate P2 Adaptive Circuit Breaker: 50% False Positive Reduction
//!
//! **Goal**: Empirically demonstrate that adaptive EMA-based thresholds reduce
//! false positive circuit trips by 40-60% compared to static thresholds.
//!
//! **Methodology**:
//! - Hardcoded 1000-evaluation workload with known transient spikes
//! - Static policy: Fixed mu/sigma thresholds (baseline)
//! - Adaptive policy: EMA-learned thresholds (P2 innovation)
//! - Report: False positive rate comparison
//!
//! **Expected**: Static 45-55% FP → Adaptive <30% FP = 40-60% reduction

#![cfg(all(feature = "circuit-breaker-auto-tune", feature = "nightly"))]

use atomic_capsule::patterns::circuit_breaker::{evaluate, CircuitBreaker, Policy, State};

#[cfg(feature = "circuit_breaker_auto_tune")]
use atomic_capsule::patterns::circuit_breaker::{
    AutoCalibrator, CalibrationMode, CalibrationTargets, HistoryBuffer,
};

/// Simulated workload: 1000 evaluations with known transient spikes
///
/// Pattern: 10 transient spikes (recover within 5 evaluations) + 990 normal ops
/// Ground truth: Transient spikes should NOT trip circuit (false positives)
struct Workload {
    evaluations: Vec<Evaluation>,
}

struct Evaluation {
    mu: f64,            // Mean latency
    sigma: f64,         // Stddev latency
    err_inc: u16,       // Error increment
    is_transient: bool, // Ground truth: transient spike (should not trip)
}

impl Workload {
    fn new() -> Self {
        let mut evaluations = Vec::with_capacity(1000);

        // Normal baseline: mu=1ms, sigma=0.1ms, no errors
        for i in 0..1000 {
            if i % 100 == 0 && i < 500 {
                // Transient spike: mu=3ms (3× baseline), sigma=0.5ms, 1 error
                // These should NOT trip circuit (recovers quickly)
                evaluations.push(Evaluation {
                    mu: 3.0,
                    sigma: 0.5,
                    err_inc: 1,
                    is_transient: true,
                });

                // Recovery: return to baseline within 4 evaluations
                for _ in 0..4 {
                    evaluations.push(Evaluation {
                        mu: 1.0,
                        sigma: 0.1,
                        err_inc: 0,
                        is_transient: false,
                    });
                }
            } else if i >= 500 {
                // Normal operation
                evaluations.push(Evaluation {
                    mu: 1.0,
                    sigma: 0.1,
                    err_inc: 0,
                    is_transient: false,
                });
            }
        }

        // Trim to exactly 1000
        evaluations.truncate(1000);

        Self { evaluations }
    }
}

/// Run static policy (baseline)
fn run_static_policy(workload: &Workload) -> (usize, usize) {
    let breaker = CircuitBreaker::new(State::Closed);

    // Static policy: Fixed thresholds (mu_max=2ms, sigma_max=0.3ms)
    let policy = Policy {
        mu_max: 2.0,    // 2× baseline (trips on transients)
        sigma_max: 0.3, // 3× baseline
        err_threshold: 5,
        open_duration_ms: 1000,
        ..Policy::default()
    };

    let mut last_change = 0u64;
    let mut false_positives = 0;
    let mut total_transients = 0;

    for (idx, eval) in workload.evaluations.iter().enumerate() {
        let timestamp = idx as u64 * 1_000_000; // 1ms between evaluations

        // Evaluate
        evaluate(
            &breaker,
            eval.mu,
            eval.sigma,
            eval.err_inc,
            timestamp,
            &mut last_change,
            &policy,
        );

        // Check for false positive: Circuit opened on transient spike
        let guard = breaker.guard();
        if eval.is_transient {
            total_transients += 1;
            if guard.state() == State::Open {
                false_positives += 1;
            }
        }
    }

    (false_positives, total_transients)
}

/// Run adaptive policy (P2 innovation)
#[cfg(feature = "circuit_breaker_auto_tune")]
fn run_adaptive_policy(workload: &Workload) -> (usize, usize) {
    let breaker = CircuitBreaker::new(State::Closed);

    // Adaptive policy: EMA-learned thresholds
    let mut history = HistoryBuffer::new();

    // Initialize calibrator with conservative targets
    let targets = CalibrationTargets {
        max_open_rate: 0.01,   // 1% max open rate
        min_closed_rate: 0.90, // 90% min closed rate
        p99_latency_ms: 5.0,   // 5ms P99 latency
        max_error_rate: 0.05,  // 5% max error rate
    };

    let mut calibrator = AutoCalibrator::new(CalibrationMode::Conservative, targets);

    let mut last_change = 0u64;
    let mut false_positives = 0;
    let mut total_transients = 0;

    // Initial static policy (will be adapted)
    let mut policy = Policy::default();

    for (idx, eval) in workload.evaluations.iter().enumerate() {
        let timestamp = idx as u64 * 1_000_000; // 1ms between evaluations

        // Evaluate with current policy
        evaluate(
            &breaker,
            eval.mu,
            eval.sigma,
            eval.err_inc,
            timestamp,
            &mut last_change,
            &policy,
        );

        // Record history
        let guard = breaker.guard();
        history.record(
            timestamp,
            guard.state(),
            eval.mu,
            eval.sigma,
            guard.error_count(),
        );

        // Adapt policy every 50 evaluations (EMA window)
        if idx % 50 == 0 && idx > 0 {
            if let Some(draft) = calibrator.calibrate(&history) {
                policy = draft.to_policy();
            }
        }

        // Check for false positive
        if eval.is_transient {
            total_transients += 1;
            if guard.state() == State::Open {
                false_positives += 1;
            }
        }
    }

    (false_positives, total_transients)
}

#[cfg(not(feature = "circuit_breaker_auto_tune"))]
fn run_adaptive_policy(_workload: &Workload) -> (usize, usize) {
    panic!("circuit_breaker_auto_tune feature not enabled");
}

fn main() {
    println!("═══════════════════════════════════════════════════════════");
    println!("  P2 Adaptive Circuit Breaker: False Positive Validation");
    println!("═══════════════════════════════════════════════════════════\n");

    // Create workload
    let workload = Workload::new();
    println!("Workload: {} evaluations", workload.evaluations.len());

    let transient_count = workload
        .evaluations
        .iter()
        .filter(|e| e.is_transient)
        .count();
    println!("Transient spikes (ground truth): {}\n", transient_count);

    // Run static policy
    println!("Running STATIC policy (baseline)...");
    let (static_fp, static_total) = run_static_policy(&workload);
    let static_fp_rate = (static_fp as f64 / static_total as f64) * 100.0;
    println!(
        "  False positives: {}/{} ({:.1}%)\n",
        static_fp, static_total, static_fp_rate
    );

    // Run adaptive policy
    #[cfg(feature = "circuit_breaker_auto_tune")]
    {
        println!("Running ADAPTIVE policy (P2 innovation)...");
        let (adaptive_fp, adaptive_total) = run_adaptive_policy(&workload);
        let adaptive_fp_rate = (adaptive_fp as f64 / adaptive_total as f64) * 100.0;
        println!(
            "  False positives: {}/{} ({:.1}%)\n",
            adaptive_fp, adaptive_total, adaptive_fp_rate
        );

        // Calculate reduction
        let reduction = ((static_fp_rate - adaptive_fp_rate) / static_fp_rate) * 100.0;

        println!("═══════════════════════════════════════════════════════════");
        println!("  RESULTS");
        println!("═══════════════════════════════════════════════════════════");
        println!("  Static FP rate:    {:.1}%", static_fp_rate);
        println!("  Adaptive FP rate:  {:.1}%", adaptive_fp_rate);
        println!(
            "  Reduction:         {:.1}% {}",
            reduction,
            if reduction >= 40.0 { "✅" } else { "⚠️" }
        );
        println!("═══════════════════════════════════════════════════════════\n");

        // Validation
        if reduction >= 40.0 && reduction <= 60.0 {
            println!("✅ SUCCESS: 40-60% false positive reduction validated!");
        } else if reduction > 0.0 {
            println!("⚠️  PARTIAL: {:.1}% reduction (target: 40-60%)", reduction);
        } else {
            println!("❌ FAILURE: No reduction achieved");
        }
    }

    #[cfg(not(feature = "circuit_breaker_auto_tune"))]
    {
        println!("❌ circuit_breaker_auto_tune feature not enabled - cannot run adaptive policy");
        println!("   Run with: cargo run --example validate_adaptive_fp_reduction --features 'circuit-breaker-auto-tune,nightly'");
    }
}
