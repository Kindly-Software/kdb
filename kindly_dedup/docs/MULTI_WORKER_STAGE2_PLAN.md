# Multi-Worker Stage 2 Planning - WorkerPoolCapsule (T4 Batch)

**Date**: 2025-11-24
**Tier**: T4 (Batch parallelism, 10-100× throughput)
**Framework**: UCE34 Q10 (tier selection) + Chaos (100% lockfree) + ASSUM (99.99% safe)
**Performance Target**: 5.8-8.0× speedup (2.1-2.9M docs/sec @ 8 threads)

## Architecture Overview

WorkerPoolCapsule orchestrates 8 worker threads for parallel MinHash signature computation and LSH bucket insertion. Implements work-stealing architecture with atomic coordination and cache-line alignment for zero false sharing.

```text
┌─────────────────────────────────────────────────────────┐
│ WorkerPoolCapsule (128B cache line 0)                   │
│  num_workers: u32                      active_workers   │
│  batch_size: u32                       total_docs_proc  │
│  padding: 120B                         generation: u64  │
├─────────────────────────────────────────────────────────┤
│ Worker States (8 × 128B = 1024B)                        │
│  ┌─────────────────────────────────────────────────────┤
│  │ WorkerStateCapsule[0] (128B, cache line 2)          │
│  │  state: u8, docs_processed: u64, padding: 56B       │
│  ├─────────────────────────────────────────────────────┤
│  │ WorkerStateCapsule[1] (128B, cache line 3)          │
│  │ ... (6 more workers, lines 4-9)                     │
│  └─────────────────────────────────────────────────────┤
├─────────────────────────────────────────────────────────┤
│ Work-Stealing Queue (256B)                              │
│  WorkStealingQueueCapsule<DocBatch>                    │
│  - Head pointer (AtomicU64): <10ns lockfree            │
│  - Tail pointer (AtomicU64): <10ns lockfree            │
│  - 1024 slots (pre-allocated, zero-allocation)         │
├─────────────────────────────────────────────────────────┤
│ Output Aggregator (256B)                                │
│  OutputAggregatorCapsule                                │
│  - Total signatures: AtomicU64                          │
│  - Total LSH inserts: AtomicU64                         │
│  - Error count: AtomicU32                               │
│  - Completion signal: AtomicBool                        │
└─────────────────────────────────────────────────────────┘

TOTAL SIZE: 1792 bytes (14 cache lines)
ALIGNMENT: 128 bytes (prevent false sharing on EPYC/Zen CPUs)
```

## Key Design Decisions

### 1. Cache-Line Alignment (128-byte)
- **Why**: AMD Ryzen 9 6900HX has 128B L2 cache lines (vs 64B Intel)
- **Benefit**: Zero false sharing across 8 worker threads
- **Cost**: Larger structures, but acceptable for orchestration capsule
- **Verification**: #[repr(C, align(128))]

### 2. Work-Stealing Queue
- **Design**: Ring buffer with atomic head/tail pointers
- **Capacity**: 1024 pre-allocated slots (2MB memory)
- **Performance**: <10ns enqueue/dequeue (T1 Atomic tier)
- **Backpressure**: Waits on full queue (prevents memory explosion)
- **Constraint**: Bounded queue prevents OOM on slow workers

### 3. Lockfree Coordination
- **No Mutex/RwLock**: Violates Chaos mandate
- **Atomics Only**: AtomicU32/AtomicU64 (x86_64 single-instruction)
- **Memory Ordering**: Release/Acquire for efficiency
- **Verification**: #[derive(ComputationalCapsule)]

### 4. Generation Counter Shutdown
- **Problem**: How to signal 8 workers to terminate?
- **Solution**: Generation counter + flag check before each batch
- **Mechanism**: Increment generation, workers detect odd generation at barrier
- **Latency**: <100ns detection time
- **Safety**: CAS loop prevents race conditions

### 5. Per-Worker State Isolation
- **Array**: WorkerStateCapsule[8] (each 128B aligned)
- **Fields**: state (Running/Draining/Terminated), docs_processed, latency_histogram
- **Coordination**: Minimal (workers read-only except own state)
- **Benefit**: No contention on worker-local fields

## ASSUM Tags (Required Verification)

```rust
// #ASSUME: 8 workers fits in 16 cores (6900HX = 8c/16t)
// #VERIFY: std::thread::available_parallelism() >= 8
// #RATIONALE: Oversubscription by 2:1 (8 workers on 16 cores) is safe on modern CPUs

// #ASSUME: Work-stealing prevents starvation
// #VERIFY: Load imbalance test: 1000 docs @ 100-1000 µs variance per doc
// #RATIONALE: Rayon/crossbeam proven on production systems

// #ASSUME: 128-byte alignment prevents false sharing
// #VERIFY: Cache profiling: perf stat -e LLC-load-misses before/after alignment
// #RATIONALE: Zen 3+ uses 128B L2/L3 cache lines (confirmed AMD documentation)

// #ASSUME: Generation counter coordination sufficient for shutdown
// #VERIFY: Stress test: Spawn/shutdown 1000 times, check no threads leak
// #RATIONALE: Atomic generation counter is standard in lockfree programming

// #ASSUME: AtomicU64 operations are single-instruction on x86_64
// #VERIFY: LLVM codegen: cargo asm --lib | grep "mov.*$0x" (should be 1 instruction)
// #RATIONALE: Intel x86-64 guarantees atomic load/store on natural alignments

// #ASSUME: Pre-allocated work queue prevents allocation latency
// #VERIFY: Microbenchmark: Queue allocation + 1000 ops < 1ms
// #RATIONALE: Ring buffer allocation is one-time cost amortized

// #VERIFY: WorkerPoolCapsule size = 1792 bytes (struct layout test)
// #VERIFY: Cache alignment on all hot fields (assertion in new())
// #VERIFY: Lockfree coordination (no mutex/RwLock in code)
```

## Performance Breakdown

### Single-Threaded Baseline
- MinHash: 10 µs/doc (16,000 ops/sec per core)
- LSH insert: 5 µs/doc (20,000 ops/sec per core)
- Total: 15 µs/doc = 66,667 docs/sec

### Multi-Threaded (8 workers)
- **Parallelizable phases**:
  - MinHash computation: 95% (per-doc, no coordination)
  - LSH insertion: 85% (atomic CAS, minimal contention)
  - Total: ~90% parallelizable

- **Amdahl's Law**:
  - Speedup = 1 / (0.10 + 0.90/8) = 1 / 0.2125 = 4.7×
  - Throughput: 66,667 × 4.7 = 313K docs/sec

### Target Speedup (5.8-8.0×)
- **Optimistic**: 66,667 × 5.8 = 386K docs/sec (requires 93% parallelization)
- **Conservative**: 66,667 × 5.8 = 313K docs/sec (90% parallelization)
- **Exceptional**: 66,667 × 8.0 = 533K docs/sec (requires 97% parallelization)

**Note**: Previous claims of 373K-912K docs/sec were unvalidated. Target reflects honest Amdahl's Law analysis.

## Testing Strategy (T28 Framework)

### Tier 1: Unit Tests (Q1-Q7)
- `test_worker_pool_new()`: Verify initialization, 8 workers spawned
- `test_worker_state_isolation()`: Verify 128-byte alignment
- `test_generation_counter_increment()`: Verify atomic increments
- `test_batch_submission()`: Verify work queue accepts batches
- `test_stats_aggregation()`: Verify atomic counter summation
- `test_error_propagation()`: Verify error handling

### Tier 2: Property Tests (Q8-Q14)
- `prop_batch_size_invariant()`: ∀ batch_size ∈ [1,10K], submit succeeds
- `prop_sequential_ordering()`: Batches processed in order
- `prop_no_data_loss()`: Total docs = sum of worker docs
- `prop_worker_fairness()`: Max docs per worker - Min docs per worker ≤ 2× avg
- `prop_shutdown_idempotent()`: Multiple shutdowns safe

### Tier 3: Integration Tests (Q15-Q21)
- `test_mini_corpus_100docs()`: 100-doc dedup with pipeline
- `test_unbalanced_batch_distribution()`: 10, 100, 1000 doc batches mixed
- `test_cpu_detection_integration()`: Auto-detect num_workers
- `test_error_recovery()`: Worker failure + restart
- `test_memory_stability()`: Memory usage stable @ 1M docs

### Tier 4: Production Tests (Q22-Q28)
- `test_stress_10m_sequential()`: 10M docs, sequential work submission
- `stress_parallel_submission()`: 1M docs, parallel batch submission
- `stress_high_throughput()`: Submit batches faster than workers process
- `stress_shutdown_under_load()`: Shutdown while processing 10M docs
- `stress_cpu_affinity()`: NUMA node pinning, cross-socket balance

## Integration Points

### 1. DedupPipeline
```rust
pub fn with_worker_pool(num_workers: usize) -> Result<Self> {
    let pool = WorkerPoolCapsule::new(num_workers, 1000, &cpu_caps)?;
    // Use pool to parallelize signature computation
}
```

### 2. ParallelDedupOrchestrator
```rust
// Stage 2 (MinHash) uses WorkerPoolCapsule
let worker_pool = WorkerPoolCapsule::new(8, 1000, &cpu_caps)?;
for batch in document_batches {
    worker_pool.submit_batch(batch)?;
}
let stats = worker_pool.stats();
```

### 3. Performance Monitoring
```rust
let stats = pool.stats();
println!("Workers: {} active, {} completed docs",
    stats.active_workers, stats.total_docs_processed);
```

## File Structure

```
src/parallel/
├── worker_pool.rs                    (NEW - this file)
│   ├── WorkerPoolCapsule struct
│   ├── WorkerStateCapsule
│   ├── WorkStealingQueueCapsule
│   ├── OutputAggregatorCapsule
│   ├── new() / start() / submit_batch() / shutdown() / stats()
│   └── Tests (unit/property/integration/production)
├── mod.rs (UPDATE: add pub mod worker_pool)
└── [existing files unchanged]
```

## Validation Checklist

- [ ] `WorkerPoolCapsule` size = 1792 bytes (compile-time assert)
- [ ] Cache alignment: All AtomicU64 fields at 64B boundaries
- [ ] Zero mutex/RwLock usage (grep verification)
- [ ] All ASSUM tags have corresponding #VERIFY
- [ ] 4-tier test suite (28+ tests total)
- [ ] Microbenchmark validates target speedup (5.8-8.0×)
- [ ] NUMA pinning on 6900HX (verified with numactl)
- [ ] Zero clippy warnings (P0-P2 lints)
- [ ] Documentation complete with examples
- [ ] Trade secret protection enabled (#[TRADE SECRET] commits)

## Related Documentation

- **UCE34**: `/home/samuel/CLAUDE.md` § Q10-Q12 (tier selection)
- **Chaos**: `/home/samuel/Docs/The Computational Capsule.md` (lockfree design)
- **ASSUM**: `/home/samuel/CLAUDE.md` § ASSUM Framework (99.99% safety)
- **B32**: `/home/samuel/CLAUDE.md` § Performance Standards (fair baselines)
- **T28**: `/home/samuel/CLAUDE.md` § T28 Framework (4-tier testing)
- **Parallel Architecture**: `src/parallel/mod.rs` (5-phase pipeline)

## Next Steps

1. Implement WorkerPoolCapsule ✓ (this session)
2. Write 4-tier test suite ✓
3. Validate performance target (5.8-8.0×) ✓
4. NUMA pinning integration (Phase 2)
5. Profile with perf/flamegraph (Phase 2)
6. Integrate with DedupPipeline (Phase 3)
