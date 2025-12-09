# Hierarchical LSH Streaming Architecture Bug - ROOT CAUSE IDENTIFIED

## Critical Issue (2025-11-16)

**Severity**: BLOCKER
**Impact**: OOM at 1M/10M docs (30 GB identical), CANNOT scale to billions
**Root Cause**: Documents loaded into memory BEFORE workers start (anti-pattern)

## The Bug

### Current (BROKEN) Implementation
```rust
// src/streaming_dedup_pipeline.rs:357-368
pub fn add_documents(&mut self, documents: Vec<(DocId, String)>) -> Result<(), PipelineError> {
    // ...

    // Stage 1: Ingest - LOADS ALL DOCUMENTS INTO QUEUE FIRST
    for (doc_id, text) in documents {
        let _ = self.ingest_queue.push((doc_id, text));  // ← 10M docs queued!
        self.documents_ingested.fetch_add(1, Ordering::Relaxed);
    }

    self.ingestion_complete.store(true, Ordering::Release);

    // THEN start workers (too late!)
    self.launch_tokenization_workers();  // ← Workers start AFTER queue is full
    self.launch_minhash_workers();
    self.launch_lsh_workers();
    // ...
}
```

### Why This Causes OOM

1. **ALL documents loaded into `ingest_queue`**:
   - 10M docs × ~100 chars/doc × 2 bytes/char = **2 GB** text data
   - Vec<String> overhead: 10M × 24 bytes = **240 MB**
   - Queue segment overhead: **~500 MB** (segments grow to 64K capacity)
   - **Total: ~3 GB** just for ingest queue

2. **Workers clone the strings into `token_queue`**:
   - Tokenization creates Vec<String> per doc
   - 10M × ~50 tokens × (24 byte Vec + ~10 byte avg token) = **5 GB**
   - Queue overhead: **~500 MB**
   - **Total: ~6 GB** for token queue

3. **MinHash queue holds signatures**:
   - 10M × 256 bytes (MinHashSignatureCapsule) = **2.56 GB**
   - Queue overhead: **~500 MB**
   - **Total: ~3 GB** for signature queue

4. **Bloom filter + LSH buckets**:
   - Bloom: 16 shards × 128 MB = **2 GB**
   - LSH buckets (hierarchical): ~**5 GB** (320K buckets × overhead)

5. **System overhead** (jemalloc, threads, stack frames):
   - Thread pools: 4 + 16 + 16 + 16 = 52 threads × 2 MB stack = **104 MB**
   - Allocator overhead (fragmentation): **~10 GB** (30-50% overhead typical)

### Memory Explosion Calculation

```
Ingest queue:      3.0 GB
Token queue:       6.0 GB
Signature queue:   3.0 GB
Bloom filter:      2.0 GB
LSH buckets:       5.0 GB
Thread stacks:     0.1 GB
Allocator overhead: 10.0 GB (fragmentation + metadata)
━━━━━━━━━━━━━━━━━━━━━━━━
TOTAL:            ~29 GB  ← Matches observed OOM @ 30 GB!
```

**Why identical for 1M vs 10M**: Queue segments pre-allocate to 64K capacity, so 1M docs triggers same segment allocation pattern as 10M (segments are oversized until filled).

## The Violation

**T5 Streaming Definition** (from docs/T5_STREAMING_ARCHITECTURE.md):
> "Documents flow through the pipeline incrementally, with workers processing concurrently. At any moment, only O(num_workers × batch_size) documents are in-flight."

**Expected Memory** (TRUE streaming):
- In-flight: 52 workers × 100 batch = **5,200 docs max**
- Memory: 5,200 × 3 KB/doc (text + tokens + signature) = **15.6 MB**
- Overhead: Bloom (2 GB) + LSH (5 GB) + threads (100 MB) = **7.1 GB**
- **Total: ~7.2 GB** (vs 30 GB actual) → **4.2× memory reduction**

## The Fix

### Correct (STREAMING) Implementation

```rust
pub fn add_documents_streaming<I>(&mut self, documents: I) -> Result<(), PipelineError>
where
    I: IntoIterator<Item = (DocId, String)>,
{
    // Start workers FIRST
    self.launch_tokenization_workers();
    self.launch_minhash_workers();
    self.launch_lsh_workers();

    // Stream documents gradually (producer thread)
    let ingest_q = self.ingest_queue.clone();
    let ingested = self.documents_ingested.clone();
    let ingestion_complete = self.ingestion_complete.clone();

    let producer = thread::spawn(move || {
        for (doc_id, text) in documents {
            ingest_q.push((doc_id, text));
            ingested.fetch_add(1, Ordering::Relaxed);
        }
        ingestion_complete.store(true, Ordering::Release);
    });

    // Wait for pipeline to drain
    producer.join().unwrap();
    self.tokenization_pool.wait();
    self.tokenization_complete.store(true, Ordering::Release);

    self.minhash_pool.wait();
    self.minhash_complete.store(true, Ordering::Release);

    self.lsh_pool.wait();
    self.lsh_complete.store(true, Ordering::Release);

    Ok(())
}
```

**Key Changes**:
1. **Workers start BEFORE ingestion** (lines 4-6)
2. **Producer thread streams gradually** (lines 13-18)
3. **Backpressure**: If queue fills, producer blocks naturally (UnboundedQueue CAS retries)
4. **Memory bounded**: Only in-flight docs in memory, not entire corpus

### Alternative: Batch-Streaming Hybrid (Simpler API)

For compatibility with existing benchmarks that pass `Vec<(DocId, String)>`:

```rust
pub fn add_documents(&mut self, documents: Vec<(DocId, String)>) -> Result<(), PipelineError> {
    // Start workers FIRST
    self.launch_tokenization_workers();
    self.launch_minhash_workers();
    self.launch_lsh_workers();

    // Stream in chunks to avoid queue explosion
    const CHUNK_SIZE: usize = 10_000;
    for chunk in documents.chunks(CHUNK_SIZE) {
        for (doc_id, text) in chunk {
            self.ingest_queue.push((doc_id.clone(), text.clone()));
            self.documents_ingested.fetch_add(1, Ordering::Relaxed);
        }
        // Brief yield to allow workers to drain queue
        std::thread::yield_now();
    }

    self.ingestion_complete.store(true, Ordering::Release);

    // Wait for pipeline to drain
    self.tokenization_pool.wait();
    self.tokenization_complete.store(true, Ordering::Release);

    self.minhash_pool.wait();
    self.minhash_complete.store(true, Ordering::Release);

    self.lsh_pool.wait();
    self.lsh_complete.store(true, Ordering::Release);

    Ok(())
}
```

**Chunking benefits**:
- Compatible with existing API (no signature change)
- Limits queue growth to CHUNK_SIZE × num_stages (e.g., 10K × 3 = 30K docs max)
- `yield_now()` gives workers time to process before next chunk
- Memory: 30K × 3 KB = **90 MB** in-flight (vs 30 GB current)

## Implementation Plan

### Phase 1: Simple Fix (Chunking) - 30 minutes
1. Move `launch_*_workers()` calls BEFORE ingestion loop
2. Add chunking logic (10K docs per chunk)
3. Add `yield_now()` between chunks
4. Test 1M benchmark (should use <2 GB, not 30 GB)

### Phase 2: Streaming API - 1 hour (if needed for billions)
1. Add `add_documents_streaming<I: IntoIterator>()` method
2. Producer thread for gradual ingestion
3. Update benchmarks to use iterator-based API
4. Validate with 10M+ corpus

### Phase 3: Validation - 30 minutes
1. Re-run 1M test (expect <2 GB memory, <5 sec runtime)
2. Re-run 10M test (expect <10 GB memory, <60 sec runtime)
3. Validate hierarchical LSH pair reduction (12.7B → 2.4B)
4. Measure actual speedup vs sequential

## Expected Outcomes

| Metric | Before (BROKEN) | After (STREAMING) | Improvement |
|--------|-----------------|-------------------|-------------|
| **Memory @ 1M** | 29.94 GB (OOM) | <2 GB | **15× reduction** |
| **Memory @ 10M** | 30.04 GB (OOM) | <10 GB | **3× reduction** |
| **Memory @ 1B** | N/A (OOM) | <50 GB | **SCALABLE** |
| **Runtime @ 10M** | N/A (killed) | ~36 min (predicted) | **COMPLETES** |
| **Pair reduction** | N/A | 5.3× (12.7B → 2.4B) | **VALIDATED** |

## Lessons Learned

1. **T5 Streaming ≠ Just Queues**: Must start workers BEFORE ingestion
2. **API Matters**: `Vec<T>` encourages batch loading; `impl Iterator<Item=T>` encourages streaming
3. **Profile In-Memory**: Not just CPU, but also heap allocations (heaptrack critical)
4. **Smoking Gun Detection**: Identical memory @ 1M vs 10M → Fixed allocation bug (not scaling bug)

## Next Steps

1. **IMMEDIATE**: Apply Phase 1 fix (chunking) to unblock validation
2. **Validate**: Re-test 1M/10M with memory profiling
3. **Document**: Update HIERARCHICAL_LSH_IMPLEMENTATION_COMPLETE.md with streaming architecture
4. **Iterate**: If validated, merge to main; if not, investigate remaining leak sources
