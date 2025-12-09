//! Terminal output for human-readable benchmark results

use crate::classification::{Classification, RecommendationAction};
use crate::stats::Statistics;
use crate::validation::HardwareInfo;

/// Print benchmark results to terminal
pub fn print_results(
    name: &str,
    tier: &str,
    baseline_kind: &str,
    optimized: &Statistics,
    baseline: &Statistics,
    classification: &Classification,
    hardware: &HardwareInfo,
) {
    println!("\n{}", "=".repeat(80));
    println!("BENCHMARK RESULTS: {}", name);
    println!("{}", "=".repeat(80));

    // Hardware summary
    println!("\nHARDWARE:");
    println!("  CPU: {}", hardware.cpu_model);
    println!("  Cores: {}", hardware.cores_total);
    println!("  Memory: {} GB", hardware.memory_size_gb.unwrap_or(0));
    if let Some(governor) = &hardware.frequency_scaling_governor {
        println!("  Governor: {}", governor);
        if governor != "performance" {
            println!("  ⚠ WARNING: CPU governor is not 'performance', results may vary");
        }
    }

    // Benchmark configuration
    println!("\nCONFIGURATION:");
    println!("  Tier: {}", tier);
    println!("  Baseline: {}", baseline_kind);
    println!("  Samples: {}", optimized.samples);

    // Performance results
    println!("\nPERFORMANCE:");
    println!("  Optimized:");
    println!("    Mean:   {:>10.2} ns", optimized.mean_ns);
    println!("    Median: {:>10.2} ns", optimized.median_ns);
    println!("    P95:    {:>10.2} ns", optimized.p95_ns);
    println!("    P99:    {:>10.2} ns", optimized.p99_ns);
    println!("    StdDev: {:>10.2} ns ({:.1}%)", optimized.stddev_ns, (optimized.stddev_ns / optimized.mean_ns) * 100.0);

    println!("\n  Baseline:");
    println!("    Mean:   {:>10.2} ns", baseline.mean_ns);
    println!("    Median: {:>10.2} ns", baseline.median_ns);
    println!("    P95:    {:>10.2} ns", baseline.p95_ns);
    println!("    P99:    {:>10.2} ns", baseline.p99_ns);
    println!("    StdDev: {:>10.2} ns ({:.1}%)", baseline.stddev_ns, (baseline.stddev_ns / baseline.mean_ns) * 100.0);

    // Speedup
    let speedup = optimized.speedup(baseline);
    println!("\nSPEEDUP:");
    println!("  Mean:   {:.2}×", speedup.mean_speedup);
    println!("  Median: {:.2}×", speedup.median_speedup);
    println!("  P95:    {:.2}×", speedup.p95_speedup);
    println!("  95% CI: [{:.2}×, {:.2}×]",
        speedup.confidence_interval_95.lower_bound,
        speedup.confidence_interval_95.upper_bound
    );

    // Classification
    println!("\nCLASSIFICATION:");
    println!("  Tier: {:?}", classification.tier);
    println!("  Confidence: {:?}", classification.confidence);
    if !classification.flags.is_empty() {
        println!("  Flags: {}", classification.flags.join(", "));
    }

    // Recommendation
    println!("\nRECOMMENDATION:");
    let action = classification.recommendation_action();
    let action_str = match action {
        RecommendationAction::Ship => "✓ SHIP",
        RecommendationAction::Optimize => "⚠ OPTIMIZE",
        RecommendationAction::Investigate => "? INVESTIGATE",
        RecommendationAction::Validate => "! VALIDATE",
        RecommendationAction::Iterate => "↻ ITERATE",
    };
    println!("  Action: {}", action_str);
    println!("  Reasoning: {}", classification.reasoning());
    println!("  Next Steps: {}", classification.next_steps());

    println!("\n{}", "=".repeat(80));
}
