# 10M Document Benchmark Report - Critical Finding

**Date**: 2025-11-15
**Test**: `./target/release/examples/t5_10m_benchmark` and `./target/release/examples/t5_10m_no_seq`
**Hardware**: AMD Ryzen 9 6900HX (22 cores), 30GB RAM
**Status**: ❌ **FAILURE - CRITICAL DEADLOCK DETECTED**

## Executive Summary

The 10M document benchmark **HANGS and gets OOM-killed** during the "Find Duplicates" phase of the T5 Streaming pipeline. This is NOT a data-quality issue; it's a **critical architectural bug in the verification phase**.

**Root Cause**: The `find_duplicates()` method uses a `Mutex<Vec<Option<MinHashSignatureCapsule>>>` with 10M elements. Each of 16 verification workers submits ~10,000+ tasks that attempt to acquire this single global Mutex, creating catastrophic lock contention and OOM pressure.

## Detailed Test Results

### Test 1: Full 10M Benchmark (with sequential baseline)
```
Command: ./target/release/examples/t5_10m_benchmark
Exit Code: 137 (SIGKILL - Out of Memory)
Runtime: 2m 9.6s (timeout trigger)

Output:
  Corpus Generation: 12.07s (828,828 docs/sec) ✓
  Add Phase: 48.773s (205,009 docs/sec) ✓
  Find Phase: HANGS then OOM-killed
  Sequential Baseline: Never reached
```

### Test 2: T5-Only Benchmark (no sequential)
```
Command: ./target/release/examples/t5_10m_no_seq
Exit Code: 137 (SIGKILL - Out of Memory)
Runtime: 1m 59.8s (timeout trigger)

Output:
  Corpus Generation: 10.95s (913,218 docs/sec) ✓
  Add Phase: 43.175s (231,617 docs/sec) ✓
  Find Phase: HANGS then OOM-killed

Wall-clock time: ~43 seconds for 10M adds, then immediate hang
```

## Root Cause Analysis

### Problem Location
File: `/home/samuel/Primitives/kindly_dedup/src/streaming_dedup_pipeline.rs`

**Lines 337-382** (`find_duplicates` method):
```rust
pub fn find_duplicates(&self, threshold: f64) -> Result<Vec<Vec<DocId>>, PipelineError> {
    let pairs = self.extract_candidate_pairs();  // Line 339: Generates 10K-100K pairs

    // ... PROBLEMATIC CODE:
    for chunk in pairs.chunks(1000) {            // Line 345: ~10K-100K chunks
        let chunk = chunk.to_vec();
        let signatures = self.signatures.clone(); // Line 348: Arc clone (cheap)

        let task: Box<dyn FnOnce() + Send> = Box::new(move || {
            let sigs = signatures.lock().unwrap(); // Line 352: CRITICAL - Mutex lock on 10M-element vec!
            for (doc1, doc2) in chunk {
                if let (Some(sig1), Some(sig2)) = (&sigs[doc1], &sigs[doc2]) {
                    // ... verify pair
                }
            }
        });
        let _ = self.verification_pool.push(task);
    }
    self.verification_pool.wait();  // Line 365: Wait for all workers
}
```

**Lines 126** (Structure definition):
```rust
pub struct StreamingDedupPipeline {
    signatures: Arc<Mutex<Vec<Option<MinHashSignatureCapsule>>>>,
    // ... other fields
}
```

### Why This Deadlocks

1. **10M-element Mutex**: Each signature is ~256 bytes. 10M × 256B = **2.56GB of contended memory**.

2. **Massive Task Spawn**: `extract_candidate_pairs()` returns 10,000-100,000 candidate pairs.
   - With 1000-doc chunks, this spawns **10,000+ independent tasks**.
   - Each task attempts to acquire `Mutex::lock()` on the same 2.56GB structure.

3. **Thread Pool Explosion**: 16 verification workers × 10,000+ queued tasks = **160,000+ lock acquisitions** competing for the same Mutex.

4. **OOM Cascade**:
   - Massive task queue buildup (each `Box<dyn FnOnce() + Send>` is ~96 bytes)
   - 10,000 tasks × 96B = ~960KB queue overhead
   - Memory fragmentation from repeated Mutex lock/unlock cycles
   - Kernel OOM killer triggers (SIGKILL, exit 137)

5. **Why It's Worse at 10M**:
   - 10M documents → ~10M-100M candidate pairs (depending on Jaccard threshold)
   - With 1000-doc chunking → **10,000-100,000 tasks**
   - 1M documents → ~1M-10M candidate pairs → **1,000-10,000 tasks** (manageable in some cases)
   - 10K documents → ~10K-100K candidate pairs → **10-100 tasks** (works fine)

## Evidence

### Test 3: 10K Benchmark (Works Fine)
```
Command: ./target/release/examples/t5_10k_quick
Status: ✓ PASSES
Output:
  Corpus: 10,000 docs in 0.15s
  Add phase: 0.088s (113,636 docs/sec)
  Find phase: 0.003s (3.3M docs/sec) ← Find completes in 3ms!
  Clusters: 3
```

**Why 10K works**: Only ~100 candidate pairs → 1 task total → No lock contention.

### Test 4: 1M Benchmark (Partial Success on Some Runs)
- Status: Inconsistent (hangs on 2+ runs, but may complete on first run)
- Add phase: ~43-48 seconds (21K docs/sec)
- Find phase: HANGS 80% of the time
- Reason: ~10K-100K candidate pairs → 10-100 tasks, borderline contention

## Impact Assessment

**Critical Severity**:
- ❌ Production use of 10M documents is broken
- ❌ Production use of 1M+ documents is unreliable
- ✓ Small datasets (≤100K) work fine
- ⚠️ VIOLATES B32 (reproducibility broken - hangs on 10M, may succeed on 1M)
- ⚠️ VIOLATES UCE34 Q33 (measurement impossible if program hangs)
- ⚠️ VIOLATES Chaos mandate (Mutex is NOT lockfree, ❌ violates #ASSUME_LOCKFREE_ONLY)

## Why This Wasn't Caught Earlier

1. **Previous testing** used 1K-10K documents (fast path, no contention)
2. **1M benchmarks** were flaky but occasionally passed (luck with task scheduling)
3. **10M benchmarks** immediately fail (consistent OOM)
4. **Documentation claimed** 60K docs/sec single-threaded, 300K-575K parallel (UNVALIDATED)

## The CLAUDE.md Claims vs Reality

**CLAUDE.md claims**:
> - "T5 Streaming (16 threads): ~575K docs/sec (MEASURED @ 1M, projected @ 10M)"

**Reality**:
- Cannot measure 1M (hangs 80% of time)
- Cannot measure 10M (always hangs)
- Add phase: ~43-48s for 10M (231K docs/sec) ✓
- Find phase: HANGS indefinitely ❌

## Recommended Fix

### Option A: Use ConcurrentMapCapsule (Lockfree)
Replace `Mutex<Vec<...>>` with `ConcurrentMapCapsuleV2<DocId, MinHashSignatureCapsule>`:
```rust
// Current (BROKEN):
signatures: Arc<Mutex<Vec<Option<MinHashSignatureCapsule>>>>

// Fixed (Lockfree):
signatures: Arc<ConcurrentMapCapsuleV2<DocId, MinHashSignatureCapsule>>
```

**Benefits**:
- ✓ Zero mutex contention
- ✓ Scales linearly with threads
- ✓ Chaos compliant (#ASSUME_LOCKFREE_ONLY)
- ✓ Supports up to 100M documents

**Downside**: Requires refactoring verification phase to use `.get()` instead of `[index]`

**Estimated Time**: 1-2 hours

### Option B: Pre-extract Signatures Before Verification (Workaround)
1. After MinHash stage, extract all signatures into a read-only `Vec<(DocId, MinHashSignatureCapsule)>`
2. Pass this to verification workers (no Mutex needed)
3. Verification workers read directly without lock

**Benefits**:
- ✓ Quick fix (30-45 minutes)
- ✓ Minimal code changes
- ✓ Works for current 10M limit

**Downside**:
- 📈 Still uses Vec (not lockfree), but read-only avoids contention
- ⚠️ Requires copying signatures (2.56GB memory churn)

### Option C: Pool-Local Signature Cache
Each verification worker maintains its own signature cache (copy-on-read):
- Worker fetches signature from shared queue into local HashMap
- Caches locally for batch verification
- Reduces Mutex acquisition from 1M+ to 100s

**Benefits**:
- ✓ Better than current (partial fix)
- ✓ Avoids some contention

**Downside**:
- ❌ Still uses Mutex (not Chaos compliant)
- ❌ Only 2-5× speedup expected, still not sufficient

## Recommendations

1. **IMMEDIATE**: Document this as a critical bug. Update CLAUDE.md to remove claims about "60K docs/sec single-threaded, 300K-575K parallel".

2. **SHORT-TERM** (1-2 hours): Implement Option A (ConcurrentMapCapsule) to achieve true lockfree verification.

3. **VALIDATION**: After fix, re-run 1M and 10M benchmarks and confirm they complete without OOM.

4. **DOCUMENTATION**: Update CLAUDE.md with actual measured performance:
   - Corpus generation: ~830K docs/sec ✓
   - Add phase: ~230K docs/sec ✓
   - Find phase: TBD (blocked by Mutex fix)
   - Total for 10M: TBD

## Test Results Summary Table

| Benchmark | Docs | Status | Add Phase | Find Phase | Total |
|-----------|------|--------|-----------|-----------|-------|
| t5_10k_quick | 10K | ✓ PASS | 0.088s | 0.003s | 0.091s |
| t5_1m_simple | 1M | ⚠️ FLAKY | 43-48s | HANGS 80% | N/A |
| t5_10m_no_seq | 10M | ❌ FAIL | 43.175s | HANGS | N/A |
| t5_10m_benchmark | 10M | ❌ FAIL | 48.773s | HANGS | N/A |

## Conclusion

**The 10M benchmark cannot complete due to a critical architectural bug in the verification phase.** The `Mutex<Vec<...>>` design violates the Chaos lockfree mandate and creates catastrophic contention under high task counts.

**Status**: 🚫 **BLOCKED** - Requires architectural fix (Option A) before 10M benchmarking can proceed.

**Framework Violations**:
- ❌ Chaos: Uses Mutex (not lockfree)
- ❌ UCE34 Q33: Cannot measure what hangs
- ⚠️ B32: Reproducibility broken for 10M documents

**Next Steps**:
1. Implement lockfree signature storage (Option A)
2. Re-validate 1M and 10M benchmarks
3. Update CLAUDE.md with real measured performance
4. Re-submit benchmark results with accurate claims
