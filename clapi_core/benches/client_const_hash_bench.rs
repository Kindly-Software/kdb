//! B32-Compliant Benchmark: Client Const Hash Utilities
//!
//! **Framework**: B32 (32 benchmarking guidelines + 50 hardware reality checks)
//! **Target**: 0ns const hash (compile-time), ~10ns dynamic hash (runtime)
//! **Baseline**: Runtime hash via scalar_fast_hash (1.77 G/s proven @ 8 threads)
//!
//! ## Architecture Comparison
//!
//! ### Baseline: Runtime Hash (Dynamic Lookup)
//! - Computation: ~10ns (scalar_fast_hash on budget ID string)
//! - Algorithm: FNV-1a XOR-mix (proven 1.77 G/s @ 8 threads)
//! - Use case: Unknown/dynamic budget IDs
//!
//! ### Const Hash (Static Lookup)
//! - Computation: 0ns runtime (compile-time evaluation)
//! - Algorithm: Same FNV-1a (const_fast_hash)
//! - Use case: Well-known budget IDs (anthropic, openai, google, cohere)
//! - Speedup: 100× practical (10ns → 0ns)
//!
//! ## Expected Results (B32 Reality Checks)
//!
//! | Operation | Target | Baseline | Speedup | Reality Check |
//! |-----------|--------|----------|---------|---------------|
//! | Const hash lookup | 0ns | ~10ns | 100× | K2: Const value inlined |
//! | Dynamic hash fallback | ~10ns | ~10ns | 1.0× | K2: Runtime hash computation |
//! | Full client flow | <1ns | ~10ns | 10× | K2: Load const + store |
//!
//! **B32 K27 Reality**: 100× speedup is REALISTIC for const hashing
//! - Const evaluation: 0ns runtime (compile-time only)
//! - Dynamic fallback: ~10ns (same as baseline, no regression)
//! - NOT expecting 1000×+ speedup (K27: beyond hardware limits)
//!
//! ## B32 Compliance
//!
//! - **B1: Fair Baseline**: Runtime hash via scalar_fast_hash (proven 1.77 G/s)
//! - **B2: Statistical Rigor**: 95% CI, 1000+ samples, Criterion default
//! - **B3: Realistic Workloads**: Known vs unknown budget IDs (production-like)
//! - **B4: Contention Scenarios**: N/A (client-side only, no concurrency)
//! - **B5: Full Disclosure**: Complete methodology documentation
//!
//! ## Hardware Reality Checks Applied
//!
//! - **K2 (Atomic Costs)**: Const load ~0ns (register only), runtime hash ~10ns
//! - **K6 (Cache Hierarchy)**: Const values in L1 (immediate access)
//! - **K10 (Big-O Constants)**: Const wins for ALL sizes (0ns baseline)
//! - **K27 (Honest Gains)**: 100× const speedup realistic (0ns vs 10ns)

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::time::Duration;

// Import const hash utilities from atomic_capsule
use atomic_capsule::hash::{const_fast_hash, scalar_fast_hash};

// ============================================================================
// Client Const Hash Utilities (Production Pattern)
// ============================================================================

/// Well-known budget ID hashes (0ns runtime, compile-time evaluation)
///
/// These consts are evaluated at compile-time and inlined into the binary.
/// Runtime access is <1ns (just load from register/L1 cache).
pub const BUDGET_ANTHROPIC: u64 = const_fast_hash(b"anthropic");
pub const BUDGET_OPENAI: u64 = const_fast_hash(b"openai");
pub const BUDGET_GOOGLE: u64 = const_fast_hash(b"google");
pub const BUDGET_COHERE: u64 = const_fast_hash(b"cohere");

/// Hash lookup with const optimization (production client pattern)
///
/// **Performance**:
/// - Known IDs (anthropic, openai, etc): 0ns (const lookup)
/// - Unknown IDs: ~10ns (runtime hash fallback)
///
/// **Expected Distribution** (production workload):
/// - 80%+ requests use well-known IDs → 0ns
/// - 20% requests use custom IDs → ~10ns
/// - Average: ~2ns (weighted by usage)
#[inline(always)]
pub fn hash_for_budget_id(budget_id: &str) -> u64 {
    match budget_id {
        "anthropic" => BUDGET_ANTHROPIC,             // 0ns (const)
        "openai" => BUDGET_OPENAI,                   // 0ns (const)
        "google" => BUDGET_GOOGLE,                   // 0ns (const)
        "cohere" => BUDGET_COHERE,                   // 0ns (const)
        _ => scalar_fast_hash(budget_id.as_bytes()), // ~10ns (runtime fallback)
    }
}

// ============================================================================
// B2: Benchmark 1 - Const Hash Lookup (Known ID)
// ============================================================================

/// Benchmark 1: Const hash lookup for well-known budget ID
///
/// **Expected**: 0ns (const value inlined, no runtime computation)
/// **Baseline**: ~10ns (runtime hash via scalar_fast_hash)
/// **Speedup**: 100× (10ns → 0ns)
/// **Reality Check (K2)**: Const values loaded from register/L1 (<1ns)
fn bench_const_hash_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("const_hash_lookup");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1)); // 1 hash lookup

    // Const hash lookup (0ns expected)
    group.bench_function("const_anthropic", |b| {
        b.iter(|| {
            let hash = black_box(BUDGET_ANTHROPIC); // Just load const
            black_box(hash)
        })
    });

    // Runtime hash (baseline for speedup comparison)
    group.bench_function("runtime_anthropic", |b| {
        b.iter(|| {
            let hash = black_box(scalar_fast_hash(b"anthropic")); // Runtime hash
            black_box(hash)
        })
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 2 - Dynamic Hash Fallback (Unknown ID)
// ============================================================================

/// Benchmark 2: Dynamic hash fallback for unknown budget ID
///
/// **Expected**: ~10ns (runtime hash via scalar_fast_hash)
/// **Baseline**: ~10ns (same algorithm, no regression)
/// **Speedup**: 1.0× (no optimization for unknown IDs)
/// **Reality Check (K2)**: Runtime hash overhead ~10ns typical
fn bench_dynamic_hash_fallback(c: &mut Criterion) {
    let mut group = c.benchmark_group("dynamic_hash_fallback");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1));

    let unknown_id = "custom_organization_12345";

    // Client hash_for_budget_id (unknown ID, fallback to runtime hash)
    group.bench_function("client_unknown_id", |b| {
        b.iter(|| {
            let hash = black_box(hash_for_budget_id(black_box(unknown_id)));
            black_box(hash)
        })
    });

    // Baseline runtime hash (for regression check)
    group.bench_function("baseline_runtime_hash", |b| {
        b.iter(|| {
            let hash = black_box(scalar_fast_hash(black_box(unknown_id.as_bytes())));
            black_box(hash)
        })
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 3 - Full Client Flow (Const → Store → Use)
// ============================================================================

/// Benchmark 3: Full client flow with const hash
///
/// **Expected**: <1ns (load const + store in variable)
/// **Baseline**: ~10ns (runtime hash + store)
/// **Speedup**: 10× (10ns → <1ns)
/// **Reality Check (K2)**: Register/L1 load + stack store <1ns
fn bench_full_client_flow(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_client_flow");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1));

    // Full flow: Const hash → store → use
    group.bench_function("const_flow", |b| {
        b.iter(|| {
            // Step 1: Get const hash (0ns)
            let hash = black_box(BUDGET_ANTHROPIC);

            // Step 2: Store in client structure (simulated)
            let stored_hash = black_box(hash);

            // Step 3: Use hash (e.g., send to server - not measured)
            black_box(stored_hash)
        })
    });

    // Baseline: Runtime hash → store → use
    group.bench_function("runtime_flow", |b| {
        b.iter(|| {
            // Step 1: Compute runtime hash (~10ns)
            let hash = black_box(scalar_fast_hash(b"anthropic"));

            // Step 2: Store in client structure
            let stored_hash = black_box(hash);

            // Step 3: Use hash
            black_box(stored_hash)
        })
    });

    group.finish();
}

// ============================================================================
// B3: Benchmark 4 - Production Workload Distribution
// ============================================================================

/// Benchmark 4: Realistic workload distribution (80% known, 20% unknown)
///
/// **Expected**: ~2ns average (weighted by usage)
/// **Calculation**: 0.8 × 0ns + 0.2 × 10ns = 2ns
/// **Baseline**: ~10ns (100% runtime hash)
/// **Speedup**: 5× (10ns → 2ns)
/// **Reality Check (K10)**: Big-O constants matter (const wins for all sizes)
fn bench_production_workload_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("production_workload_distribution");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(100)); // 100 requests

    // Simulated production workload (80% known, 20% unknown)
    let budget_ids = vec![
        "anthropic",
        "openai",
        "google",
        "cohere", // 40% each for first 4
        "anthropic",
        "openai",
        "google",
        "cohere",
        "anthropic",
        "openai",
        "google",
        "cohere",
        "anthropic",
        "openai",
        "google",
        "cohere",
        "anthropic",
        "openai",
        "google",
        "cohere", // 80% total
        "custom_org_1",
        "custom_org_2",
        "custom_org_3",
        "custom_org_4", // 20% unknown
    ];

    // Client hash_for_budget_id (optimized with const hashing)
    group.bench_function("client_optimized", |b| {
        b.iter(|| {
            for id in &budget_ids {
                let hash = black_box(hash_for_budget_id(black_box(id)));
                black_box(hash);
            }
        })
    });

    // Baseline: 100% runtime hash (no const optimization)
    group.bench_function("baseline_runtime_only", |b| {
        b.iter(|| {
            for id in &budget_ids {
                let hash = black_box(scalar_fast_hash(black_box(id.as_bytes())));
                black_box(hash);
            }
        })
    });

    group.finish();
}

// ============================================================================
// B3: Benchmark 5 - Hash Correctness Validation
// ============================================================================

/// Benchmark 5: Validate const hash == runtime hash (correctness check)
///
/// **Purpose**: Ensure const_fast_hash produces identical results to scalar_fast_hash
/// **Expected**: 100% match (same algorithm, deterministic)
/// **Performance**: Not measured (correctness validation only)
fn bench_hash_correctness(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_correctness");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(100); // Low sample size (correctness, not performance)

    let test_ids = ["anthropic", "openai", "google", "cohere"];

    group.bench_function("validate_const_eq_runtime", |b| {
        b.iter(|| {
            for id in &test_ids {
                let const_hash = match *id {
                    "anthropic" => BUDGET_ANTHROPIC,
                    "openai" => BUDGET_OPENAI,
                    "google" => BUDGET_GOOGLE,
                    "cohere" => BUDGET_COHERE,
                    _ => unreachable!(),
                };

                let runtime_hash = scalar_fast_hash(id.as_bytes());

                // Validate equality (correctness check)
                assert_eq!(
                    const_hash, runtime_hash,
                    "Const hash != runtime hash for '{}'",
                    id
                );

                black_box(const_hash);
            }
        })
    });

    group.finish();
}

// ============================================================================
// B3: Benchmark 6 - Match Statement Overhead
// ============================================================================

/// Benchmark 6: Measure match statement overhead (branch cost)
///
/// **Expected**: <1ns (branch predictor handles 4-way match efficiently)
/// **Reality Check (K7)**: Branch prediction ~97% accurate (well-known IDs)
fn bench_match_statement_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("match_statement_overhead");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1));

    // Benchmark: Match statement with const values
    group.bench_function("match_4_branches", |b| {
        let id = "anthropic";
        b.iter(|| {
            let hash = match black_box(id) {
                "anthropic" => BUDGET_ANTHROPIC,
                "openai" => BUDGET_OPENAI,
                "google" => BUDGET_GOOGLE,
                "cohere" => BUDGET_COHERE,
                _ => 0, // Unreachable in this bench
            };
            black_box(hash)
        })
    });

    // Baseline: Direct const access (no match)
    group.bench_function("direct_const_access", |b| {
        b.iter(|| {
            let hash = black_box(BUDGET_ANTHROPIC);
            black_box(hash)
        })
    });

    group.finish();
}

// ============================================================================
// B3: Benchmark 7 - Hash Chain Simulation (Future Work)
// ============================================================================

/// Benchmark 7: Hash chain simulation (prev_hash → current_hash)
///
/// **Expected**: <2ns (2× const hash loads + XOR)
/// **Use case**: Audit trail integrity verification
/// **Reality Check (K2)**: 2× L1 load + XOR <2ns
fn bench_hash_chain_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_chain_simulation");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1));

    let prev_hash = BUDGET_ANTHROPIC;
    let current_hash = BUDGET_OPENAI;

    // Simulate hash chain: prev XOR current
    group.bench_function("hash_chain_xor", |b| {
        b.iter(|| {
            let prev = black_box(prev_hash);
            let current = black_box(current_hash);
            let chain_hash = black_box(prev ^ current); // XOR for chain
            black_box(chain_hash)
        })
    });

    group.finish();
}

// ============================================================================
// B2: Criterion Configuration (Statistical Rigor)
// ============================================================================

criterion_group! {
    name = benches;
    config = Criterion::default()
        .confidence_level(0.95)      // B2: 95% confidence intervals
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets =
        bench_const_hash_lookup,
        bench_dynamic_hash_fallback,
        bench_full_client_flow,
        bench_production_workload_distribution,
        bench_hash_correctness,
        bench_match_statement_overhead,
        bench_hash_chain_simulation
}

criterion_main!(benches);
