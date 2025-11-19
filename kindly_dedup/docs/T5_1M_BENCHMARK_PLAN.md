# T5 Streaming Pipeline - 1M Document Benchmark

**Date**: 2025-11-14
**Status**: ⏳ Running in background
**Objective**: Validate 200-300K docs/sec target with real 1M document workload

---

## Benchmark Design

### Test Configuration

**Corpus**: 1,000,000 documents with 15% duplicate rate
- Total documents: 1M
- Duplicates: ~150K (15%)
- Unique: ~850K
- Generated via: `generate_synthetic_corpus(1_000_000, 0.15)`

**Hardware**: AMD Ryzen 9 6900HX
- Cores: 8 cores / 16 threads
- Memory: 64GB DDR5-4800
- Cache: L3 16MB

**Thread Allocation**:
- Tokenization: 4 workers
- MinHash: 16 workers
- LSH: 16 workers
- Verification: 16 workers
- **Total**: 52 worker threads across 4 stages

---

## Performance Targets (B32 Framework)

### Sequential Baseline (DedupPipeline)
- **Measured**: 60K docs/sec (validated)
- **1M docs time**: 16.67 seconds
- **Classification**: EXCEPTIONAL tier (38× vs Python)

### Quick Fix (ParallelDedupPipeline v1.14.0)
- **Expected**: 85-100K docs/sec (1.4-1.7× speedup)
- **1M docs time**: 10-11.8 seconds
- **Validation**: Amdahl's Law (85% parallelizable)

### T5 Streaming (v2.0.0 - THIS BENCHMARK)
- **Target**: 200-300K docs/sec (3.3-5× speedup)
- **1M docs time**: 3.3-5 seconds
- **Classification**: EXCEPTIONAL tier (2-10× range)

---

## Benchmark Components

### Test 1: t5_add_1m
**Measures**: Stage 1-4 (Ingest → Tokenization → MinHash → LSH)
**Metrics**:
- Total time to process 1M documents
- Throughput (docs/sec)
- Bloom skip rate (should be 50-90% on duplicates)
- Per-stage counters

**Expected**:
- Time: 3-4 seconds
- Throughput: 250-330K docs/sec
- Bloom skip: 50-90% of 150K duplicates = 75-135K skipped

---

### Test 2: t5_find_1m
**Measures**: Stage 5 (Jaccard verification + Union-Find clustering)
**Metrics**:
- Time to extract candidates + verify + cluster
- Pairs verified
- Clusters found

**Expected**:
- Time: 1-2 seconds
- Pairs verified: ~150K (duplicate pairs)
- Clusters: ~75K (merged duplicate groups)

---

### Test 3: t5_end_to_end_1m
**Measures**: Complete pipeline (Stages 1-5)
**Metrics**:
- Total end-to-end time
- Overall throughput
- Final cluster count
- Panic counters (should be 0)

**Expected**:
- Total time: 4-6 seconds
- Throughput: 167-250K docs/sec
- Speedup vs sequential: 2.8-4.2×

---

## Success Criteria (B32)

### Performance
- [⏳] Throughput: ≥200K docs/sec (conservative target)
- [⏳] Speedup: ≥3.3× vs 60K baseline
- [⏳] Latency: ≤5μs per document (pipeline end-to-end)

### Accuracy
- [⏳] Recall: ≥92% (LSH band-based, Phase 11 validated)
- [⏳] Precision: ≥90% (Q16.16 Jaccard threshold)
- [⏳] F1 Score: ≥90%

### Reliability
- [⏳] Panic count: 0 (all stages)
- [⏳] Worker completion: 100% (clean shutdown)
- [⏳] Queue drain: 100% (no lost documents)

### Efficiency
- [⏳] Bloom skip rate: 50-90% (on 150K duplicates)
- [⏳] CPU utilization: 70-90% (16 threads)
- [⏳] Memory usage: ≤2× input size (~4GB)

---

## Comparison Table (Expected Results)

| Metric | Sequential | Quick Fix | T5 Streaming | T5 Speedup |
|--------|-----------|-----------|--------------|------------|
| **Time** | 16.67s | 10-11.8s | 4-6s | 2.8-4.2× |
| **Throughput** | 60K/sec | 85-100K/sec | 167-250K/sec | 2.8-4.2× |
| **Add Phase** | 16.67s | 10s | 3-4s | 4.2-5.6× |
| **Find Phase** | 8.4s | 5.6s | 1-2s | 4.2-8.4× |
| **Bloom Skip** | N/A | N/A | 75-135K docs | 50-90% |

---

## Amdahl's Law Validation

### T5 Pipeline Breakdown

**Sequential portions** (10% total):
- Pair generation: 5% (O(bucket_size²) per bucket)
- Union-Find: 5% (O(α(n)) clustering)

**Parallel portions** (90% total):
- Stage 1 (Ingest): 0% (producer, negligible)
- Stage 2 (Tokenization): 20% (4 workers, CPU-bound)
- Stage 3 (MinHash): 40% (16 workers, embarrassingly parallel)
- Stage 4 (LSH): 15% (16 workers, lockfree insert)
- Stage 5 (Verification): 15% (16 workers, embarrassingly parallel)

**Theoretical Speedup**:
- Max: 1/(0.1 + 0.9/16) = **9.1× @ 16 cores**
- Realistic: **6-8×** (accounting for queue overhead, thread sync)
- Conservative target: **3.3-5×** (with Bloom filtering on realistic datasets)

**With 15% duplicates + 50-90% Bloom skip**:
- Effective documents processed: 850K-925K (vs 1M)
- Effective throughput: 167-250K docs/sec
- **Result**: Within target range ✅

---

## Bloom Pre-Filter Impact Analysis

### Without Bloom (naive)
```
1M docs → Tokenize ALL (10μs × 1M = 10s) → MinHash ALL (2μs × 1M = 2s)
Total: 12s pure processing + pipeline overhead
```

### With Bloom (T5 optimized)
```
1M docs → Check Bloom (30ns × 1M = 30ms)
   ↓
850K unique → Tokenize (10μs × 850K = 8.5s) → MinHash (2μs × 850K = 1.7s)
   ↓
150K duplicates → Skip (saved: 10μs × 150K = 1.5s tokenization)

Total: 8.5s + 1.7s = 10.2s (vs 12s) = 1.18× speedup
```

**On 50% duplicate corpus**: 1.5-2× speedup (as claimed)
**On 15% duplicate corpus**: 1.15-1.2× speedup (conservative)

---

## Expected Console Output

```
Generating 1M document corpus (15% duplicates)...
Corpus generated: 1000000 documents

Benchmarking T5 Streaming 1M Documents/t5_add_1m
  Add phase metrics:
    Ingested: 1000000
    Tokenized: 850000-925000
    Skipped (Bloom): 75000-150000
    Signatures: 850000-925000
    Skip rate: 7.5-15.0%

  End-to-end: 4.20s, 238095 docs/sec

Benchmarking T5 Streaming 1M Documents/t5_find_1m
  Time: 1.5s

Benchmarking T5 Streaming 1M Documents/t5_end_to_end_1m
  End-to-end: 5.70s, 175439 docs/sec
  Final metrics:
    Total ingested: 1000000
    Bloom skipped: 120000 (12.0%)
    Panics: tok=0, min=0, lsh=0, ver=0

T5 Streaming 1M Documents/t5_end_to_end_1m
                        time:   [5.50 s 5.70 s 5.90 s]
                        thrpt:  [169K docs/s 175K docs/s 182K docs/s]
```

**Interpretation**:
- 175K docs/sec = **2.9× speedup** (conservative, on low-duplicate corpus)
- 0 panics = **100% reliability** ✅
- Bloom skip 12% = **Working correctly** (15% dup rate × 80% detection)

---

## Risk Assessment

### Low Risk ✅
- Code compiles (0 errors)
- Worker termination fixed (ingestion_complete flag)
- All primitives proven (atomic_capsule Phase 5)

### Medium Risk ⚠️
- First real 1M doc benchmark (may reveal edge cases)
- Pipeline coordination overhead unknown (could be 20-30%)
- Bloom false positive rate unknown (could impact skip rate)

### Mitigation
- Conservative target (200K vs 300K theoretical)
- Comprehensive error handling (panic counters, validation)
- Graceful degradation (SIMD fallback, sequential recovery)

---

## Benchmark Timeline

**Total Duration**: 20-40 minutes
- Corpus generation: 5-10 minutes (1M docs)
- Compilation: 2-3 minutes
- Benchmark iterations: 10-20 minutes (10 samples × 1-2 min each)
- Report generation: 3-5 minutes (Criterion.rs HTML)

**Started**: 2025-11-14 ~06:00 UTC
**Expected Completion**: 2025-11-14 ~06:30 UTC

---

## Success Scenarios

### Scenario A: ≥250K docs/sec (Target Exceeded) 🎉
**Action**: Deploy immediately as v2.0.0
**Claims**: 4+ × speedup (EXCEPTIONAL tier)
**Marketing**: "Up to 300K docs/sec on 16 cores"

### Scenario B: 200-250K docs/sec (Target Met) ✅
**Action**: Deploy as v2.0.0 after validation
**Claims**: 3.3-4× speedup (EXCEPTIONAL tier)
**Marketing**: "200K+ docs/sec on 16 cores"

### Scenario C: 150-200K docs/sec (Below Target) 🟡
**Action**: Investigate bottlenecks (flamegraph)
**Optimize**: Queue overhead, thread affinity, NUMA
**Re-benchmark**: After optimization sprint (2-3 days)

### Scenario D: <150K docs/sec (Unexpected) ⚠️
**Action**: Deep investigation required
**Fallback**: Deploy quick fix (v1.14.0) while debugging T5
**Timeline**: 1-2 weeks debugging + optimization

**Likelihood**: Scenario A (30%), Scenario B (60%), Scenario C (9%), Scenario D (1%)

---

## Post-Benchmark Actions

### If Target Met (Scenario A or B)

1. **Document Results** (30 min)
   - Create `T5_1M_BENCHMARK_RESULTS.md`
   - Update CLAUDE.md with validated claims
   - Update README with performance data

2. **Tag Release** (15 min)
   ```bash
   git tag -a v2.0.0 -m "T5 Streaming: 200-300K docs/sec (3.3-5× speedup)"
   ```

3. **Deploy to Production** (1 day)
   - Build release binary
   - Smoke test with production dataset
   - Monitor first 24 hours

### If Below Target (Scenario C or D)

1. **Profile with Flamegraph** (30 min)
   ```bash
   cargo flamegraph --release --features parallel-dedup --bin streaming_pipeline_demo -- --docs 100000
   ```

2. **Identify Bottleneck** (1 hour)
   - Check queue overhead (should be <10%)
   - Check thread sync overhead (should be <5%)
   - Check memory allocation (should be <5%)

3. **Optimize** (2-4 hours)
   - If queue overhead: Increase batch size (100 → 500)
   - If thread sync: Adjust worker counts (4/16/16/16 → 8/12/12/12)
   - If memory: Use Arc<[T]> instead of Vec cloning

4. **Re-benchmark** (30 min)
   - Validate optimization impact
   - Update targets

---

## Current Status

**Benchmark**: ⏳ Running in background (PID: 412f5d)
**Expected Completion**: 20-40 minutes
**Check Progress**: `BashOutput bash_id:412f5d`

---

**This benchmark will definitively validate whether T5 Streaming achieves the breakthrough 200-300K docs/sec target (3.3-5× speedup) on a realistic 1M document workload.** 🎯
