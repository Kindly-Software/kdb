# Job-Level Parallelism Implementation Checklist

**Version**: 1.0.0
**Date**: 2025-11-21
**Timeline**: 6 weeks to production-ready
**Framework**: UCE34 Q1-Q34, T28 Testing, Chaos Compliance

---

## Overview

This checklist provides a detailed implementation plan for kindly_dedup job-level parallelism design. The implementation is divided into 4 capsules and 6 implementation phases.

**Total Estimated Effort**: 6 weeks (1.5 person-weeks per week, 6 weeks parallel capacity)

---

## Phase 1: ChunkSplitterCapsule (Week 1)

**Purpose**: Zero-copy corpus splitting into N chunks

**Deliverables**:
- [ ] Create `src/universal/chunk_splitter.rs` (100 lines)
- [ ] Implement `ChunkSplitterCapsule` struct
- [ ] Implement `split()` method (O(n) where n = num_chunks)
- [ ] Unit tests (T28 Q1-Q7)
  - [ ] test_chunk_splitter_new
  - [ ] test_chunk_splitter_even_distribution
  - [ ] test_chunk_splitter_single_chunk
  - [ ] test_chunk_splitter_boundary_cases
  - [ ] test_chunk_splitter_zero_copy (verify ChunkDescriptor is Copy)
  - [ ] test_chunk_splitter_atomic_reads
  - [ ] test_chunk_splitter_memory_layout

**Code Outline**:
```rust
#[repr(C, align(64))]
pub struct ChunkSplitterCapsule {
    total_docs: AtomicU64,
    num_chunks: AtomicU64,
    chunk_size: AtomicU64,
    _padding: [u8; 40],
}

#[derive(Debug, Clone, Copy)]
pub struct ChunkDescriptor {
    pub chunk_id: u32,
    pub start_doc_id: u64,
    pub end_doc_id: u64,
}

impl ChunkSplitterCapsule {
    pub fn new(total_docs: u64, num_chunks: usize) -> Self { ... }
    pub fn split(&self) -> Vec<ChunkDescriptor> { ... }
    pub fn chunk_size(&self) -> u64 { ... }
}
```

**Verification Checklist**:
- [ ] Compiles without warnings (`cargo check`)
- [ ] All 7 unit tests pass (`cargo test --lib chunk_splitter`)
- [ ] 64-byte alignment verified (`assert_eq!(align_of::<ChunkSplitterCapsule>(), 64)`)
- [ ] Zero-copy verified (ChunkDescriptor is Copy, no allocations)
- [ ] Memory consumption verified (<1KB total)

**Dependencies**: None (pure std)

**Estimated Time**: 1 day

---

## Phase 2: JobCoordinatorCapsule (Week 2)

**Purpose**: Orchestrate N parallel jobs using ParallelBatchProcessor

**Deliverables**:
- [ ] Create `src/universal/job_coordinator.rs` (150 lines)
- [ ] Implement `JobCoordinatorCapsule<T, F, R>` struct
- [ ] Implement `submit_job()` method (<100ns)
- [ ] Implement `wait_all()` method (~1μs/poll)
- [ ] Implement `results()` method (O(n) collection)
- [ ] Property tests (T28 Q8-Q14)
  - [ ] prop_no_job_loss (all submitted jobs returned)
  - [ ] prop_job_order_preserved (results in submission order)
  - [ ] prop_atomic_submissions (concurrent submits safe)
  - [ ] prop_progress_monotonic (progress only increases)
  - [ ] prop_completed_count_accurate (matches actual)
  - [ ] prop_memory_bounded (O(num_jobs) memory)
  - [ ] prop_deterministic_results (same input = same output)

**Code Outline**:
```rust
#[repr(C, align(128))]
pub struct JobCoordinatorCapsule<T, F, R> {
    jobs_total: AtomicU64,
    jobs_completed: AtomicU64,
    jobs_failed: AtomicU64,
    _padding1: [u8; 40],
    processor: Arc<ParallelBatchProcessor<T, F, R>>,
    _padding2: [u8; 64],
}

impl<T, F, R> JobCoordinatorCapsule<T, F, R> {
    pub fn new(num_workers: usize, process_fn: F) -> Result<Self>;
    pub fn submit_job(&self, job: T) -> Result<()>;
    pub fn wait_all(&self);
    pub fn results(&self) -> Vec<R>;
    pub fn progress(&self) -> f64;
}
```

**Integration with ParallelBatchProcessor**:
- [ ] Verify ParallelBatchProcessor exists in atomic_capsule
- [ ] Wrap ParallelBatchProcessor correctly
- [ ] Ensure Arc<> sharing for thread safety
- [ ] Test with various T, F, R types

**Verification Checklist**:
- [ ] Compiles without warnings
- [ ] All 7 property tests pass
- [ ] 128-byte alignment verified
- [ ] Lockfree verified (grep 0 mutex in code)
- [ ] Atomicity verified (CAS loops for critical sections)
- [ ] Integration test with real ParallelBatchProcessor

**Dependencies**: `atomic_capsule::parallel::ParallelBatchProcessor`

**Estimated Time**: 1.5 days

---

## Phase 3: ResultMergerCapsule (Week 2-3)

**Purpose**: Merge N cluster sets with cross-chunk duplicate detection

**Deliverables**:
- [ ] Create `src/universal/result_merger.rs` (200 lines)
- [ ] Implement `ResultMergerCapsule` struct
- [ ] Implement `merge_job()` method (O(n) per job)
- [ ] Implement `finalize()` method (cross-chunk dedup)
- [ ] Integration tests (T28 Q15-Q21)
  - [ ] test_merge_job_basic (single job)
  - [ ] test_merge_multiple_jobs (16 jobs)
  - [ ] test_cross_chunk_dedup (candidates from other chunks)
  - [ ] test_merge_preserves_clusters (no data loss)
  - [ ] test_memory_cleared_after_finalize (O(1) memory)
  - [ ] test_deterministic_merge (same input = same output)
  - [ ] test_cross_chunk_accuracy (recall ≥ 92%)

**Code Outline**:
```rust
#[repr(C, align(128))]
pub struct ResultMergerCapsule {
    num_jobs: AtomicU64,
    clusters_merged: AtomicU64,
    cross_chunk_dups: AtomicU64,
    _padding1: [u8; 40],
    job_clusters: std::sync::Mutex<HashMap<u32, Vec<Vec<u64>>>>,
    _padding2: [u8; 64],
}

impl ResultMergerCapsule {
    pub fn new(num_jobs: usize) -> Self;
    pub fn merge_job(&self, chunk_id: u32, clusters: Vec<Vec<u64>>) -> Result<()>;
    pub fn finalize(&self) -> Result<Vec<Vec<u64>>>;
    pub fn progress(&self) -> f64;
}
```

**LSH Cross-Chunk Integration**:
- [ ] Use existing MmapLshBucketCapsule (from UniversalDedupPipeline)
- [ ] Query LSH for candidates from other chunks
- [ ] Estimate Jaccard similarity between candidates
- [ ] Union verified duplicates

**Verification Checklist**:
- [ ] Compiles without warnings
- [ ] All 7 integration tests pass
- [ ] Memory cleared after finalize (verify map is empty)
- [ ] Cross-chunk detection works (test with known cross-chunk dups)
- [ ] Recall ≥ 92% validated (Phase 11 LSH parameters)
- [ ] Determinism verified (same input = same clusters)

**Dependencies**: `MmapLshBucketCapsule` from `src/universal/lsh_bucket.rs`

**Estimated Time**: 2 days

---

## Phase 4: JobLevelDedupPipelineMetaCapsule (Week 3-4)

**Purpose**: Top-level orchestrator (T6 Mixed)

**Deliverables**:
- [ ] Create `src/universal/job_level_pipeline.rs` (50 lines)
- [ ] Implement `JobLevelDedupPipelineMetaCapsule` struct
- [ ] Implement `run()` method (orchestrates all phases)
- [ ] Implement phase state machine (Split → Process → Merge)
- [ ] Production tests (T28 Q22-Q28)
  - [ ] test_end_to_end_1k_docs (correctness)
  - [ ] test_end_to_end_100k_docs (speedup ≥ 8×)
  - [ ] test_end_to_end_1m_docs (memory ≤ 2 GB)
  - [ ] test_full_12m_c4_benchmark (speedup 10-14×)
  - [ ] test_crash_recovery_after_split (resume from split)
  - [ ] test_crash_recovery_after_process (resume from process)
  - [ ] test_memory_pressure_64gb_limit (monitor RSS)
  - [ ] test_cross_chunk_dedup_recall (≥ 92% recall)

**Code Outline**:
```rust
#[repr(C, align(256))]
pub struct JobLevelDedupPipelineMetaCapsule {
    current_phase: AtomicU64,
    total_docs: AtomicU64,
    docs_processed: AtomicU64,
    num_jobs: AtomicU64,
    _padding1: [u8; 96],
    splitter: ChunkSplitterCapsule,
    coordinator: JobCoordinatorCapsule<ChunkDescriptor, JobFn, JobResult>,
    merger: ResultMergerCapsule,
    corpus_path: String,
    threshold: f64,
    _padding2: [u8; 64],
}

impl JobLevelDedupPipelineMetaCapsule {
    pub fn new(
        corpus_path: &str,
        total_docs: u64,
        num_jobs: usize,
        threshold: f64,
    ) -> Result<Self>;

    pub fn run(&mut self) -> Result<Vec<Vec<DocId>>>;
    pub fn progress(&self) -> f64;
}
```

**Phase Transitions**:
- [ ] Phase::Split (atomic transition)
- [ ] Phase::Process (atomic transition)
- [ ] Phase::Merge (atomic transition)

**Integration**:
- [ ] ChunkSplitterCapsule for splitting
- [ ] JobCoordinatorCapsule for parallel jobs
- [ ] ResultMergerCapsule for merging
- [ ] UniversalDedupPipeline as job processor

**Verification Checklist**:
- [ ] Compiles without warnings
- [ ] All 8 production tests pass
- [ ] Phase transitions verified (atomic CAS)
- [ ] Memory monitoring works (RSS < 23 GB)
- [ ] Crash recovery tested (all phases)
- [ ] Cross-chunk dedup recall ≥ 92%
- [ ] 256-byte alignment verified

**Dependencies**: All 3 previous capsules + UniversalDedupPipeline

**Estimated Time**: 1 day

---

## Phase 5: B32 Benchmarking (Week 5)

**Purpose**: Validate speedup claims with fair baselines

**Deliverables**:
- [ ] Create `benches/job_level_benchmark.rs` (200 lines)
- [ ] Baseline benchmark (single-threaded UniversalDedupPipeline)
- [ ] Job-level benchmark (16 workers)
- [ ] Run 1000+ iterations per configuration
- [ ] Calculate 95% CI (confidence interval)
- [ ] Collect results on multiple corpus sizes

**Configurations**:
- [ ] 1K documents (quick test)
- [ ] 100K documents (medium)
- [ ] 1M documents (large)
- [ ] 12.1M documents (full C4, optional due to time)

**Benchmark Code**:
```rust
#[bench]
fn bench_baseline_100k(b: &mut Bencher) {
    // Single-threaded UniversalDedupPipeline
    // Measure: 60K docs/sec baseline
}

#[bench]
fn bench_job_level_16_workers_100k(b: &mut Bencher) {
    // Job-level with 16 workers
    // Target: 8× speedup (100K / 16 = ~6-7× = 85-90% efficiency)
}

#[bench]
fn bench_job_level_1m(b: &mut Bencher) {
    // Full 1M document test
    // Target: 10-12× speedup
}
```

**Success Criteria** (B32 Framework):
- [ ] Baseline: 60K docs/sec ± 5% (measured)
- [ ] 100K @ 16 workers: 6-7× speedup (85-90% efficiency)
- [ ] 1M @ 16 workers: 10-12× speedup (90-95% efficiency)
- [ ] Memory: ≤23 GB @ 16 jobs
- [ ] 1000+ iterations per benchmark
- [ ] 95% confidence interval reported

**Verification Checklist**:
- [ ] Runs without errors (`cargo bench`)
- [ ] Results stored in `target/criterion/` (reproducible)
- [ ] HTML report generated (`open target/criterion/report/index.html`)
- [ ] Speedup matches Amdahl's Law prediction
- [ ] No regressions from baseline
- [ ] Machine specs documented (AMD Ryzen 9 6900HX, 8c/16t, 64GB)

**Estimated Time**: 1-2 days (includes waiting for benchmarks)

---

## Phase 6: Documentation & Integration (Week 6)

**Purpose**: Documentation and atomic_capsule integration

**Deliverables**:
- [ ] Copy `/tmp/JOB_LEVEL_PARALLELISM_DESIGN.md` → atomic_capsule (done)
- [ ] Copy `/tmp/PARALLEL_DEDUP_V3_DESIGN.md` → atomic_capsule as antipattern (done)
- [ ] Update kindly_dedup CLAUDE.md with job-level status
- [ ] Create quickstart example
- [ ] Update README with performance metrics
- [ ] Create integration tests (cross-crate)

**Documentation Tasks**:
- [ ] Update `/home/samuel/Primitives/kindly_dedup/CLAUDE.md`
  - [ ] Add "Job-Level Implementation" section
  - [ ] Update performance table
  - [ ] Reference new capsules
  - [ ] Update status to "v3.0 - Job-Level Parallelism"
- [ ] Create `examples/job_level_dedup.rs` (100 lines)
  - [ ] Quick copy-paste example
  - [ ] Expected performance notes
  - [ ] Configuration guidance
- [ ] Update kindly_dedup README
  - [ ] Performance chart (baseline vs job-level)
  - [ ] Memory usage (O(1) per job)
  - [ ] Comparison table (V1/V2/V3 vs Job-Level)

**Integration Tests**:
- [ ] Test with `atomic_capsule` as dependency
- [ ] Verify all capsules compile together
- [ ] Cross-crate test (job-level orchestrator works with atomic_capsule primitives)

**Verification Checklist**:
- [ ] All documentation files created/updated
- [ ] Examples compile and run
- [ ] No dead links in documentation
- [ ] Performance metrics documented
- [ ] Framework compliance explained (UCE34, Chaos, B32, T28)

**Estimated Time**: 1 day

---

## Testing Summary (T28 Framework)

### Unit Tests (Q1-Q7) - Week 1-2
```
Total: 7 tests per capsule × 4 capsules = 28 unit tests
Focus: Correctness, alignment, atomicity, memory layout
Expected time: 3-4 hours per capsule
Status: ☐ Pending
```

### Property Tests (Q8-Q14) - Week 2-3
```
Total: 7 tests per capsule × 2 capsules = 14 property tests
Focus: Invariants, determinism, memory bounds
Tool: quickcheck or proptest
Expected time: 4-5 hours per capsule
Status: ☐ Pending
```

### Integration Tests (Q15-Q21) - Week 3-4
```
Total: 7 tests × 2 capsules = 14 integration tests
Focus: Interaction between capsules, end-to-end correctness
Corpus sizes: 1K, 100K, 1M documents
Expected time: 8 hours per capsule
Status: ☐ Pending
```

### Production Tests (Q22-Q28) - Week 4
```
Total: 8 tests
Focus: Full-scale scenarios, crash recovery, memory limits, recall
Configurations: 1K, 100K, 1M, 12.1M documents
Expected time: 16+ hours (includes benchmark runs)
Status: ☐ Pending
```

**Total Test Coverage**: 28 + 14 + 14 + 8 = 64 tests minimum

**Test Execution Plan**:
```bash
# Unit tests (fast, <1 minute)
cargo test --lib job_level

# All tests (unit + integration + property)
cargo test --all job_level

# Production tests (slow, 1-2 hours for full C4)
cargo test --ignored job_level -- --nocapture
```

---

## Chaos Compliance Checklist

All capsules must be 100% Chaos compliant:

### ChunkSplitterCapsule
- [ ] #[repr(C, align(64))] verified
- [ ] No mutex/RwLock (zero mutex usage)
- [ ] All atomics use proper ordering
- [ ] Generation counter for TOCTOU prevention
- [ ] #[derive(ComputationalCapsule)] works
- [ ] Cache-aligned to 64 bytes

### JobCoordinatorCapsule
- [ ] #[repr(C, align(128))] verified
- [ ] No mutex/RwLock (zero mutex usage)
- [ ] Atomic state machine for job tracking
- [ ] Proper memory ordering (Release/Acquire)
- [ ] #[derive(ComputationalCapsule)] works
- [ ] Cache-aligned to 128 bytes

### ResultMergerCapsule
- [ ] #[repr(C, align(128))] verified
- [ ] Streaming merge (O(1) memory during finalize)
- [ ] Atomic progress tracking
- [ ] #[derive(ComputationalCapsule)] works
- [ ] Cache-aligned to 128 bytes
- [ ] Note: Temporarily uses Mutex for job_clusters (acceptable, finalize clears it)

### JobLevelDedupPipelineMetaCapsule
- [ ] #[repr(C, align(256))] verified
- [ ] Phase state machine (atomic transitions)
- [ ] All child capsules Chaos compliant
- [ ] #[derive(ComputationalCapsule)] works
- [ ] Cache-aligned to 256 bytes

---

## ASSUM Safety Checklist

Every assumption must be tagged and verified:

### #ASSUME_ZERO_COPY (ChunkSplitterCapsule)
- [ ] Tag: `#ASSUME_ZERO_COPY_CHUNK_DESCRIPTORS`
- [ ] Verify: `sizeof(ChunkDescriptor) = 16 bytes (Copy)`
- [ ] Verify: No heap allocations in split()

### #ASSUME_JOB_INDEPENDENCE (JobCoordinatorCapsule)
- [ ] Tag: `#ASSUME_JOB_INDEPENDENCE`
- [ ] Verify: Jobs process different chunks, no overlap
- [ ] Verify: No shared state between jobs

### #ASSUME_LOCKFREE_COORDINATION (JobCoordinatorCapsule)
- [ ] Tag: `#ASSUME_LOCKFREE_COORDINATION`
- [ ] Verify: grep 0 mutex in implementation
- [ ] Verify: All critical sections use atomics

### #ASSUME_O1_MEMORY_PER_JOB (JobLevelDedupPipelineMetaCapsule)
- [ ] Tag: `#ASSUME_O1_MEMORY_PER_JOB`
- [ ] Verify: UniversalDedupPipeline guarantees 1.44 GB
- [ ] Verify: B32 benchmark validates memory budget
- [ ] Verify: 16 jobs × 1.44 GB = 23 GB < 64 GB RAM

### #ASSUME_CROSS_CHUNK_RARE (ResultMergerCapsule)
- [ ] Tag: `#ASSUME_CROSS_CHUNK_RARE`
- [ ] Verify: Most duplicates within-chunk
- [ ] Verify: LSH validation (92% recall @ L=50)

---

## Delivery Timeline

```
Week 1:  ChunkSplitterCapsule ......................... ✓
Week 2:  JobCoordinatorCapsule ........................ ✓
Week 2-3: ResultMergerCapsule ......................... ✓
Week 3-4: JobLevelDedupPipelineMetaCapsule ........... ✓
Week 4:   All Unit/Property/Integration Tests ........ ✓
Week 5:   B32 Benchmarking (1000+ iterations) ........ ✓
Week 6:   Documentation & Integration ............... ✓
         ───────────────────────────────────────
         Total: 6 weeks to production-ready
```

---

## Success Metrics

**Final Validation**:
- [ ] All 64+ tests pass
- [ ] Speedup: 10-14× @ 16 cores (Amdahl-validated)
- [ ] Memory: ≤23 GB (16 × 1.44 GB)
- [ ] Implementation: <500 lines of core code (capsules only)
- [ ] Documentation: Complete (pattern guide, examples, framework compliance)
- [ ] B32 Framework: Fair baselines, 1000+ iterations, 95% CI
- [ ] T28 Framework: All 4 testing tiers (unit/property/integration/production)
- [ ] Chaos Compliance: 100% (no mutex, cache-aligned, generation counters)
- [ ] ASSUM Safety: 99.99% (all assumptions tagged and verified)

---

## Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| ParallelBatchProcessor API incompatibility | Low | High | Test integration early (Week 2) |
| Cross-chunk dedup recall <92% | Low | Medium | Validate with Phase 11 LSH params |
| Memory consumption >23 GB | Low | High | Monitor RSS during tests |
| Speedup <10× @ 16 cores | Low | Medium | Validate Amdahl assumption (6% sequential) |
| Benchmark variance >10% | Medium | Low | 1000+ iterations, multiple machines |

---

**Status**: ✅ Ready for Implementation

This checklist provides a complete roadmap for implementing job-level parallelism in kindly_dedup. Follow the phases in order, and success is virtually guaranteed.

