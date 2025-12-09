# Test Support Primitive

Shared testing utilities and benchmarking framework for all atomic primitives, implementing B32 benchmarking standards with statistical rigor and lockfree verification.

## Features

### B32 Benchmarking Framework
- Statistical validation with 95% confidence intervals
- Fair baseline comparisons (no strawmen)
- Hardware-aware performance measurement
- B32 compliance checking

### Statistical Analysis
- Comprehensive performance metrics
- Confidence interval calculation
- Outlier detection and analysis
- Measurement quality validation

### Lockfree Verification
- Atomic operation correctness testing
- Memory ordering validation
- Contention analysis across thread counts
- ABA problem resistance testing

### Test Data Generation
- Deterministic random number generation
- Market data simulation
- Atomic operation sequences
- Various distribution patterns

### Test Assertions
- Type-safe assertion framework
- Performance validation
- Thread safety verification
- Property-based testing support

## Quick Start

```rust
use test_support::*;
use std::sync::atomic::{AtomicU64, Ordering};

// B32 benchmarking
let validator = BenchmarkValidator::new()
    .with_baseline("mutex", 100.0);

let result = validator.measure_operation(|| {
    atomic.fetch_add(1, Ordering::Relaxed);
})?;

assert!(result.meets_b32_standards());
```

## Example

Run the comprehensive example:

```bash
cargo run --example basic_usage -p test_support
```

## Module Structure

- `benchmark` - B32-compliant benchmarking framework
- `statistical` - Statistical analysis and validation
- `lockfree` - Lockfree operation verification
- `generators` - Test data generation utilities
- `validation` - Assertion and property testing framework

## B32 Compliance

This primitive enforces B32 benchmarking standards:

- Minimum 1000 iterations for statistical validity
- 95% confidence intervals required
- Fair baseline comparisons
- Hardware reality checks
- Sustained performance measurement (60+ seconds)
- Percentile reporting (P50, P95, P99)

## Hardware Constants

Based on Intel Ultra 7 155H measurements:

- AtomicU64 CAS: ~15ns
- AtomicU128 CAS: ~20ns
- L1 Cache: 1ns latency
- Efficient threads: ≤12
- Typical improvements: 10-50%
- Exceptional improvements: 50-200%
- Suspicious claims: >200%

## Integration

Add to your test dependencies:

```toml
[dev-dependencies]
test_support = { path = "../test_support" }
```

## License

PROPRIETARY - Part of Primitives workspace