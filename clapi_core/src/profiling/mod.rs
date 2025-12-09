//! Real-time performance profiling with latency histograms
//!
//! This module provides lockfree, cache-aligned latency tracking for production systems.
//!
//! # Features
//!
//! - **Tier 1 Atomic Capsules**: <10ns metric collection
//! - **Logarithmic Bucketing**: 50 buckets covering 1ns to 1s
//! - **O(1) Percentile Queries**: <50ns p50/p99/p999 calculation
//! - **100% Lockfree**: Zero mutex/RwLock, atomic bucket updates
//! - **Compile-Time Verification**: #[derive(ComputationalCapsule)]
//!
//! # Architecture
//!
//! ```text
//! LatencyHistogramCapsule (512B, 64B-aligned)
//! ├─ buckets: [AtomicU64; 50]  (Logarithmic: 2^0ns to 2^49ns)
//! ├─ total_count: AtomicU64     (Total samples)
//! ├─ sum_ns: AtomicU64          (For mean calculation)
//! ├─ min_ns: AtomicU64          (Minimum latency)
//! ├─ max_ns: AtomicU64          (Maximum latency)
//! └─ generation: AtomicU64      (TOCTOU prevention)
//! ```
//!
//! # Performance
//!
//! - **record()**: <10ns (atomic bucket increment)
//! - **percentile()**: <50ns (linear scan through 50 buckets)
//! - **mean()**: <20ns (two atomic loads + division)
//! - **stats()**: <100ns (full snapshot)
//!
//! # Example
//!
//! ```rust
//! use clapi_core::profiling::{LatencyHistogramCapsule, HistogramStats};
//! use std::time::Instant;
//!
//! let histogram = LatencyHistogramCapsule::new();
//!
//! // Record operation latency
//! let start = Instant::now();
//! perform_operation();
//! histogram.record(start.elapsed().as_nanos() as u64);
//!
//! // Query percentiles (<50ns)
//! let p99 = histogram.percentile(99.0);
//! let p999 = histogram.percentile(99.9);
//!
//! // Get full statistics (<100ns)
//! let stats = histogram.stats();
//! println!("Latency: min={}ns, p50={}ns, p99={}ns, max={}ns",
//!          stats.min, stats.p50, stats.p99, stats.max);
//! ```
//!
//! # UCE34 Framework Compliance
//!
//! - **Q10**: Tier 1 Atomic Capsule (lockfree coordination) + Tier 2 SIMD (percentile optimization)
//! - **Q11**: Rust Transform (AtomicU64 array, #[repr(C, align(512))], portable_simd)
//! - **Q12**: Nightly Enhancement (portable_simd for 2.5× percentile speedup)
//! - **Q33**: Verification (verify_capsule_properties! macro)
//!
//! # SIMD Optimization (Week 4 - Part 2)
//!
//! Enable `portable_simd` feature for 2.5× percentile speedup:
//! - **Without SIMD**: ~50ns percentile calculation (scalar)
//! - **With SIMD**: ~20ns percentile calculation (u64x8 parallel bucket scanning)
//!
//! Build with nightly: `cargo +nightly build --features portable_simd`
//!
//! # Testing (T28 Framework)
//!
//! - **Unit tests**: 10 tests (bucket assignment, percentile accuracy)
//! - **Property tests**: 3 tests (1000 random latencies, percentile bounds)
//! - **Integration tests**: 2 tests (end-to-end profiling)
//! - **Stress tests**: 1 test (1M concurrent samples)
//! - **SIMD tests**: 8+ tests (SIMD/scalar equivalence, batch percentiles)

pub mod capsule;
pub mod histogram;
pub mod histogram_simd;  // SIMD percentile optimization (Week 4)

pub use capsule::{HistogramStats, LatencyHistogramCapsule};
pub use histogram::LatencyProfiler;
