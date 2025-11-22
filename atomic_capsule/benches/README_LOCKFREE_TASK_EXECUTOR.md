# B32 Benchmark Suite: LockfreeTaskExecutor (ThreadPool)

## Overview

Comprehensive B32-compliant benchmark suite for `atomic_capsule::parallel::ThreadPool`, comparing against Rayon as a fair baseline.

## Files

- **`lockfree_task_executor_bench.rs`**: New comprehensive benchmark suite (7 groups, 20+ benchmarks)
- **`parallel_benchmarks.rs`**: Existing benchmark suite (already comprehensive)

## B32 Framework Compliance

### B1: Fair Baseline Selection
- ✅ **Rayon 1.8+**: Industry-standard optimized parallel library (NOT strawman)
- ✅ **Same hardware**: All benchmarks run on same CPU
- ✅ **Same compiler**: Rust nightly with same RUSTFLAGS

### B2: Measurement Methodology
- ✅ **Criterion.rs**: Automatic 1000+ iterations with 95% CI
- ✅ **Warmup period**: 3 seconds to stabilize caches and JIT
- ✅ **Multiple runs**: Criterion automatically runs 3+ independent measurements
- ✅ **Variance reporting**: Standard deviation and percentiles included

### B3: Realistic Workloads
- ✅ **Empty tasks**: Coordination overhead only (baseline)
- ✅ **Simulated CNLS**: 100μs compute per task (realistic quantum simulation)
- ✅ **Scalability test**: Fixed workload across 1-32 threads

### B4: Contention Scenarios
- ✅ **Uncontended** (1 thread): Baseline performance
- ✅ **Light contention** (4 threads): Typical case
- ✅ **Moderate contention** (16 threads): Hardware thread count
- ✅ **Heavy contention** (32 threads): Stress test

### B5: Reporting Standards
All benchmarks output:
- P50, P95, P99 percentiles
- Standard deviation
- Sample size (1000+ iterations)
- Hardware specifications (documented)
- Compiler version and flags
- Thermal conditions (sustained load)

## Performance Claims to Validate

### Claim 1: Cold Start (10-100× faster)
**Test**: `bench_executor_creation`
- **Claimed**: 100-500ns (vs Rayon 1-10μs)
- **B32 Reality Check**: 2-5× more realistic (pool creation is expensive)
- **Expected Result**: ~100μs per worker (thread spawn overhead)

### Claim 2: Hot Iteration (within 10%)
**Test**: `bench_comparison_to_rayon`
- **Claimed**: Similar to Rayon
- **B32 Reality Check**: Within 10-20% is realistic
- **Expected Result**: Comparable throughput

### Claim 3: Batch Tasks (10× faster)
**Test**: `bench_task_claiming_empty_10k_16_workers`
- **Claimed**: 50μs (vs Rayon 500μs)
- **B32 Reality Check**: 2-5× more realistic
- **Expected Result**: 100-200μs for 10K empty tasks

### Claim 4: P99.9 Latency (50-250× better)
**Test**: `bench_global_pool_scope_1000_tasks` (measure tail latency)
- **Claimed**: <2μs (vs Rayon 100-500μs)
- **B32 Reality Check**: 5-10× improvement realistic for tail latency
- **Expected Result**: P99.9 <20μs

## Benchmark Groups

### Group 1: Executor Creation
```bash
cargo bench --bench lockfree_task_executor_bench -- executor_creation
```
**Measures**: Thread pool initialization overhead
**Expected**: <2ms for 16 workers (50-100μs per thread spawn)

### Group 2: Coordination Overhead (Empty Tasks)
```bash
cargo bench --bench lockfree_task_executor_bench -- task_claiming_empty
```
**Measures**: Pure coordination overhead (no work)
**Expected**: <10μs per task for 10K tasks = <100ms total

### Group 3: Real Workload (Simulated CNLS)
```bash
cargo bench --bench lockfree_task_executor_bench -- simulated_cnls_workload
```
**Measures**: 100μs compute per task × 1000 tasks
**Expected**: ~62ms (1000 / 16 workers = 62.5 tasks/worker × 100μs)

### Group 4: Scalability (Thread Count)
```bash
cargo bench --bench lockfree_task_executor_bench -- scalability
```
**Measures**: Scaling from 1 to 32 threads (fixed workload)
**Expected**: Linear scaling up to hardware thread count (16), diminishing beyond

### Group 5: Contention Scenarios
```bash
cargo bench --bench lockfree_task_executor_bench -- contention_scenarios
```
**Measures**: Performance under different contention levels
**Expected**:
- 1 thread: 100ms (baseline)
- 4 threads: 25ms (4× speedup)
- 16 threads: 6.25ms (16× speedup)

### Group 6: Comparison to Rayon (Fair Baseline)
```bash
cargo bench --bench lockfree_task_executor_bench -- comparison_rayon
```
**Measures**: Direct head-to-head against Rayon
**Expected**: Within 10-20% (fair fight)

### Group 7: Global Pool API
```bash
cargo bench --bench lockfree_task_executor_bench -- global_pool_api
```
**Measures**: `get_global_pool()` and scoped execution
**Expected**: Minimal overhead vs direct pool creation

## Hardware Reality Checks (K1-K50)

### K18: Scheduling Overhead
- **Thread creation**: 50μs typical
- **Task spawn (tokio)**: 200ns
- **Our claim**: 5-10ns
- **B32 Verdict**: 10ns realistic for push, thread creation still 50μs

### K20: Throughput Scaling
- **6 P-cores**: 6.5× actual (not 6×)
- **8 P-cores**: Memory bandwidth saturated
- **16 threads**: 12-14× with E-cores
- **B32 Verdict**: Expect 12-14× at 16 workers

### K27: HONEST GAINS
- **Typical**: 10-50% improvement
- **Exceptional**: 2× speedup
- **Suspicious**: 10×+ without algorithm change
- **B32 Verdict**: Cold start 10× claim requires extensive validation

## Running Benchmarks

### Prerequisites
```bash
# Ensure atomic_capsule compiles
cd /home/samuel/Primitives/atomic_capsule
cargo build --release --features native,std

# Install Criterion report viewer (optional)
cargo install cargo-criterion
```

### Run Full Suite
```bash
# All benchmarks (~10-15 minutes)
cargo bench --bench lockfree_task_executor_bench --features native,std

# View HTML reports
open target/criterion/report/index.html
```

### Run Specific Group
```bash
# Only scalability tests
cargo bench --bench lockfree_task_executor_bench --features native,std -- scalability

# Only Rayon comparison
cargo bench --bench lockfree_task_executor_bench --features native,std -- comparison_rayon
```

### Save Baseline for Comparison
```bash
# Save current results as baseline
cargo bench --bench lockfree_task_executor_bench --features native,std -- --save-baseline main

# Compare against baseline after changes
cargo bench --bench lockfree_task_executor_bench --features native,std -- --baseline main
```

## Expected Results (B32 Honest Estimates)

### Cold Start
- **Claimed**: 10-100× faster
- **Realistic**: 2-5× (pool creation expensive)
- **Measurement**: ~100-500μs per pool

### Hot Iteration
- **Claimed**: Similar to Rayon
- **Realistic**: Within 10-20%
- **Measurement**: 10M tasks/sec on 8-core

### Batch Tasks
- **Claimed**: 10× faster
- **Realistic**: 2-5× (coordination overhead)
- **Measurement**: 100-200μs for 10K tasks

### Tail Latency (P99.9)
- **Claimed**: 50-250× better
- **Realistic**: 5-10× improvement
- **Measurement**: P99.9 <20μs vs Rayon ~100μs

## Compilation Status

**Current Status**: ❌ atomic_capsule has compilation errors (2025-11-02)

**Blockers**:
1. Missing `atomic_capsule_derive` crate resolution
2. `#[capsule]` attribute not found
3. Requires `std` feature for benchmarks

**Fix**:
```bash
# Once fixed, compile with:
cargo build --release --features native,std

# Then run benchmarks:
cargo bench --bench lockfree_task_executor_bench --features native,std
```

## ASSUM Safety Validation

All benchmarks validate ASSUM safety:
- ✅ **PANIC_SAFETY**: No panic in hot paths (queue full returns Err)
- ✅ **MEMORY_ORDERING**: All atomic operations use proper ordering
- ✅ **LOCKFREE**: 100% lockfree (no mutex/RwLock in benchmarks)
- ✅ **BLACK_BOX**: All results passed through `black_box()` to prevent optimizer erasure

## Framework References

- **B32 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/B32_BENCHMARK_FRAMEWORK.md`
- **ASSUM Safety**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/ASSUM_SAFETY.md`
- **UCE34 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/UCE34_FRAMEWORK.md`

## Benchmark Output Format

Criterion.rs generates:
```
executor_creation/executor_creation_16_workers
                        time:   [143.25 µs 145.87 µs 148.73 µs]
Found 2 outliers among 100 measurements (2.00%)
  1 (1.00%) high mild
  1 (1.00%) high severe

task_claiming_empty/task_claiming_empty_10k_16_workers
                        time:   [85.234 ms 87.156 ms 89.234 ms]
                        thrpt:  [112.01 Kelem/s 114.74 Kelem/s 117.31 Kelem/s]
```

Interpret:
- **time**: P50 with 95% CI (±3% typical)
- **thrpt**: Throughput (elements/sec)
- **outliers**: Statistical outliers (should be <5%)

## Integration with planck-universe

Once benchmarks pass, integrate results into:
- `planck-universe/CLAUDE.md` (update Phase 4.2 performance section)
- `planck-universe/docs/PHASE4_2_CNLS_PERFORMANCE_REPORT.md` (new file)

## Success Criteria

**PASS**: All benchmarks complete with:
- 95% CI < ±10%
- <5% outliers
- P99.9 < 100μs for coordination-only tasks
- Within 2× of Rayon for hot iteration

**EXCEPTIONAL**: If P99.9 < 10μs AND throughput within 20% of Rayon

**FAIL**: If >10% performance regression vs Rayon OR >50% variance

## Next Steps

1. ✅ **Benchmark Suite**: Created `lockfree_task_executor_bench.rs` (7 groups, 20+ benchmarks)
2. ⏳ **Fix Compilation**: Resolve atomic_capsule compilation errors
3. ⏳ **Run Benchmarks**: Execute full B32 suite
4. ⏳ **Analyze Results**: Compare against performance claims
5. ⏳ **Update Documentation**: Document actual vs claimed performance
6. ⏳ **Integrate with CNLS**: Use validated executor for Phase 4.2 hypothesis test

## Author Notes

**Created**: 2025-11-02
**Framework**: B32 Benchmark Framework (fair baselines, statistical rigor, honest reporting)
**Status**: Benchmark code complete, awaiting atomic_capsule compilation fix
**Next**: Fix atomic_capsule, run benchmarks, validate claims
