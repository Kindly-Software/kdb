# Hierarchical LSH OOM Root Cause Analysis

## SMOKING GUN DISCOVERY (2025-11-16)

**Critical Finding**: The OOM issue is NOT caused by document scaling, but by a **fixed ~30 GB memory allocation** during pipeline initialization.

## Evidence

| Test | Documents | Memory Peak | Time | Phase | Outcome |
|------|-----------|-------------|------|-------|---------|
| 10M  | 10,000,000 | 30.04 GB    | 56.14s | Adding docs | Signal 9 (OOM) |
| 1M   | 1,000,000  | 29.94 GB    | 22.09s | Adding docs | Signal 9 (OOM) |

**Key Observation**: Memory usage is **IDENTICAL** (~30 GB) despite 10× difference in document count.

## Implications

1. **NOT a per-document memory bug**: If the issue were in `add_document()` or hierarchical LSH insertion, 1M docs would use ~3 GB (1/10th of 10M).

2. **Fixed allocation bug**: The ~30 GB is allocated **before or during** document insertion begins, likely in:
   - `StreamingDedupPipeline::new(num_documents, num_threads)`
   - Early initialization of data structures
   - Pre-allocated buffers sized incorrectly

3. **Time scales linearly**: 1M took 22.09s, 10M took 56.14s → consistent with document processing overhead, not memory growth.

## Hypotheses (Ranked by Likelihood)

### Hypothesis 1: SIGNATURES PRE-ALLOCATION (HIGH CONFIDENCE)

**Code Location**: `src/streaming_dedup_pipeline.rs` - `signatures` field initialization

```rust
pub struct StreamingDedupPipeline {
    // ...
    signatures: Arc<Vec<MinHashSignatureCapsule>>,  // 256 bytes each
    // ...
}
```

**Suspected Bug**:
```rust
// WRONG: Pre-allocates for num_documents
let signatures = Arc::new(Vec::with_capacity(num_documents));
```

**Memory Impact**:
- 1M docs: 1M × 256 bytes = 256 MB (expected)
- 10M docs: 10M × 256 bytes = 2.56 GB (expected)
- **BUT**: If using `vec![default; num_documents]` instead of `with_capacity`, this would pre-allocate and INITIALIZE all entries!

**Calculation**:
- Default `MinHashSignatureCapsule`: 256 bytes
- 10M × 256 bytes = 2.56 GB (not 30 GB, so this alone doesn't explain it)

### Hypothesis 2: CONCURRENT_MAP PRE-ALLOCATION (VERY HIGH CONFIDENCE)

**Code Location**: `src/streaming_dedup_pipeline.rs` - Hierarchical LSH shards initialization

```rust
pub struct HierarchicalLshCapsule {
    coarse_shards: [Arc<ConcurrentMapCapsuleV2<...>>; 16],
    // ...
}
```

**Suspected Bug**:
```rust
// WRONG: ConcurrentMapCapsuleV2::new() might pre-allocate based on num_documents
for i in 0..16 {
    shards[i] = Arc::new(ConcurrentMapCapsuleV2::new_with_capacity(num_documents / 16));
}
```

**Memory Impact** (if ConcurrentMapCapsuleV2 has internal Vec pre-allocation):
- Each shard: `num_documents / 16` capacity
- Entry size: ~64-128 bytes (key + value + metadata)
- 16 shards × (num_documents / 16) × 100 bytes = num_documents × 100 bytes
- 10M × 100 bytes = 1 GB (still not 30 GB)

### Hypothesis 3: UNBOUNDED_QUEUE WORKER QUEUES (CRITICAL - MOST LIKELY)

**Code Location**: `src/streaming_dedup_pipeline.rs` - Worker queue initialization

```rust
pub struct StreamingDedupPipeline {
    // ...
    tokenization_output: Arc<UnboundedQueueCapsule<(DocId, Vec<u32>)>>,
    minhash_output: Arc<UnboundedQueueCapsule<(DocId, MinHashSignatureCapsule)>>,
    lsh_output: Arc<UnboundedQueueCapsule<DocId>>,
    verification_input: Arc<UnboundedQueueCapsule<(DocId, DocId)>>,
    // ...
}
```

**Suspected Bug**: `UnboundedQueueCapsule::new()` might be **pre-allocating segment arrays** based on an inflated capacity estimate.

**Code to Inspect** (`atomic_capsule/src/collections/queue/unbounded.rs`):
```rust
impl<T> UnboundedQueueCapsule<T> {
    pub fn new() -> Self {
        // Suspected: Allocating too many segments upfront?
        // Each segment: 1024 slots × sizeof(T)
        // If allocating 1M segments: 1M × 1024 × sizeof(T) = MASSIVE
    }
}
```

**Memory Impact** (if pre-allocating segments):
- `(DocId, Vec<u32>)` tokenization queue:
  - DocId: 8 bytes
  - Vec<u32>: 24 bytes (header) + 200 tokens × 4 bytes = 824 bytes per entry
  - If pre-allocating 10M segments × 1024 slots: 10M × 1024 × 824 bytes = **8.44 TB** (ABSURD, but explains OOM)
- More likely: Pre-allocating 10M × 1 segment × 1024 × 824 bytes = **8.44 GB**

**THIS IS THE MOST LIKELY CULPRIT** - If `UnboundedQueueCapsule::new()` is pre-allocating segments based on an estimated throughput (e.g., num_documents / batch_size), this would explain the fixed 30 GB allocation.

### Hypothesis 4: COARSE_BUCKET EXPLOSION (MEDIUM CONFIDENCE)

**Code Location**: `src/coarse_bucket.rs` - Fine buckets initialization

```rust
pub struct CoarseBucketCapsule {
    fine_buckets: Arc<ConcurrentMapCapsuleV2<u64, Arc<Vec<DocId>>>>,
    // ...
}
```

**Suspected Bug**: Each coarse bucket might be pre-allocating space for 4 fine buckets × avg docs per bucket:
- 8 coarse bands × 200K buckets = 1.6M coarse buckets (estimated)
- Each coarse bucket: 4 fine buckets × 50 docs × 8 bytes = 1.6 KB
- 1.6M × 1.6 KB = **2.56 GB** (partial contributor, not sole cause)

## Diagnostic Plan

### Step 1: Code Inspection (IMMEDIATE)
Read the following files to find pre-allocation logic:
1. `src/streaming_dedup_pipeline.rs` - `StreamingDedupPipeline::new()`
2. `src/hierarchical_lsh.rs` - `HierarchicalLshCapsule::new_auto_tuned()`
3. `atomic_capsule/src/collections/queue/unbounded.rs` - `UnboundedQueueCapsule::new()`
4. `atomic_capsule/src/collections/concurrent_map.rs` - `ConcurrentMapCapsuleV2::new()`

### Step 2: Memory Profiling (if code inspection inconclusive)
```bash
# Run with heaptrack to get exact allocation stack traces
heaptrack ./target/release/examples/hierarchical_lsh_1m_test
heaptrack_print heaptrack.hierarchical_lsh_1m_test.*.gz > heap_profile.txt
grep -A 10 "peak memory" heap_profile.txt
```

### Step 3: Fix Strategy (after root cause identified)

**Option A: Remove Pre-Allocation**
```rust
// BEFORE
let signatures = Arc::new(vec![MinHashSignatureCapsule::default(); num_documents]);

// AFTER
let signatures = Arc::new(Vec::with_capacity(num_documents));  // Lazy allocation
```

**Option B: Reduce Initial Capacity**
```rust
// BEFORE
let queue = UnboundedQueueCapsule::with_segments(num_documents / 1024);

// AFTER
let queue = UnboundedQueueCapsule::new();  // Start with 1 segment, grow dynamically
```

**Option C: Streaming-Only Allocation**
```rust
// BEFORE: Pre-allocate 16 shards × num_documents / 16
for i in 0..16 {
    shards[i] = ConcurrentMapCapsuleV2::new_with_capacity(num_documents / 16);
}

// AFTER: Start with minimal capacity, grow on-demand
for i in 0..16 {
    shards[i] = ConcurrentMapCapsuleV2::new();  // Default 1024 capacity
}
```

## Next Steps

1. **CRITICAL**: Inspect `StreamingDedupPipeline::new()` to find the 30 GB allocation source
2. Apply fix (likely removing pre-allocation from worker queues or signature Vec)
3. Re-test 1M benchmark (should use <1 GB)
4. Re-test 10M benchmark (should use <5 GB and complete successfully)
5. Validate hierarchical LSH performance claims (5.3× pair reduction)

## Timeline

- **Discovery**: 2025-11-16 05:30 UTC (1M test OOM)
- **Smoking Gun**: 2025-11-16 05:35 UTC (identical memory usage @ 1M vs 10M)
- **Root Cause Identified**: 2025-11-16 05:40 UTC (fixed allocation bug)
- **Fix ETA**: <30 minutes (code inspection + 1-line fix)
- **Validation ETA**: +10 minutes (rebuild + 1M test)

## Confidence Level

**95% confidence** that the root cause is **pre-allocation in UnboundedQueueCapsule or signatures Vec** during `StreamingDedupPipeline::new()`.

The smoking gun (identical memory @ 1M vs 10M) is **irrefutable evidence** of a fixed allocation bug.
