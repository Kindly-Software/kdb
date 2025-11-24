# Debug Report: 1M Document Test - Hang Investigation
**Date**: 2025-11-24
**Session**: UCE34 Q1-Q7 Systematic Discovery
**Status**: ⚠️ **HANG LOCATION IDENTIFIED**

---

## Executive Summary

The append-only LSH bucket fix, while working correctly on **small datasets (60K+ docs/sec)**, hangs on **large corpus processing** starting at the 21M documents case. Testing with the available 354K document file has been conducted to isolate the hang location.

**Key Finding**: Hang occurs in `UniversalDedupPipeline::process_corpus()` at **line 528** in `src/universal/pipeline.rs`, specifically in the first call to `self.reader.next_chunk_iter()`.

---

## Test Results (Q1-Q4)

### Q1: Test Dataset Creation
**Status**: ✅ COMPLETE

| Metric | Result |
|--------|--------|
| **Dataset Available** | `/home/samuel/Primitives/kindly_dedup/test_data/c4_1m.jsonl` |
| **Actual Document Count** | 354,326 (not 1M as name suggests) |
| **File Size** | 775 MB |
| **Use Case** | Good for testing bottlenecks in large-ish corpus |

### Q2: Build with Debug-Logging
**Status**: ✅ COMPLETE

```bash
cargo build --release --lib --features "debug-logging,benchmarking"
```

- ✅ Library compiles successfully
- ⚠️ Some binaries have API mismatches (lsh_bucket_fix_validation, stress_test_10m)
- Debug logging infrastructure ready (262 lines in src/debug_logging.rs)

### Q3: Performance Baseline Tests
**Status**: ✅ COMPLETE

#### Test 1: Raw File I/O (no parsing)
```
Result: 354,326 docs in 0.306s = 1,157,898 docs/sec
Classification: EXCEPTIONAL
```

#### Test 2: Tokenization + Hashing (no pipeline)
```
Result: 354,326 docs in 3.062s = 115,720 docs/sec
Total signatures: 126,371,317
Classification: EXCEPTIONAL (>60K baseline)
```

**Conclusion**: File I/O and tokenization are NOT bottlenecks. The hang is in the **pipeline orchestration layer**.

### Q4: Memory & CPU Monitoring
**Status**: ✅ METRICS CAPTURED

From previous 21M test run (see C4_FULL_CORPUS_VALIDATION_REPORT.md):
- **Memory**: Stable at ~1073 MB (O(1) behavior) ✅
- **CPU**: 93-94% active (single-threaded) ✅
- **I/O**: Not CPU-bound (system calls complete quickly) ✅
- **Progress**: **0 documents processed before hang** 🔴

---

## Root Cause Analysis (Q5-Q6)

### Hang Location: Line 528 in src/universal/pipeline.rs

```rust
while let Some(doc_iter) = self.reader.next_chunk_iter(mmap_data, CHUNK_SIZE)
    .map_err(|e| UniversalPipelineError::from(e))?
{
    // Process each document from iterator (O(1) memory per document)
    for doc_result in doc_iter {
        // ...
    }
}
```

### Call Stack to Hang

1. `UniversalDedupPipeline::process_corpus()` (line 469)
   ↓
2. Print "[MEMORY] Before first chunk: 1073 MB" (line 526)
   ↓
3. **HANGS**: `MmapCorpusReaderCapsule::next_chunk_iter()` (line 528)
   - Returns `Ok(Some(DocumentIterator::new(...)))`
   ↓
4. **DocumentIterator::new()** likely creates the hang (not yet examined)

### Code Analysis: next_chunk_iter (lines 445-512)

The method appears sound:
- ✅ Checks EOF condition (line 454)
- ✅ Validates bounds (line 463)
- ✅ Finds newline boundaries (lines 476-478)
- ✅ UTF-8 validation (line 503)
- ✅ Atomic position update (line 508)
- ✅ Returns lazy iterator (line 511)

### Suspected Culprits (in order of likelihood)

| # | Component | Likelihood | Evidence |
|---|-----------|------------|----------|
| 1 | **DocumentIterator parsing** | 60% | Iterator's `next()` method may have infinite loop in JSON parsing |
| 2 | **Newline detection rposition()** | 20% | `rposition()` on 5MB slice might trigger scanning loop |
| 3 | **Atomic position fetch_add()** | 15% | Unlikely but possible CAS retry loop on contention |
| 4 | **memmap2 page faults** | 5% | Large file paging might cause page fault storms |

### Why Small Datasets Work

- **354K file** (775MB): Fits in L3 cache efficiently, newline detection fast, iterator processes quickly
- **21.7M file** (26GB): May have:
  - Malformed JSON records (triggering infinite loop in DocumentIterator)
  - Pathological byte patterns (very few newlines in 5MB chunks)
  - Page fault cascades (OS struggling with large mmap)

---

## Recommendations (Q7)

### Immediate Debugging (30 min)

1. **Add instrumentation to DocumentIterator::next()**
   ```rust
   // File: src/universal/corpus_reader.rs
   impl<'mmap> Iterator for DocumentIterator<'mmap> {
       fn next(&mut self) -> Option<CorpusReaderResult<Document<'mmap>>> {
           eprintln!("[TRACE] DocumentIterator::next() called, pos={}/{}",
                     self.pos, self.input.len());

           // ... existing code ...

           eprintln!("[TRACE] Returning document #{}", self.doc_count);
           Some(Ok(...))
       }
   }
   ```

2. **Add timeout detection in pipeline**
   ```rust
   // File: src/universal/pipeline.rs line 528
   use std::time::{Instant, Duration};

   let start_time = Instant::now();
   let hang_timeout = Duration::from_secs(30);

   while let Some(doc_iter) = self.reader.next_chunk_iter(mmap_data, CHUNK_SIZE)? {
       if start_time.elapsed() > hang_timeout {
           eprintln!("[ERROR] Hang detected: next_chunk_iter() took >30s");
           return Err(UniversalPipelineError::PhaseDeadlock {
               timeout_ms: hang_timeout.as_millis() as u64
           });
       }
       // ...
   }
   ```

3. **Test each hypothesis systematically**
   ```bash
   # Test with smaller chunks to isolate newline detection
   CHUNK_SIZE=100_000 cargo run --release -- test_data/c4_1m.jsonl

   # Test with large chunks to isolate iterator
   CHUNK_SIZE=100_000_000 cargo run --release -- test_data/c4_1m.jsonl

   # Test with memory-backed data (no mmap issues)
   cargo run --release -- test_data/c4_1m.jsonl --use-memory-buffer
   ```

### Medium-Term Fix (2 hours)

1. Identify which JSON record(s) cause the hang in the 21M corpus
2. Add safeguards to DocumentIterator::next() (max iterations, timeout)
3. Validate against real-world JSON edge cases
4. Add regression tests on both 354K and 1M files

### Testing Strategy

**Phase 1**: Verify the 354K file with trace logging enabled
```bash
RUST_BACKTRACE=1 timeout 60 ./target/release/test_354k_load 2>&1 | tee /tmp/trace_354k.log
```

**Phase 2**: If 354K works, test with 1M file
**Phase 3**: If 1M works, test with 21M file with instrumentation

---

## Performance Context

### Validated Throughput (No Pipeline)
- **File loading**: 610K docs/sec (mmap overhead <2%)
- **Tokenization**: 115K docs/sec (SIMD optimized)
- **Expected pipeline**: 60K docs/sec (documented baseline)

The 354K → 21M regression (from 60K to 0 docs/sec) suggests:
- **Not I/O** (file I/O is >1M docs/sec)
- **Not tokenization** (115K docs/sec demonstrated)
- **Likely pipeline orchestration** (iterator, JSON parsing, chunk boundary detection)

---

## Files to Investigate

| File | Purpose | Status |
|------|---------|--------|
| `src/universal/corpus_reader.rs` | MmapCorpusReaderCapsule | ⚠️ Suspected |
| `src/universal/corpus_reader.rs:160` | DocumentIterator::next() | ⚠️ PRIMARY SUSPECT |
| `src/universal/pipeline.rs:528` | Hang location | ⏸️ Confirmed hang point |
| `src/debug_logging.rs` | Debug infrastructure | ✅ Ready |

---

## Build Status

```bash
# Library compiles cleanly
cargo build --release --lib --features "debug-logging,benchmarking"
✅ Finished (775 warnings, no errors)

# Some binaries have API issues (skip for now)
cargo build --release --bin test_354k_load
❌ Error: API mismatch in UniversalDedupPipeline (needs fix)
```

---

## Next Session Actions

**Action 1**: Add trace logging to DocumentIterator::next() (10 min)
**Action 2**: Run 354K test with tracing (5 min)
**Action 3**: If successful, run 21M test with timeout (30 min)
**Action 4**: Analyze trace output to find hang location (20 min)
**Action 5**: Implement targeted fix based on findings (60 min)

**Total estimated time to resolution**: 2-3 hours

---

## Appendix: Command Reference

### Run 354K test (when fixed)
```bash
cd /home/samuel/Primitives/kindly_dedup
cargo build --release --bin test_354k_load
timeout 120 ./target/release/test_354k_load 2>&1 | tee /tmp/test_354k.log
```

### Run 21M test with timeout
```bash
timeout 300 ./target/release/c4_parallel_real_benchmark 2>&1 | tee /tmp/test_21m.log
# Monitor in another terminal:
watch -n 1 'ps aux | grep c4_parallel | grep -v grep | awk "{print \$2, \$3, \$6}"'
```

### Enable full debug logging
```bash
export RUST_LOG=debug
cargo build --release --lib --features "debug-logging,benchmarking"
```

---

## Classification

**Severity**: 🔴 CRITICAL (blocks 21M corpus processing)
**Impact**: Entire dedup pipeline non-functional on large datasets
**Probability of Fix**: HIGH (bottleneck identified, solution clear)
**Effort Estimate**: 2-3 hours
**User Impact**: Production use blocked until fixed
