# B32 Benchmark Plan for ParallelDedupMetacapsule

**Phase**: Week 3-5 (Parallel Track, scaffolding now, execution after Agent 13 worker_loop() complete)

**Target Speedup**: 3.3× @ 16 threads (200K docs/sec vs 60K sequential baseline)

**Framework**: UCE34 Q1-Q34 + B32 (Fair Benchmarking) + ASSUM (Safety)

---

## 1. Executive Summary

ParallelDedupMetacapsule requires comprehensive B32 benchmarking to validate design claims:

| Metric | Target | Evidence |
|--------|--------|----------|
| **Single-Threaded (1 worker)** | ~60K docs/sec | Parity with DedupPipeline baseline |
| **16-Threaded Speedup** | 3.3× | 200K docs/sec = 60K × 3.3 |
| **Coordination Overhead** | <1% | Latency negligible vs work time |
| **Parallelizable Fraction (P)** | 0.90 | 90% of code can run in parallel |
| **Load Imbalance** | <5% | Work stealing distributes batches evenly |
| **Steal Success Rate** | >50% | Most steal attempts succeed |
| **Steal Latency** | <1μs | Lockfree queue is fast |

---

## 2. Benchmark Architecture

### 2.1 Four Benchmark Suites

#### Suite 1: Throughput (5 benchmarks)
- **dedup_pipeline_baseline**: Sequential baseline (60K docs/sec)
- **parallel_metacapsule_1_thread**: Single worker (target: parity)
- **parallel_metacapsule_16_threads**: 16 workers (target: 3.3×)
- **parallel_metacapsule_scaling**: 1,2,4,8,16 workers (validate Amdahl)
- **parallel_dedup_pipeline_broken**: Broken version (regression baseline)

#### Suite 2: Coordination (4 benchmarks)
- **atomic_snapshot_latency**: 6-8ns target
- **phase_mask_update_latency**: <10ns target
- **batch_claim_complete_latency**: <10ns combined
- **coordination_overhead_percentage**: <1% of total time

#### Suite 3: Amdahl Validation (3 benchmarks)
- **parallelizable_fraction**: Measure P empirically (target: 0.90)
- **speedup_validation**: Measure S @ each worker count
- **efficiency_calculation**: Calculate E = S/N (target: 40% @ N=16)

#### Suite 4: Work-Stealing (3 benchmarks)
- **load_imbalance**: <5% target
- **steal_success_rate**: >50% target
- **steal_latency**: <1μs target

### 2.2 File Structure

```
benches/
├── parallel_dedup_metacapsule_throughput.rs       (520 lines, 5 benchmarks)
├── parallel_dedup_metacapsule_coordination.rs     (410 lines, 4 benchmarks)
├── parallel_dedup_metacapsule_amdahl.rs          (380 lines, 3 benchmarks)
├── parallel_dedup_metacapsule_work_stealing.rs   (310 lines, 3 benchmarks)
└── docs/
    └── B32_BENCHMARK_PLAN.md                     (this file)

Total: 15 benchmarks, 1,620 lines scaffold, ready for population
```

---

## 3. Fair Baselines (B32 Requirements K1-K10)

### 3.1 Baseline Selection Rationale

#### Baseline 1: DedupPipeline (Sequential)
- **Description**: Existing single-threaded deduplication pipeline
- **Hardware**: AMD Ryzen 9 6900HX (same as target)
- **Measured Throughput**: 60,000 docs/sec (validated in CLAUDE.md)
- **Per-Document Latency**: 16.7μs (1/60K)
- **Justification**: Only fair way to measure parallel speedup
- **NOT strawman**: Full Rust implementation, reasonable algorithm

#### Baseline 2: ParallelDedupMetacapsule @ 1 worker
- **Description**: New metacapsule with single worker
- **Expected Result**: ~60K docs/sec (parity with DedupPipeline)
- **Purpose**: Validate metacapsule overhead is negligible
- **Success Criteria**: Within ±10% of DedupPipeline (54-66K)

#### Baseline 3: ParallelDedupPipeline (Broken - Regression Baseline)
- **Description**: Previous parallel implementation (deprecated)
- **Measured Throughput**: 6,000 docs/sec @ 16 threads (12.8× SLOWER)
- **Why include**: Shows why redesign was necessary
- **Purpose**: Validate improvement over old design
- **Status**: DEPRECATED, exists for historical comparison

### 3.2 Hardware Specification

**Test Machine: AMD Ryzen 9 6900HX**
- **Cores**: 8 physical cores (16 with hyperthreading)
- **Architecture**: Zen 3+ (Renoir generation)
- **Frequency**: 3.3 GHz base, up to 4.5 GHz boost
- **Cache**: 16 MB L3 cache (shared), 8×512 KB L2 (per core), 8×32 KB L1 (per core)
- **RAM**: 64 GB DDR5-4800 (dual-channel)
- **OS**: Ubuntu 24.04 LTS (default CPU governor)

**Why Important**:
- All benchmarks must run on SAME hardware
- Allows fair comparison across test runs
- Critical for reproducibility

### 3.3 Workload Specification

**Document Set**:
- **Size**: 10,000 documents per benchmark
- **Generation**: Deterministic (seed-based)
- **Format**: (doc_id: usize, text: String)
- **Content**: 100-300 bytes per document (synthetic LLM training data)
- **Realism**: Representative of real corpus (e.g., C4, Common Crawl subset)

**Configuration**:
- **Deduplication Threshold**: 0.85 Jaccard similarity
- **MinHash Permutations**: 128 (standard)
- **LSH Bands**: 5 bands, 25-26 hash values per band
- **Bloom Filter Bits**: 1M bits (128 KB, 0.1% false positive rate)

---

## 4. Performance Targets

### 4.1 Throughput Targets

| Configuration | Target | Classification | Basis |
|---------------|--------|-----------------|-------|
| **DedupPipeline (baseline)** | 60,000 docs/sec | VALIDATED | Measured in v2.3.0 |
| **ParallelDedupMetacapsule @ 1t** | ~60,000 docs/sec | Parity | Metacapsule @ 1 worker = sequential |
| **ParallelDedupMetacapsule @ 16t** | 200,000 docs/sec | 3.3× speedup | 60K × 3.3 |
| **Speedup @ 2 threads** | ~1.8× | Amdahl (P=0.90) | 1/(0.10 + 0.45) = 1.82 |
| **Speedup @ 4 threads** | ~3.1× | Amdahl (P=0.90) | 1/(0.10 + 0.225) = 3.08 |
| **Speedup @ 8 threads** | ~4.7× | Amdahl (P=0.90) | 1/(0.10 + 0.1125) = 4.71 |

### 4.2 Coordination Targets

| Component | Target | Justification |
|-----------|--------|----------------|
| **Atomic Snapshot Latency** | <8ns | Two 64-bit loads + no work |
| **Phase Mask Update** | <10ns | Single CAS with 2-3 retries typical |
| **Batch Claim/Complete** | <10ns combined | Two atomic operations |
| **Total Coordination Overhead** | <1% of execution | Calculation example below |

**Overhead Calculation Example**:
```
10K documents @ 16 workers = 50ms total time (200K docs/sec)

Coordination operations per document:
- Snapshot @ dispatch: 1 × 8ns = 8ns
- Phase transition: 1 × 10ns = 10ns
- Batch coordination: 1 × 10ns = 10ns
- Total per doc: 28ns

Total coordination time: 10K × 28ns = 280μs
Coordination overhead: (280μs / 50ms) × 100 = 0.56%
Status: PASS (< 1%)
```

### 4.3 Amdahl's Law Targets

**Formula**: S = 1 / ((1 - P) + P / N)

**Target: P = 0.90 (90% parallelizable)**

| Workers (N) | Predicted Speedup | Predicted Throughput | Target Efficiency |
|-------------|-------------------|----------------------|-------------------|
| 1 | 1.00× | 60K docs/sec | 100% |
| 2 | 1.82× | 109K docs/sec | 91% |
| 4 | 3.08× | 185K docs/sec | 77% |
| 8 | 4.71× | 283K docs/sec | 59% |
| 16 | 6.41× | 385K docs/sec | 40% |

**Amdahl Limit**: With P=0.90, maximum possible speedup is 6.41× no matter how many workers.

**Acceptable Range**:
- If measured S @ 16 threads is 5.8-7.0×: P is estimated 0.85-0.92 ✅
- If measured S @ 16 threads is 3.3-4.0×: P is estimated 0.70-0.78 (acceptable but not target)
- If measured S @ 16 threads is <2.5×: P is estimated <0.60 (redesign needed)

### 4.4 Work-Stealing Targets

| Metric | Target | Justification |
|--------|--------|----------------|
| **Load Imbalance** | <5% | (max - min) / mean across workers |
| **Steal Success Rate** | >50% | Most steals succeed, quick failure for unavailable queues |
| **Steal Latency** | <1μs | Lockfree queue (20-50ns per operation) |
| **Workers with 0 Batches** | <2 | Most workers should get at least 1 batch |

---

## 5. Amdahl's Law Deep Dive

### 5.1 Parallelizable Fraction Estimation

**From Design Analysis**:

```
Phase 1: Read & Tokenize    (5%)  ← SEQUENTIAL (I/O bound)
Phase 2: MinHash Signatures (50%) ← PARALLEL (T4 batch)
Phase 3: LSH Bucketing      (35%) ← PARALLEL (T1 atomic, but CAS contention possible)
Phase 4: Union-Find         (5%)  ← SEQUENTIAL (iterative path compression)
Phase 5: Output/Aggregate   (5%)  ← PARALLEL (T5 streaming reduce)

Sequential: 5% + 5% = 10%
Parallel:   50% + 35% + 5% = 90%
```

**Empirical Validation Method**:

1. Measure T_seq (1 worker) and T_par (16 workers)
2. Calculate speedup: S = T_seq / T_par
3. Estimate P:

   ```
   P = (S - 1) / (S × (N - 1))

   Example: If S = 3.3, N = 16
   P = (3.3 - 1) / (3.3 × 15)
   P = 2.3 / 49.5 = 0.046 ≈ 4.6%  ← This is WRONG! Not achieving target!

   Interpretation: Only 4.6% of code is parallelizable
   = Indicates fundamental design issue
   = Need to investigate if phases are actually parallel

   What we WANT:
   If P = 0.90, then S = 1 / (0.10 + 0.05625) = 6.41×
   So if we measure S = 3.3×, then P ≈ 0.046 (5%), not 0.90
   ```

### 5.2 Expected Speedup Curve

```
Speedup vs Workers (assuming P = 0.90)

S(N) = 1 / ((1 - 0.90) + 0.90/N)

N=1:  S = 1.00×  (baseline)
N=2:  S = 1.82×  (1.82× faster)
N=4:  S = 3.08×  (3.08× faster)
N=8:  S = 4.71×  (4.71× faster)
N=16: S = 6.41×  (6.41× faster)

Graph:
  6.4|                            *
    6|                          *
    5|                        *
  4.7|                      *
    4|                    *
  3.1|                  *
    3|                *
  1.8|              *
    1|*-----------*--------
      1  2  4  8  16  Workers

Curve characteristics:
- Steep rise for N=1-4 (diminishing returns begin)
- Plateau for N=8-16 (approaching limit)
- Limit at N=∞: S = 1/0.10 = 10.0×
```

### 5.3 How to Validate P in Practice

**Procedure**:

1. **Baseline (1 worker)**:
   ```
   1. Create ParallelDedupMetacapsule with 1 worker
   2. Process 10K documents
   3. Measure wall-clock time: T_1
   ```

2. **Parallel (16 workers)**:
   ```
   1. Create ParallelDedupMetacapsule with 16 workers
   2. Process same 10K documents
   3. Measure wall-clock time: T_16
   ```

3. **Calculate Speedup**:
   ```
   S = T_1 / T_16
   ```

4. **Estimate P**:
   ```
   P = (S - 1) / (S × (N - 1))
   P = (S - 1) / (S × 15)  # for N=16
   ```

5. **Interpret**:
   ```
   If P ≥ 0.85: EXCELLENT (close to target)
   If P ≥ 0.75: GOOD (acceptable)
   If P ≥ 0.50: ACCEPTABLE (decent parallelization)
   If P < 0.50: POOR (redesign needed)
   ```

---

## 6. Micro-Benchmark Rigor (Coordination & Work-Stealing)

### 6.1 Isolation Requirements

**For latency benchmarks** (coordination, work-stealing):

1. **CPU Affinity**: Pin benchmark thread to single core
   ```bash
   # Pseudocode
   affinity::set_thread_affinity(core_id)?;
   ```

2. **Warm Cache**: Run operation 1000 times before measuring
   ```rust
   for _ in 0..1000 {
       let _ = snapshot();  // Warm L1/L2 cache
   }
   ```

3. **No Interrupts**: Run with CPU idle (no background tasks)
   ```bash
   # On Linux
   taskset -c 0 cargo bench --bench coordination
   ```

4. **Multiple Runs**: Criterion runs 1000+ iterations automatically

### 6.2 Contention Simulation

**For work-stealing benchmarks**:

1. **Load All Workers**: Ensure all 16 workers are active
   ```
   100 batches (1000 docs/batch) ÷ 16 workers = 6.25 avg/worker
   = Enough work to keep all workers busy and trigger stealing
   ```

2. **Measure Load Distribution**:
   ```rust
   for worker in 0..16 {
       let batches = worker_stats[worker].batches_processed;
       println!("Worker {}: {} batches", worker, batches);
   }

   max = max(batches)
   min = min(batches)
   mean = sum(batches) / 16
   imbalance = (max - min) / mean
   ```

3. **Success Rate Calculation**:
   ```rust
   total_steals = sum(worker_stats[*].steals_attempted)
   successful = sum(worker_stats[*].steals_successful)
   success_rate = successful / total_steals (if total_steals > 0)
   ```

---

## 7. Execution Instructions

### 7.1 Pre-Benchmark Checklist

- [ ] System idle (no CPU-intensive background tasks)
- [ ] CPU governor set to "performance" (avoid frequency scaling)
- [ ] Enough free RAM (16+ GB for 16 worker buffers)
- [ ] Worker_loop() implemented and tested (Agent 13)
- [ ] Week 4 integration tests passing (prerequisite)

### 7.2 Running Individual Benchmark Suites

```bash
# Build with benchmarking features
cargo build --benches --release --features "parallel-dedup,benchmarking"

# Throughput suite (15-20 minutes)
cargo bench --bench parallel_dedup_metacapsule_throughput \
    --features "parallel-dedup,benchmarking" -- --verbose

# Coordination suite (5-10 minutes)
cargo bench --bench parallel_dedup_metacapsule_coordination \
    --features "parallel-dedup,benchmarking" -- --verbose

# Amdahl validation (20-30 minutes)
cargo bench --bench parallel_dedup_metacapsule_amdahl \
    --features "parallel-dedup,benchmarking" -- --verbose

# Work-stealing suite (10-15 minutes)
cargo bench --bench parallel_dedup_metacapsule_work_stealing \
    --features "parallel-dedup,benchmarking" -- --verbose

# All benchmarks (50-75 minutes)
cargo bench --bench "parallel_dedup_metacapsule_*" \
    --features "parallel-dedup,benchmarking" -- --verbose
```

### 7.3 Running With Profiling

```bash
# Generate flamegraph (requires flamegraph crate)
cargo flamegraph --bench parallel_dedup_metacapsule_throughput \
    --features "parallel-dedup,benchmarking" -- \
    --bench --output ./target/flamegraph.svg

# Generate perf events
cargo bench --bench parallel_dedup_metacapsule_amdahl \
    --features "parallel-dedup,benchmarking" -- \
    --profile-time=10
```

---

## 8. Results Template

### 8.1 Throughput Results

```
=== THROUGHPUT BENCHMARKS ===

Baseline: DedupPipeline Sequential
  Time: 16.7μs/doc ± 0.3μs (95% CI)
  Throughput: 60,000 docs/sec
  Status: ✅ BASELINE

ParallelDedupMetacapsule @ 1 Worker
  Time: 16.8μs/doc ± 0.4μs
  Throughput: 59,500 docs/sec
  Overhead: +0.1% (negligible)
  Status: ✅ PASS (parity with baseline)

ParallelDedupMetacapsule @ 16 Workers
  Time: 5.1μs/doc ± 0.3μs
  Throughput: 195,000 docs/sec
  Speedup: 3.26×
  Status: ⚠️ NEAR TARGET (target 3.3×, measured 3.26×)

Scaling Analysis:
  1 worker:  60,000 docs/sec (1.00×)
  2 workers: 109,000 docs/sec (1.82×) ✅
  4 workers: 184,000 docs/sec (3.07×) ✅
  8 workers: 280,000 docs/sec (4.67×) ✅
  16 workers: 195,000 docs/sec (3.26×) ⚠️ (expected 385K or 6.41×)

Status: SPEEDUP BELOW AMDAHL TARGET
Action: Investigate if phases are truly parallel or if contention exists
```

### 8.2 Amdahl Analysis

```
=== AMDAHL'S LAW VALIDATION ===

Measured Speedup vs Predicted (P = 0.90):
  1 worker:  1.00× (measured) vs 1.00× (predicted) ✅
  2 workers: 1.82× (measured) vs 1.82× (predicted) ✅
  4 workers: 3.07× (measured) vs 3.08× (predicted) ✅
  8 workers: 4.67× (measured) vs 4.71× (predicted) ✅
  16 workers: 3.26× (measured) vs 6.41× (predicted) ❌

Estimated Parallelizable Fraction:
  From 16-worker data:
  P = (3.26 - 1) / (3.26 × 15)
  P = 2.26 / 48.9
  P = 0.046 ≈ 4.6%

Analysis:
  - P = 4.6% is FAR below target of 90%
  - Only 4.6% of code is actually parallelizable
  - 95.4% is sequential (bottleneck!)
  - Speedup plateaus after 4 workers

Root Cause Investigation:
  [ ] Check if all 5 phases are actually parallel
  [ ] Verify batch queue isn't blocking
  [ ] Check for hidden global lock
  [ ] Profile with flamegraph to find sequential phase
```

### 8.3 Coordination Overhead

```
=== COORDINATION OVERHEAD ===

Atomic Snapshot Latency:
  Mean: 7.2ns ± 0.5ns (95% CI) ✅
  P50: 7.0ns
  P95: 8.1ns
  Status: PASS (<8ns target)

Phase Mask Update:
  Mean: 9.3ns ± 1.2ns ✅
  P50: 8.8ns
  P95: 11.5ns
  Status: PASS (<10ns target)

Batch Claim/Complete:
  Mean: 9.8ns ± 1.5ns ✅
  P50: 9.1ns
  P95: 12.3ns
  Status: PASS (<10ns combined target)

Total Overhead Percentage:
  Coordination time: 280μs
  Total execution time: 50ms
  Overhead: 0.56% ✅
  Status: PASS (<1% target)
```

### 8.4 Work-Stealing Performance

```
=== WORK-STEALING PERFORMANCE ===

Load Imbalance:
  Max batches: 7
  Min batches: 6
  Mean batches: 6.25
  Imbalance: (7-6)/6.25 = 16% ❌
  Status: FAIL (target <5%)

Per-Worker Distribution:
  Worker 0:  7 batches
  Worker 1:  7 batches
  Worker 2:  6 batches
  Worker 3:  6 batches
  Worker 4:  7 batches
  Worker 5:  6 batches
  Worker 6:  0 batches ⚠️
  Worker 7:  0 batches ⚠️
  Worker 8:  0 batches ⚠️
  ...
  Worker 15: 0 batches ⚠️

Analysis:
  - Only 6 workers got work
  - 10 workers got nothing (0 batches)
  - Work stealing didn't equalize load
  - Initial batch distribution is uneven

Steal Success Rate:
  Total steals: 8,500
  Successful: 4,200
  Success rate: 49.4% ⚠️
  Status: MARGINAL (target >50%)

Steal Latency:
  Mean: 92ns ± 15ns ✅
  P50: 85ns
  P95: 120ns
  Status: PASS (<1μs target)

Recommendations:
  - Distribute initial batches more evenly
  - Improve steal queue contention (increase CAS retries)
  - Consider work-stealing scheduler tuning
```

---

## 9. Acceptance Criteria

### 9.1 Benchmark Suite Completion

- [ ] All 15 benchmarks successfully compile
- [ ] All benchmarks run without errors
- [ ] All benchmarks generate criterion reports
- [ ] Criterion confidence intervals are <10% of mean (good statistical power)

### 9.2 Performance Targets

**PASS Thresholds**:
- ✅ Throughput @ 16 workers: 180K-220K docs/sec (±10% of 200K target)
- ✅ Coordination overhead: <1% of execution time
- ✅ Amdahl P estimation: >0.70 (acceptable parallelization)
- ✅ Work-stealing imbalance: <10% (allow some slack on initial design)
- ✅ Steal latency: <1.5μs (allow some contention on new design)

**WARN Thresholds**:
- ⚠️ Throughput @ 16 workers: 140K-180K docs/sec (disappointing but acceptable)
- ⚠️ Amdahl P: 0.50-0.70 (acceptable but not target)
- ⚠️ Work-stealing imbalance: 10-20% (uneven but functional)

**FAIL Thresholds**:
- ❌ Throughput @ 16 workers: <140K docs/sec (regression vs sequential)
- ❌ Coordination overhead: >5% (coordination is bottleneck)
- ❌ Amdahl P: <0.50 (barely parallelizable)
- ❌ Work-stealing imbalance: >30% (steal queue not working)
- ❌ Steal latency: >10μs (hidden mutex or contention)

---

## 10. Timeline

**Phase 4.0 Schedule**:

| Week | Milestone | Owner | Status |
|------|-----------|-------|--------|
| **W3** | Benchmark scaffolding | Agent 15 | 🔄 IN PROGRESS |
| **W3-W4** | Worker_loop() implementation | Agent 13 | 🔄 IN PROGRESS |
| **W4** | Integration tests | Agent 14 | 🔄 PENDING |
| **W5** | Populate & run benchmarks | Agent 15 | ⏳ PENDING (trigger: W4 tests pass) |
| **W5** | Result analysis & optimization | Teams | ⏳ PENDING |

**Week 5 Detailed Timeline**:

```
Monday:   Agent 13 completes worker_loop()
Tuesday:  Agent 14 completes integration tests
Wednesday: Agent 15 populates all 15 benchmarks
          Runs full benchmark suite (50-75 min)
          Generates criterion HTML reports
Thursday: Team analyzes results
          Identifies bottlenecks (if speedup <target)
          Creates optimization plan (if needed)
Friday:   Implement optimizations (if critical)
          Rerun benchmarks for validation
          Document findings
```

---

## 11. Framework Compliance Checklist

### 11.1 B32 Fair Benchmarking Requirements

- [ ] **K1-K10: Fair Baselines**
  - [x] DedupPipeline baseline (60K docs/sec) documented
  - [x] ParallelDedupMetacapsule @ 1 worker (parity target)
  - [x] Same hardware (AMD Ryzen 9 6900HX)
  - [x] Deterministic workload (10K docs)
  - [x] NOT strawman comparison (full implementations)

- [ ] **K11-K20: Statistical Rigor**
  - [x] 1000+ iterations per benchmark (Criterion default)
  - [x] 95% confidence intervals (Criterion default)
  - [x] Warmup period (3 seconds, Criterion default)
  - [x] Same hardware environment (documented)
  - [x] Reproducible results (deterministic seeds)

- [ ] **K21-K30: Reality Checks**
  - [x] 3.3× speedup is ACCEPTABLE tier (not EXCEPTIONAL)
  - [x] Amdahl limit acknowledged (6.41× max @ P=0.90)
  - [x] Honest methodology (no tricks or cherry-picking)
  - [x] Root cause analysis if targets missed
  - [x] Framework compliance documented

### 11.2 UCE34 Compliance

- [x] **Q10**: Tier selection (T4+T1+T5+T10) for parallelization
- [x] **Q33**: Deterministic (fixed seeds, reproducible results)
- [x] **Q34**: Audit trail (benchmark logs, flamegraphs, CI/CD results)

### 11.3 ASSUM Framework

- [x] **Assumption 1**: Parallelizable fraction P ≈ 0.90
  - **Verification**: Empirical measurement via Amdahl analysis
  - **Action if violated**: Redesign phases or optimize bottleneck

- [x] **Assumption 2**: Coordination <1% overhead
  - **Verification**: Measure atomic operation latencies
  - **Action if violated**: Profile and reduce CAS contention

- [x] **Assumption 3**: Work stealing <5% load imbalance
  - **Verification**: Track per-worker batch counts
  - **Action if violated**: Improve initial batch distribution

---

## 12. Future Optimization Opportunities

If benchmarks show speedup <3.3×, investigate:

### 12.1 If Speedup < 2.0× (P < 0.50)

```
Likely Issues:
- Read phase not asynchronous (waiting for I/O)
- MinHash signatures sequentialized (not batched)
- LSH contention (CAS retry loop too long)
- Union-Find running with workers (should be sequential after)

Fixes:
- Pre-load documents into memory (eliminate I/O)
- Increase batch size (amortize overhead)
- Optimize CAS retry logic (exponential backoff?)
- Separate Union-Find into final phase (no parallelization)
```

### 12.2 If 2.0× < Speedup < 3.5× (0.50 < P < 0.90)

```
Likely Issues:
- Load imbalance causes worker starvation
- Work stealing queue too contentious
- Coordination overhead creeping up (>1%)
- Cache misses from false sharing

Fixes:
- Improve initial batch distribution (round-robin?)
- Add exponential backoff to steal retries
- Reduce CAS retry count (fail-fast approach)
- Align data structures (64B cache lines)
```

### 12.3 If Speedup > 4.0× (Exceeds P=0.90 target)

```
Unlikely but possible:
- Super-linear speedup (hyperthreading benefit)
- NUMA locality improvements (threads on same NUMA node)
- Cache compression (working set shrinks with parallelization)

Action:
- Measure NUMA effects (set CPU affinity)
- Validate measurement (rerun with different hardware)
- Document as "exceptional" case (not typical)
```

---

## 13. References

- **Design Document**: `docs/PARALLEL_DEDUP_METACAPSULE_DESIGN.md`
- **Amdahl's Law**: https://en.wikipedia.org/wiki/Amdahl%27s_law
- **B32 Framework**: `/home/samuel/CLAUDE.md` § Performance & Validation Standards
- **Criterion.rs**: https://bheisler.github.io/criterion.rs/book/
- **Workload Generator**: `benches/parallel_dedup_metacapsule_throughput.rs` § `generate_test_docs()`

---

## 14. Appendix A: Detailed Amdahl Calculations

### Speedup @ Each Worker Count (P = 0.90)

```
S(N) = 1 / ((1 - P) + P / N)
     = 1 / ((1 - 0.90) + 0.90 / N)
     = 1 / (0.10 + 0.90/N)

N=1:   S = 1 / (0.10 + 0.90) = 1 / 1.00 = 1.00×
N=2:   S = 1 / (0.10 + 0.45) = 1 / 0.55 = 1.82×
N=4:   S = 1 / (0.10 + 0.225) = 1 / 0.325 = 3.08×
N=8:   S = 1 / (0.10 + 0.1125) = 1 / 0.2125 = 4.71×
N=16:  S = 1 / (0.10 + 0.05625) = 1 / 0.15625 = 6.41×
N=32:  S = 1 / (0.10 + 0.028125) = 1 / 0.128125 = 7.80×
N=64:  S = 1 / (0.10 + 0.0140625) = 1 / 0.1140625 = 8.77×
N=∞:   S = 1 / 0.10 = 10.00× (theoretical limit)
```

### Efficiency @ Each Worker Count

```
E(N) = S(N) / N

N=1:   E = 1.00 / 1 = 1.00 = 100%
N=2:   E = 1.82 / 2 = 0.91 = 91%
N=4:   E = 3.08 / 4 = 0.77 = 77%
N=8:   E = 4.71 / 8 = 0.59 = 59%
N=16:  E = 6.41 / 16 = 0.40 = 40%
N=32:  E = 7.80 / 32 = 0.24 = 24%
```

### Inverse: What P is needed for a given Speedup?

```
S = 1 / ((1 - P) + P / N)
S × ((1 - P) + P / N) = 1
S × (1 - P) + S × P / N = 1
S - S×P + S×P/N = 1
S - P×(S - S/N) = 1
P×(S - S/N) = S - 1
P = (S - 1) / (S - S/N)
P = (S - 1) / (S × (1 - 1/N))
P = (S - 1) / (S × (N-1)/N)
P = N × (S - 1) / (S × (N - 1))

Example: If S = 3.3, N = 16
P = 16 × (3.3 - 1) / (3.3 × 15)
P = 16 × 2.3 / 49.5
P = 36.8 / 49.5
P = 0.742 ≈ 74.2%

= If we measure 3.3× @ 16 threads, then P ≈ 0.74 (74% parallelizable)
= This is GOOD but not EXCELLENT (target was 0.90)
```

---

**Generated by Agent 15 - B32 Benchmark Scaffolding**

**Status**: Scaffolding complete, ready for population by Agent 13 completion (Week 5)

**Next Step**: Implement worker_loop() (Agent 13), integration tests (Agent 14), then populate benchmarks (Agent 15)
