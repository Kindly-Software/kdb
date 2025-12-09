//! XML output for machine-readable benchmark results
//!
//! Schema-compliant output for automation and CI/CD integration

use crate::classification::Classification;
use crate::stats::Statistics;
use crate::validation::HardwareInfo;

/// Generate XML output (simplified for Phase 1)
pub fn generate_xml(
    name: &str,
    tier: &str,
    baseline_kind: &str,
    optimized: &Statistics,
    baseline: &Statistics,
    classification: &Classification,
    hardware: &HardwareInfo,
) -> String {
    let speedup = optimized.speedup(baseline);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<benchmark-results xmlns="http://kindly.software/schemas/benchmark-results"
                   version="1.0.0"
                   timestamp="{}">

  <metadata>
    <run-id>{}</run-id>
    <framework-version>kindly_bench 1.0.0</framework-version>
    <b32-compliant>true</b32-compliant>
  </metadata>

  <hardware>
    <cpu>
      <model>{}</model>
      <microarchitecture>{}</microarchitecture>
      <cores-total>{}</cores-total>
      <cache-line-bytes>{}</cache-line-bytes>
    </cpu>
    <memory>
      <size-gb>{}</size-gb>
    </memory>
  </hardware>

  <benchmarks>
    <benchmark id="{}">
      <name>{}</name>
      <tier>{}</tier>
      <baseline-kind>{}</baseline-kind>
      <optimized>
        <samples>{}</samples>
        <mean-ns>{:.2}</mean-ns>
        <median-ns>{:.2}</median-ns>
        <p95-ns>{:.2}</p95-ns>
        <p99-ns>{:.2}</p99-ns>
        <stddev-ns>{:.2}</stddev-ns>
        <min-ns>{:.2}</min-ns>
        <max-ns>{:.2}</max-ns>
        <outliers>{}</outliers>
        <confidence-interval-95>
          <lower-bound-ns>{:.2}</lower-bound-ns>
          <upper-bound-ns>{:.2}</upper-bound-ns>
        </confidence-interval-95>
      </optimized>
      <baseline>
        <samples>{}</samples>
        <mean-ns>{:.2}</mean-ns>
        <median-ns>{:.2}</median-ns>
        <p95-ns>{:.2}</p95-ns>
        <p99-ns>{:.2}</p99-ns>
        <stddev-ns>{:.2}</stddev-ns>
        <min-ns>{:.2}</min-ns>
        <max-ns>{:.2}</max-ns>
        <outliers>{}</outliers>
        <confidence-interval-95>
          <lower-bound-ns>{:.2}</lower-bound-ns>
          <upper-bound-ns>{:.2}</upper-bound-ns>
        </confidence-interval-95>
      </baseline>
      <speedup>
        <mean-speedup>{:.2}</mean-speedup>
        <median-speedup>{:.2}</median-speedup>
        <p95-speedup>{:.2}</p95-speedup>
        <confidence-interval-95>
          <lower-bound>{:.2}</lower-bound>
          <upper-bound>{:.2}</upper-bound>
        </confidence-interval-95>
      </speedup>
      <classification>
        <tier>{:?}</tier>
        <confidence>{:?}</confidence>
      </classification>
      <recommendation>
        <action>{:?}</action>
        <reasoning>{}</reasoning>
        <next-steps>{}</next-steps>
      </recommendation>
    </benchmark>
  </benchmarks>

</benchmark-results>
"#,
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
        uuid::Uuid::new_v4(),
        hardware.cpu_model,
        hardware.microarchitecture,
        hardware.cores_total,
        hardware.cache_line_bytes,
        hardware.memory_size_gb.unwrap_or(0),
        name.replace(" ", "-"),
        name,
        tier,
        baseline_kind,
        optimized.samples,
        optimized.mean_ns,
        optimized.median_ns,
        optimized.p95_ns,
        optimized.p99_ns,
        optimized.stddev_ns,
        optimized.min_ns,
        optimized.max_ns,
        optimized.outliers,
        optimized.confidence_interval_95.lower_bound_ns,
        optimized.confidence_interval_95.upper_bound_ns,
        baseline.samples,
        baseline.mean_ns,
        baseline.median_ns,
        baseline.p95_ns,
        baseline.p99_ns,
        baseline.stddev_ns,
        baseline.min_ns,
        baseline.max_ns,
        baseline.outliers,
        baseline.confidence_interval_95.lower_bound_ns,
        baseline.confidence_interval_95.upper_bound_ns,
        speedup.mean_speedup,
        speedup.median_speedup,
        speedup.p95_speedup,
        speedup.confidence_interval_95.lower_bound,
        speedup.confidence_interval_95.upper_bound,
        classification.tier,
        classification.confidence,
        classification.recommendation_action(),
        classification.reasoning(),
        classification.next_steps()
    )
}

/// Save XML to file
pub fn save_xml(xml: &str, filename: &str) -> std::io::Result<()> {
    std::fs::write(filename, xml)
}
