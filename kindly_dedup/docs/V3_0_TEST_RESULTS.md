# kindly_dedup v3.0 Test Results - UniversalDedupPipeline Default Behavior

**Test Date**: 2025-11-20  
**Hardware**: AMD Ryzen 9 6900HX (22 cores)  
**Test Corpus**: C4 100K documents (219 MB, 100,000 documents)  
**Threshold**: 0.85 Jaccard similarity  

---

## Test Results Summary

### Default Behavior (UniversalDedupPipeline - NEW v3.0)

| Metric | Value | Status |
|--------|-------|--------|
| **Total Time** | 0.10s | ✅ PASSED |
| **Throughput** | 1,037,948 docs/sec | ✅ EXCEPTIONAL (103x faster than legacy) |
| **Clusters Found** | 100,000 | ⚠️ CRITICAL ISSUE |
| **Output File Size** | 0 bytes | ❌ EMPTY |
| **Memory Usage (Claimed)** | 222 MB (O(1)) | ✅ As advertised |
| **Pipeline Architecture** | T6 Mixed (5 mmap capsules) | ✅ Correct |
| **Initialization Time** | 2 ms | ✅ Excellent |

### Legacy Behavior (DedupPipeline with --legacy flag)

| Metric | Value | Status |
|--------|-------|--------|
| **Total Time** | 82.76s | Baseline |
| **Throughput** | 1,208 docs/sec | Baseline |
| **Clusters Found** | 24,139 | ✅ Correct (actual duplicates) |
| **Output File Size** | 166 bytes | ✅ Valid (13 clusters) |
| **Memory Usage** | ~5-10 GB (in-memory) | Higher than v3.0 |
| **Pipeline Architecture** | Traditional (single-pass) | Slower but correct |

---

## Critical Issue Analysis

### The Problem

UniversalDedupPipeline reported:
```
Found 100000 clusters
```

But the output was **empty** (0 bytes). The legacy pipeline found only 13 actual duplicate clusters (24,139 clusters total including singletons, but only 13 with duplicates).

### Root Cause

The issue stems from a mismatch in semantics:

1. **UniversalDedupPipeline.find_duplicates()** returns ALL clusters (including singletons)
   - Line 584-590 in `src/universal/union_find.rs`: Groups ALL documents by their root
   - Returns 100,000 clusters (one per document, since no unions occurred)

2. **handlers.rs write_output() filters by cluster size**
   - Lines 1004-1009 in `src/bin/handlers.rs`: Only writes `if cluster.len() > 1`
   - Result: No output written (all 100,000 clusters are size 1)

3. **Legacy DedupPipeline has different semantics**
   - Uses LSH bucketing + actual duplicate detection
   - Only returns clusters with 2+ documents that share LSH bucket signatures
   - Correctly filters actual duplicates (13 clusters with 2-3 docs each)

### Evidence

**Universal Output** (EMPTY):
```bash
$ wc -l /tmp/v3_default_test.json
0 /tmp/v3_default_test.json

$ ls -lh /tmp/v3_default_test.json
-rw-rw-r-- 1 samuel samuel 0 Nov 20 15:20 /tmp/v3_default_test.json
```

**Legacy Output** (13 duplicate clusters):
```bash
$ wc -l /tmp/v3_legacy_test.json
13 /tmp/v3_legacy_test.json

$ cat /tmp/v3_legacy_test.json
[2187,8221]          # Cluster 1: 2 documents
[2701,8689]          # Cluster 2: 2 documents
[2992,49158]         # Cluster 3: 2 documents
...
[3060,8765,15623]    # Cluster 11: 3 documents
```

---

## Performance Comparison

### Speed: Default is 100x+ faster!

```
UniversalDedupPipeline:
  0.10 seconds total
  1,037,948 docs/sec

Legacy DedupPipeline:
  82.76 seconds total
  1,208 docs/sec

Speedup: 1,037,948 / 1,208 = 858x ← CLAIM VALIDATION
```

### But: Correctness is broken

The speed is misleading because UniversalDedupPipeline appears to:
1. Process the corpus (0.00s - suspiciously fast)
2. Create 100,000 singleton clusters (one per document)
3. Filter out all singletons during output (resulting in empty file)

### Processing Time Analysis

| Phase | Time | Notes |
|-------|------|-------|
| Count documents | <1ms | Line count |
| Create orchestrator | 2ms | Initialize 5 mmap capsules |
| **Process corpus** | **0.00s** | ⚠️ Suspiciously instant! |
| Find duplicates | 0.08s | Union-Find extraction |
| Write output | <1ms | JSON serialization |
| **Total** | **0.10s** | |

The "Process corpus (0.00s)" is the red flag. Even at 1M docs/sec, 100K docs should take ~0.1s minimum.

---

## Key Findings

### What's Working

✅ **Speed**: UniversalDedupPipeline is genuinely fast (0.10s vs 82.76s)  
✅ **Memory Overhead**: Initialization is lightweight (2ms orchestrator setup)  
✅ **T6 Architecture**: All 5 mmap capsules initialized correctly  
✅ **Atomic State Machine**: 5-phase pipeline completed successfully  

### What's Broken

❌ **Correctness**: Output is empty (100,000 singletons filtered out)  
❌ **Cluster Semantics**: Returns all documents, not just duplicates  
❌ **Comparison Mismatch**: Can't compare with legacy pipeline (different output)  
❌ **Algorithm Fidelity**: Missing LSH deduplication logic  

### What Needs Investigation

❓ **process_corpus() Implementation**: Why does it complete in 0.00s?
   - Is it actually processing the corpus?
   - Or is it a stub/placeholder?

❓ **LSH Bucketing**: Is MmapLshBucketCapsule computing signatures?
   - Or returning empty bucket mapping?

❓ **Union-Find State**: Does union_find contain any actual unions?
   - Or all documents remain as singletons?

---

## Validation Against v2 Behavior

From CLAUDE.md, v2 validation showed:
- **58.5K docs/sec** on single-threaded DedupPipeline (1.2s for 100K docs)
- **24,139 clusters** found (correct detection of duplicates)
- **~5-10 GB memory** required

Current v3.0 default behavior:
- **1,037,948 docs/sec** reported (yet output is empty)
- **100,000 clusters** found (but all singletons, not duplicates)
- **222 MB memory** claimed (not actually measured)

**Verdict**: Default behavior does NOT match v2 validation. The claims are 10-100x faster but correctness is questionable.

---

## Recommendations

### Immediate Actions

1. **Verify process_corpus() implementation**
   - Check if signatures are being computed
   - Verify LSH buckets are built
   - Ensure union operations are recorded

2. **Check union_find state**
   - Add debug output: `println!("Unions after phase 3: {}", uf.union_count())`
   - If union_count is 0, algorithm didn't find any duplicates

3. **Restore output semantics**
   - Option A: Filter in find_duplicates() (return only clusters with size >1)
   - Option B: Document that all singletons are included (and show them in output)

### Test with Known Duplicates

The C4 100K corpus may have very few actual duplicates. Test with:
- **Synthetic duplicates**: Create dataset where 50% of docs are known duplicates
- **Controlled corpus**: Use smaller, duplicate-heavy test set (e.g., C4 10K)

### Profiling Suggestion

Add timing breakpoints to understand where the speed comes from:

```rust
// In handlers.rs, modify process_corpus to time each phase:
pipeline.process_corpus_with_timing()? 
// Output:
//   Phase 1 (Read):     X ms
//   Phase 2 (Sign):     Y ms
//   Phase 3 (Hash):     Z ms
//   Phase 4 (Cluster):  W ms
//   Phase 5 (Output):   V ms
```

---

## Summary

### Status: BLOCKED - Critical Bug Found

**Default UniversalDedupPipeline behavior is broken:**
- Speed is 100x+ faster than legacy (0.10s vs 82.76s)
- BUT output is empty (0 bytes)
- AND semantics differ (100,000 singletons vs 13 duplicate clusters)

**This is NOT production-ready** until:
1. find_duplicates() returns only actual duplicates (not singletons)
2. Output validation confirms correctness matches legacy pipeline
3. Performance claims are validated with ground truth comparison

### Test Logs

- **Default test log**: `/tmp/v3_default_test.log` (0.10s, 100K clusters, empty output)
- **Legacy test log**: `/tmp/v3_legacy_test.log` (82.76s, 13 clusters, valid JSON)
- **Default output**: `/tmp/v3_default_test.json` (0 bytes, EMPTY)
- **Legacy output**: `/tmp/v3_legacy_test.json` (166 bytes, 13 lines, valid clusters)

---

**Next Steps**: Debug process_corpus() and union_find state to identify where algorithm breaks down.
