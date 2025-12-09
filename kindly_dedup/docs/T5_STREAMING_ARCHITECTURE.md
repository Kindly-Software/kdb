# T5 Streaming Pipeline Architecture - kindly_dedup

**Version**: 2.0 - Pure Atomic Capsule Design (No Rayon)
**Date**: 2025-11-13
**Status**: Design Phase
**Expected Speedup**: 3-5× total (200-300K docs/sec @ 16 cores)

---

## Executive Summary

**Objective**: Replace broken ParallelDedupPipeline with T5 Streaming architecture using pure atomic_capsule primitives.

**Key Principle**: Pipeline parallelism (multiple stages running concurrently) beats data parallelism (rayon fork-join).

**Architecture**: 5-stage lockfree pipeline with work-stealing thread pool.

---

## Current State vs Target

### Current (ParallelDedupPipeline - BROKEN)

```
Documents → [Rayon Fork-Join] → [All-in-one parallel workers] → Results
              ↓
         Tokenize (BROKEN: sequential inside workers)
         MinHash (contention on Arc)
         LSH (CAS storms)
         Jaccard (scatter-gather)

Performance: 6K docs/sec (12.8× SLOWER than sequential!)
Issue: 89.5% parallelizable but only 31.3% achieved
```

### Target (T5 StreamingDedupPipeline)

```
Stage 1: Ingest          Stage 2: MinHash        Stage 3: LSH           Stage 4: Verify
Documents → Queue → Tokenize → Queue → Signatures → Queue → Buckets → Queue → Pairs → Clusters
   ↓            ↓              ↓           ↓            ↓         ↓           ↓
[Producer]  [Workers]      [Workers]   [Workers]   [Aggregator] [Merger] [Workers]

Each stage runs CONCURRENTLY (pipeline parallelism)
Each stage has DEDICATED thread pool (work stealing)
Queues are LOCKFREE (UnboundedQueueCapsule or RingBufferCapsule)

Performance: 200-300K docs/sec (3-5× sequential, 33-50× broken parallel)
Parallelism: 95%+ achievable (pipeline stages overlap)
```

---

## Stage Breakdown

### Stage 1: Document Ingest (Producer)

**Purpose**: Read raw documents and feed into pipeline.

**Input**: `Vec<(DocId, String)>` or streaming iterator
**Output**: `UnboundedQueueCapsule<(DocId, String)>`

**Architecture**:
```rust
struct IngestStage {
    output_queue: Arc<UnboundedQueueCapsule<(DocId, String)>>,
    documents_sent: AtomicUsize,
}

impl IngestStage {
    fn run(&self, documents: Vec<(DocId, String)>) {
        for (doc_id, text) in documents {
            self.output_queue.push((doc_id, text)); // <50ns lockfree push
            self.documents_sent.fetch_add(1, Ordering::Relaxed);
        }
    }
}
```

**Performance**:
- Push latency: <50ns per document (lockfree queue)
- Throughput: 20M docs/sec (I/O bound, not CPU bound)
- Threads: 1 producer (sequential, no contention)

**ASSUM Safety**:
- `#ASSUME_QUEUE_LOCKFREE`: UnboundedQueueCapsule proven lockfree
- `#VERIFY_QUEUE_LOCKFREE`: Zero mutex, 100% atomic CAS

---

### Stage 2: Tokenization (Worker Pool)

**Purpose**: Convert raw text → Vec<String> tokens.

**Input**: `UnboundedQueueCapsule<(DocId, String)>` (from Stage 1)
**Output**: `UnboundedQueueCapsule<(DocId, Vec<String>)>` (to Stage 3)

**Architecture**:
```rust
struct TokenizationStage {
    input_queue: Arc<UnboundedQueueCapsule<(DocId, String)>>,
    output_queue: Arc<UnboundedQueueCapsule<(DocId, Vec<String>)>>,
    pool: Arc<ThreadPool>,  // atomic_capsule::parallel::ThreadPool
    tokens_processed: AtomicUsize,
}

impl TokenizationStage {
    fn run(&self) {
        // Worker loop (each thread independently)
        loop {
            // 1. Pop from input queue (lockfree, <50ns)
            let Some((doc_id, text)) = self.input_queue.pop() else {
                if self.is_done() { break; }
                std::hint::spin_loop(); // Backoff
                continue;
            };

            // 2. Tokenize (CPU-bound, 10μs per doc)
            let tokens = tokenize(&text);

            // 3. Push to output queue (lockfree, <50ns)
            self.output_queue.push((doc_id, tokens));
            self.tokens_processed.fetch_add(1, Ordering::Relaxed);
        }
    }
}
```

**Performance**:
- Tokenization: 10μs per document (CPU-bound)
- Throughput: 100K docs/sec per worker
- Threads: 4 workers (contention-free, each pulls independently)
- Scalability: Linear up to 8 workers (CPU bound)

**Why 4 workers?**:
- Tokenization is 10μs (vs 2μs MinHash)
- 4 workers × 100K/sec = 400K docs/sec (enough to saturate 16 MinHash workers)
- More workers = queue contention (diminishing returns)

**Alternative: Skip This Stage**:
- Pre-tokenize sequentially BEFORE Stage 1 (simpler)
- Trade-off: No pipeline parallelism for tokenization, but saves 1 queue
- Decision: Keep Stage 2 for full pipeline parallelism

---

### Stage 3: MinHash Signature (Worker Pool)

**Purpose**: Compute MinHash signatures from tokens.

**Input**: `UnboundedQueueCapsule<(DocId, Vec<String>)>` (from Stage 2)
**Output**: `UnboundedQueueCapsule<(DocId, MinHashSignatureCapsule)>` (to Stage 4)

**Architecture**:
```rust
struct MinHashStage {
    input_queue: Arc<UnboundedQueueCapsule<(DocId, Vec<String>)>>,
    output_queue: Arc<UnboundedQueueCapsule<(DocId, MinHashSignatureCapsule)>>,
    pool: Arc<ThreadPool>,
    cpu_caps: Arc<CpuCapabilityCapsule>,  // For SIMD dispatch
    signatures_computed: AtomicUsize,
}

impl MinHashStage {
    fn run(&self) {
        loop {
            // 1. Pop tokenized document
            let Some((doc_id, tokens)) = self.input_queue.pop() else {
                if self.is_done() { break; }
                std::hint::spin_loop();
                continue;
            };

            // 2. Compute MinHash signature (SIMD if available)
            let token_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();

            #[cfg(feature = "simd-minhash")]
            let signature = if self.cpu_caps.has_avx2() {
                simd_compute_signature(&token_refs)  // 7.1× speedup
            } else {
                MinHashSignatureCapsule::compute_signature(&token_refs)
            };

            #[cfg(not(feature = "simd-minhash"))]
            let signature = MinHashSignatureCapsule::compute_signature(&token_refs);

            // 3. Push to output queue
            self.output_queue.push((doc_id, signature));
            self.signatures_computed.fetch_add(1, Ordering::Relaxed);
        }
    }
}
```

**Performance**:
- MinHash: 2μs per document (scalar) or 0.28μs (SIMD 7.1×)
- Throughput: 500K docs/sec per worker (scalar), 3.5M/sec (SIMD)
- Threads: 16 workers (embarrassingly parallel)
- Scalability: Linear up to CPU core count

**Bloom Pre-Filter Integration**:
```rust
// Before computing signature, check Bloom filter
if bloom.query_fast(doc_id, &tokens) {
    // Already seen → skip signature computation
    continue;
}

// After computing signature, insert into Bloom
bloom.insert_fast(doc_id, &tokens);
```

**Why This Stage is Critical**:
- MinHash is the CPU bottleneck (70% of time in investigation)
- Embarrassingly parallel (zero dependencies)
- SIMD accelerates 7.1× (portable_simd)
- Pipeline parallelism: Runs WHILE Stage 2 tokenizes new docs

---

### Stage 4: LSH Bucketing (Batch Aggregator)

**Purpose**: Group signatures into LSH buckets (candidate pairs).

**Input**: `UnboundedQueueCapsule<(DocId, MinHashSignatureCapsule)>` (from Stage 3)
**Output**: `HashMap<(usize, u64), Vec<DocId>>` (buckets, sequential output)

**Architecture**:
```rust
struct LshStage {
    input_queue: Arc<UnboundedQueueCapsule<(DocId, MinHashSignatureCapsule)>>,
    pool: Arc<ThreadPool>,
    num_bands: usize,  // Adaptive: 5 (small corpus) to 12 (10M docs)
    rows_per_band: usize,
}

impl LshStage {
    fn run(&self) -> HashMap<(usize, u64), Vec<DocId>> {
        // DESIGN CHOICE: Thread-local aggregation (Fix #3 pattern)

        let thread_local_buckets = Arc::new(Mutex<Vec<HashMap<(usize, u64), Vec<DocId>>>>>::new(vec![HashMap::new(); num_threads]));

        // Worker loop (batch processing)
        loop {
            // 1. Pop batch of signatures (amortize queue overhead)
            let mut batch = Vec::with_capacity(1000);
            for _ in 0..1000 {
                if let Some(item) = self.input_queue.pop() {
                    batch.push(item);
                } else {
                    break;
                }
            }

            if batch.is_empty() {
                if self.is_done() { break; }
                std::hint::spin_loop();
                continue;
            }

            // 2. Thread-local bucket aggregation (Fix #3)
            let thread_id = get_thread_id(); // ThreadPool worker ID
            let mut local_buckets = thread_local_buckets.lock()[thread_id].clone();

            for (doc_id, signature) in batch {
                for band_idx in 0..self.num_bands {
                    let band_hash = compute_band_hash(&signature, band_idx);
                    local_buckets.entry((band_idx, band_hash))
                        .or_insert_with(Vec::new)
                        .push(doc_id);
                }
            }

            thread_local_buckets.lock()[thread_id] = local_buckets;
        }

        // 3. Sequential merge (after all workers finish)
        let mut merged = HashMap::with_capacity(244_000);
        for local_map in thread_local_buckets.lock().drain(..) {
            for (key, mut docs) in local_map {
                merged.entry(key)
                    .or_insert_with(Vec::new)
                    .append(&mut docs);
            }
        }

        // 4. Deduplicate docs per bucket
        for docs in merged.values_mut() {
            docs.sort_unstable();
            docs.dedup();
        }

        merged
    }
}
```

**WAIT - This uses Mutex!**

Let me redesign without Mutex (pure atomic_capsule):

```rust
// LOCKFREE VERSION (atomic_capsule::collections::ConcurrentMapCapsule)

struct LshStage {
    input_queue: Arc<UnboundedQueueCapsule<(DocId, MinHashSignatureCapsule)>>,
    buckets: Vec<Arc<ConcurrentMapCapsule<(usize, u64), Arc<LockfreeList<DocId>>>>>,  // 16 shards
    pool: Arc<ThreadPool>,
}

impl LshStage {
    fn run(&self) {
        // Worker loop (lockfree insert)
        loop {
            let Some((doc_id, signature)) = self.input_queue.pop() else {
                if self.is_done() { break; }
                continue;
            };

            for band_idx in 0..self.num_bands {
                let band_hash = compute_band_hash(&signature, band_idx);
                let bucket_key = (band_idx, band_hash);

                // Shard selection (same as StreamingLshBucketer)
                let shard_idx = (band_hash % 16) as usize;
                let shard = &self.buckets[shard_idx];

                // Get-or-insert bucket (lockfree ConcurrentMapCapsule)
                let list = if let Some(existing) = shard.get(&bucket_key) {
                    existing.clone()  // Arc refcount
                } else {
                    let new_list = Arc::new(LockfreeList::new());
                    shard.insert(bucket_key, new_list.clone());
                    new_list
                };

                // Append to lockfree list (<50ns)
                list.push(doc_id);
            }
        }
    }

    fn extract_candidates(&self) -> Vec<(DocId, DocId)> {
        // Same as StreamingLshBucketer (O(k) extraction)
        let mut pairs = Vec::new();
        for shard in &self.buckets {
            for bucket_key in shard.keys() {
                let docs = shard.get(&bucket_key).unwrap();
                let doc_vec: Vec<DocId> = docs.iter().collect();

                // Generate pairs (n choose 2)
                for i in 0..doc_vec.len() {
                    for j in (i+1)..doc_vec.len() {
                        pairs.push((doc_vec[i].min(doc_vec[j]), doc_vec[i].max(doc_vec[j])));
                    }
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
        pairs
    }
}
```

**Performance**:
- Bucket insert: <100ns per band (16-way sharding, zero contention)
- Total per-doc: 500ns (5 bands × 100ns)
- Throughput: 2M docs/sec (bottleneck: pair generation O(n²) per bucket)
- Threads: 16 workers (lockfree, scales linearly)

**Why Lockfree Matters**:
- CAS contention was 50% of find phase (investigation Section 3)
- 16-way sharding reduces contention 16× (birthday paradox)
- LockfreeList append <50ns (vs 100ns+ with CAS retry)

---

### Stage 5: Jaccard Verification (Worker Pool)

**Purpose**: Verify candidate pairs with exact Jaccard similarity.

**Input**: `Vec<(DocId, DocId)>` (from Stage 4)
**Output**: `Vec<Vec<DocId>>` (clusters, via Union-Find)

**Architecture**:
```rust
struct VerificationStage {
    pairs: Vec<(DocId, DocId)>,
    signatures: Arc<Vec<Option<MinHashSignatureCapsule>>>,
    threshold: f64,
    pool: Arc<ThreadPool>,
}

impl VerificationStage {
    fn run(&self) -> Vec<Vec<DocId>> {
        // Parallel Jaccard verification (embarrassingly parallel)
        let verified_pairs: Vec<(DocId, DocId)> = self.pairs
            .par_chunks(1000)  // Wait, no rayon!
            .flat_map(|chunk| {
                chunk.iter()
                    .filter_map(|(doc1, doc2)| {
                        let sig1 = &self.signatures[*doc1].as_ref()?;
                        let sig2 = &self.signatures[*doc2].as_ref()?;

                        // Q16.16 deterministic Jaccard
                        let jaccard = sig1.jaccard_similarity_q16(sig2);
                        let threshold_q16 = Q16_16::from_f64(self.threshold);

                        if jaccard >= threshold_q16 {
                            Some((*doc1.min(doc2), *doc1.max(doc2)))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        // Sequential Union-Find clustering (fast O(α(n)))
        let mut uf = UnionFind::new(self.signatures.len());
        for (doc1, doc2) in verified_pairs {
            uf.union(doc1, doc2);
        }
        uf.get_clusters()
    }
}
```

**WAIT - par_chunks is rayon!**

Lockfree version:

```rust
impl VerificationStage {
    fn run(&self) -> Vec<Vec<DocId>> {
        // ThreadPool work-stealing pattern
        let chunk_size = 1000;
        let verified_queue = Arc::new(UnboundedQueueCapsule::new());

        // Spawn workers
        for chunk in self.pairs.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let verified = verified_queue.clone();
            let signatures = self.signatures.clone();
            let threshold = self.threshold;

            self.pool.execute(move || {
                for (doc1, doc2) in chunk {
                    let sig1 = &signatures[doc1].as_ref().unwrap();
                    let sig2 = &signatures[doc2].as_ref().unwrap();

                    let jaccard = sig1.jaccard_similarity_q16(sig2);
                    let threshold_q16 = Q16_16::from_f64(threshold);

                    if jaccard >= threshold_q16 {
                        verified.push((doc1.min(doc2), doc1.max(doc2)));
                    }
                }
            });
        }

        // Wait for completion, collect results
        self.pool.wait_all();

        let mut verified_pairs: Vec<(DocId, DocId)> = Vec::new();
        while let Some(pair) = verified_queue.pop() {
            verified_pairs.push(pair);
        }
        verified_pairs.sort_unstable();
        verified_pairs.dedup();

        // Union-Find clustering (sequential, fast)
        let mut uf = UnionFind::new(self.signatures.len());
        for (doc1, doc2) in verified_pairs {
            uf.union(doc1, doc2);
        }
        uf.get_clusters()
    }
}
```

**Performance**:
- Jaccard: <1μs per pair (Q16.16 deterministic)
- Throughput: 1M pairs/sec per worker
- Threads: 16 workers (embarrassingly parallel)
- Union-Find: O(α(n)) ≈ O(1), <1ms for 100K pairs

---

## Full Pipeline Integration

```rust
pub struct StreamingDedupPipeline {
    // Stage 1: Ingest
    ingest_queue: Arc<UnboundedQueueCapsule<(DocId, String)>>,

    // Stage 2: Tokenization
    token_queue: Arc<UnboundedQueueCapsule<(DocId, Vec<String>)>>,
    tokenization_pool: Arc<ThreadPool>,

    // Stage 3: MinHash
    signature_queue: Arc<UnboundedQueueCapsule<(DocId, MinHashSignatureCapsule)>>,
    minhash_pool: Arc<ThreadPool>,

    // Stage 4: LSH
    lsh_buckets: Vec<Arc<ConcurrentMapCapsule<(usize, u64), Arc<LockfreeList<DocId>>>>>,
    lsh_pool: Arc<ThreadPool>,

    // Stage 5: Verification
    verification_pool: Arc<ThreadPool>,

    // Shared state
    signatures: Arc<Vec<Option<MinHashSignatureCapsule>>>,
    cpu_caps: Arc<CpuCapabilityCapsule>,
    bloom: Arc<ShardedDedupBloomFilter>,

    // Metrics
    documents_ingested: AtomicUsize,
    documents_tokenized: AtomicUsize,
    signatures_computed: AtomicUsize,
    pairs_verified: AtomicUsize,
}

impl StreamingDedupPipeline {
    pub fn new(num_documents: usize, num_threads: usize) -> Self {
        // Thread pool allocation strategy
        let tokenization_threads = 4;   // CPU-bound, 10μs per doc
        let minhash_threads = 16;       // Embarrassingly parallel, 2μs per doc
        let lsh_threads = 16;           // Lockfree insert, <100ns per band
        let verification_threads = 16;  // Embarrassingly parallel, <1μs per pair

        Self {
            ingest_queue: Arc::new(UnboundedQueueCapsule::new()),
            token_queue: Arc::new(UnboundedQueueCapsule::new()),
            signature_queue: Arc::new(UnboundedQueueCapsule::new()),

            tokenization_pool: Arc::new(ThreadPool::new(tokenization_threads).unwrap()),
            minhash_pool: Arc::new(ThreadPool::new(minhash_threads).unwrap()),
            lsh_pool: Arc::new(ThreadPool::new(lsh_threads).unwrap()),
            verification_pool: Arc::new(ThreadPool::new(verification_threads).unwrap()),

            lsh_buckets: (0..16).map(|_| Arc::new(ConcurrentMapCapsule::new())).collect(),

            signatures: Arc::new(vec![None; num_documents]),
            cpu_caps: Arc::new(CpuCapabilityCapsule::detect()),
            bloom: Arc::new(ShardedDedupBloomFilter::new(num_documents)),

            documents_ingested: AtomicUsize::new(0),
            documents_tokenized: AtomicUsize::new(0),
            signatures_computed: AtomicUsize::new(0),
            pairs_verified: AtomicUsize::new(0),
        }
    }

    pub fn add_documents(&mut self, documents: Vec<(DocId, String)>) {
        // Stage 1: Ingest (single producer)
        for (doc_id, text) in documents {
            self.ingest_queue.push((doc_id, text));
            self.documents_ingested.fetch_add(1, Ordering::Relaxed);
        }

        // Launch Stage 2: Tokenization workers
        for _ in 0..4 {
            let ingest_q = self.ingest_queue.clone();
            let token_q = self.token_queue.clone();
            let counter = Arc::new(self.documents_tokenized.clone());

            self.tokenization_pool.execute(move || {
                loop {
                    let Some((doc_id, text)) = ingest_q.pop() else {
                        break;
                    };
                    let tokens = tokenize(&text);
                    token_q.push((doc_id, tokens));
                    counter.fetch_add(1, Ordering::Relaxed);
                }
            });
        }

        // Launch Stage 3: MinHash workers
        for _ in 0..16 {
            let token_q = self.token_queue.clone();
            let sig_q = self.signature_queue.clone();
            let cpu_caps = self.cpu_caps.clone();
            let counter = Arc::new(self.signatures_computed.clone());

            self.minhash_pool.execute(move || {
                loop {
                    let Some((doc_id, tokens)) = token_q.pop() else {
                        break;
                    };

                    let token_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
                    let signature = MinHashSignatureCapsule::compute_signature(&token_refs);

                    sig_q.push((doc_id, signature));
                    counter.fetch_add(1, Ordering::Relaxed);
                }
            });
        }

        // Launch Stage 4: LSH workers
        for _ in 0..16 {
            let sig_q = self.signature_queue.clone();
            let buckets = self.lsh_buckets.clone();

            self.lsh_pool.execute(move || {
                loop {
                    let Some((doc_id, signature)) = sig_q.pop() else {
                        break;
                    };

                    for band_idx in 0..NUM_BANDS {
                        let band_hash = compute_band_hash(&signature, band_idx);
                        let shard_idx = (band_hash % 16) as usize;
                        let shard = &buckets[shard_idx];

                        let list = shard.get_or_insert((band_idx, band_hash), || {
                            Arc::new(LockfreeList::new())
                        });

                        list.push(doc_id);
                    }
                }
            });
        }

        // Wait for all stages to complete
        self.tokenization_pool.wait_all();
        self.minhash_pool.wait_all();
        self.lsh_pool.wait_all();
    }

    pub fn find_duplicates(&self, threshold: f64) -> Vec<Vec<DocId>> {
        // Extract candidate pairs from LSH buckets (sequential)
        let pairs = self.extract_candidate_pairs();

        // Stage 5: Parallel Jaccard verification
        let verified_queue = Arc::new(UnboundedQueueCapsule::new());

        for chunk in pairs.chunks(1000) {
            let chunk = chunk.to_vec();
            let verified = verified_queue.clone();
            let signatures = self.signatures.clone();
            let threshold_q16 = Q16_16::from_f64(threshold);

            self.verification_pool.execute(move || {
                for (doc1, doc2) in chunk {
                    let sig1 = &signatures[doc1].as_ref().unwrap();
                    let sig2 = &signatures[doc2].as_ref().unwrap();

                    if sig1.jaccard_similarity_q16(sig2) >= threshold_q16 {
                        verified.push((doc1, doc2));
                    }
                }
            });
        }

        self.verification_pool.wait_all();

        // Collect verified pairs
        let mut verified_pairs = Vec::new();
        while let Some(pair) = verified_queue.pop() {
            verified_pairs.push(pair);
        }
        verified_pairs.sort_unstable();
        verified_pairs.dedup();

        // Union-Find clustering (sequential)
        let mut uf = UnionFind::new(self.signatures.len());
        for (doc1, doc2) in verified_pairs {
            uf.union(doc1, doc2);
        }
        uf.get_clusters()
    }

    fn extract_candidate_pairs(&self) -> Vec<(DocId, DocId)> {
        let mut pairs = Vec::new();
        for shard in &self.lsh_buckets {
            for bucket_key in shard.keys() {
                let docs_list = shard.get(&bucket_key).unwrap();
                let docs: Vec<DocId> = docs_list.iter().collect();

                for i in 0..docs.len() {
                    for j in (i+1)..docs.len() {
                        pairs.push((docs[i].min(docs[j]), docs[i].max(docs[j])));
                    }
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
        pairs
    }
}
```

---

## Performance Analysis

### Amdahl's Law Breakdown

**Current ParallelDedupPipeline** (broken):
- Sequential: 81.6% (tokenization inside workers)
- Parallel: 18.4%
- Max speedup: 2.1× @ 16 cores
- **Actual**: 0.19× (regression)

**T5 StreamingDedupPipeline** (design):
- Sequential: 10% (pair generation, Union-Find)
- Parallel: 90% (all 5 stages overlap)
- Max speedup: 9.1× @ 16 cores
- **Expected**: 6-8× (accounting for queue overhead)

### Throughput Projection (16 cores)

| Stage | Latency | Workers | Throughput | Bottleneck? |
|-------|---------|---------|------------|-------------|
| **Ingest** | 50ns | 1 | 20M/sec | ❌ No |
| **Tokenize** | 10μs | 4 | 400K/sec | ❌ No |
| **MinHash** | 2μs (scalar) | 16 | 8M/sec | ❌ No |
| **MinHash** | 0.28μs (SIMD) | 16 | 57M/sec | ❌ No |
| **LSH** | 500ns | 16 | 32M/sec | ❌ No |
| **Verify** | 1μs | 16 | 16M/sec | ❌ No |
| **Pipeline** | - | - | **200-300K/sec** | ✅ **Pipeline overhead** |

**Bottleneck**: Pipeline coordination overhead (queue push/pop, thread sync).

**Realistic expectation**:
- Best case: 300K docs/sec (5× sequential)
- Typical: 200K docs/sec (3.3× sequential)
- Worst case: 100K docs/sec (1.67× sequential, still better than broken parallel!)

### Comparison Table

| Implementation | Throughput | Speedup | Parallelism | Status |
|----------------|------------|---------|-------------|--------|
| **DedupPipeline** (sequential) | 60K/sec | 1× | 0% | ✅ Production |
| **ParallelDedupPipeline** (broken) | 6K/sec | 0.1× | 31.3% | ❌ Broken |
| **ParallelDedupPipeline** (Fix #1-#3) | 85-100K/sec | 1.4-1.7× | 89.5% | 🟡 Amdahl-limited |
| **StreamingDedupPipeline** (T5) | 200-300K/sec | 3.3-5× | 90%+ | 🟢 Target |

---

## Implementation Plan

### Phase 1: Core Pipeline (Week 1)
1. ✅ Design architecture (this document)
2. ⏳ Implement IngestStage (1 hour)
3. ⏳ Implement TokenizationStage (2 hours)
4. ⏳ Implement MinHashStage (3 hours, SIMD integration)
5. ⏳ Implement LshStage (4 hours, lockfree buckets)
6. ⏳ Implement VerificationStage (2 hours, Union-Find)
7. ⏳ Integration testing (4 hours)

**Total**: ~16 hours (2 days)

### Phase 2: Optimization (Week 2)
1. ⏳ Bloom pre-filter integration (2 hours)
2. ⏳ Adaptive LSH parameters (2 hours)
3. ⏳ Queue batching (amortize push/pop) (3 hours)
4. ⏳ Thread affinity (cache-aware scheduling) (2 hours)
5. ⏳ Benchmarking (B32 validation) (4 hours)

**Total**: ~13 hours (1.5 days)

### Phase 3: Production Hardening (Week 3)
1. ⏳ Error handling (queue full, worker panic) (3 hours)
2. ⏳ Graceful shutdown (drain queues) (2 hours)
3. ⏳ Progress tracking (real-time metrics) (2 hours)
4. ⏳ Q34 audit trail integration (2 hours)
5. ⏳ T28 comprehensive testing (8 hours)

**Total**: ~17 hours (2 days)

### Total Timeline: 2-3 weeks

---

## Trade-Offs vs Rayon

### Rayon (Rejected)
✅ Simple API (fold/reduce)
✅ Work stealing built-in
❌ Not Chaos compliant (uses Mutex internally)
❌ Fork-join model (not pipeline)
❌ Hidden overhead (thread sync)
❌ Hard to debug performance

### atomic_capsule T5 (Chosen)
✅ 100% lockfree (Chaos compliant)
✅ Pipeline parallelism (stages overlap)
✅ Explicit control (queues, thread pools)
✅ Observable metrics (queue depth, worker throughput)
❌ More complex implementation
❌ Manual queue management

**Decision**: T5 Streaming aligns with Chaos mandate and provides 2-3× better parallelism (90% vs Amdahl's 89.5% limit with rayon).

---

## ASSUM Safety Tags

### Queue Safety
- `#ASSUME_QUEUE_LOCKFREE`: UnboundedQueueCapsule uses CAS only
- `#VERIFY_QUEUE_LOCKFREE`: grep -r "Mutex" atomic_capsule/src/collections/queue → 0 results

### Thread Pool Safety
- `#ASSUME_POOL_WORK_STEALING`: ThreadPool balances load
- `#VERIFY_POOL_WORK_STEALING`: atomic_capsule::parallel::ThreadPool proven lockfree

### Stage Coordination Safety
- `#ASSUME_STAGE_ISOLATION`: Each stage has dedicated workers
- `#VERIFY_STAGE_ISOLATION`: No shared state between stages (only queues)

### Termination Safety
- `#ASSUME_QUEUE_DRAIN`: Workers check is_done() + queue empty
- `#VERIFY_QUEUE_DRAIN`: wait_all() blocks until workers finish

---

## Success Criteria (B32)

1. **Throughput**: ≥200K docs/sec @ 16 cores (3.3× sequential)
2. **Latency**: ≤5μs per document (end-to-end pipeline)
3. **Parallelism**: ≥90% (Amdahl's Law)
4. **Recall**: ≥92% (LSH band-based, unchanged from baseline)
5. **Safety**: 99.99% ASSUM safe (all assumptions verified)
6. **Reproducibility**: 100% (Q16.16 deterministic Jaccard)

---

## Next Steps

1. ✅ Design complete (this document)
2. ⏳ Revert rayon code to atomic_capsule primitives (Fix #1-#3 redo)
3. ⏳ Implement StreamingDedupPipeline (Phase 1, Week 1)
4. ⏳ Benchmark vs sequential baseline (B32 validation)
5. ⏳ Compare vs broken ParallelDedupPipeline (prove 33-50× improvement)

---

**Status**: Ready for implementation
**Approval**: Pending user review
**Risk**: Low (all primitives proven in atomic_capsule Phase 5)
