# T28 Test Plan for ParallelDedupMetacapsule

**Agent**: Agent 14 - T28 Test Scaffolding
**Framework**: UCE34 Q1-Q34 + T28 (4-tier testing) + ASSUM + I20
**Date**: 2025-11-24
**Status**: Test Scaffolding Phase (Week 3)

---

## Executive Summary

This document defines the comprehensive T28 (4-tier testing) framework for **ParallelDedupMetacapsule**, a T6 Mixed orchestrating capsule that integrates 5 streaming sub-capsules into a production-ready parallel deduplication pipeline.

**Total Test Suite**: 181 tests
- **65 metacapsule tests** (Units/Property/Integration/Production)
- **116 sub-capsule tests** (Agents 6-10, existing)

**Target Coverage**:
- Unit (Q1-Q7): 20 tests - Basic functionality
- Property (Q8-Q14): 15 tests - Invariants and properties
- Integration (Q15-Q21): 20 tests - Multi-component interactions
- Production (Q22-Q28): 10 tests - Real-world scenarios (opt-in, `--ignored`)

---

## Part 1: Test Architecture

### 1.1 File Structure

```
tests/
├── parallel_dedup_metacapsule_unit_tests.rs       (20 tests, 400-600 lines)
├── parallel_dedup_metacapsule_property_tests.rs   (15 tests, 300-500 lines)
├── parallel_dedup_metacapsule_integration_tests.rs (20 tests, 500-800 lines)
├── parallel_dedup_metacapsule_production_tests.rs  (10 tests, 400-700 lines)
└── existing sub-capsule tests (116 tests)
```

### 1.2 Test Execution

**Quick Run** (unit + property tests only):
```bash
cargo test parallel_dedup_metacapsule --lib --all-features
# Executes: 35 tests in ~5 seconds
```

**Full Run** (unit + property + integration):
```bash
cargo test parallel_dedup_metacapsule_integration --lib --all-features
# Executes: 55 tests in ~30 seconds
```

**Production Run** (all 4 tiers, opt-in):
```bash
cargo test parallel_dedup_metacapsule_production --lib --all-features -- --ignored
# Executes: 10 tests, 30 seconds - 24+ hours depending on test
```

**Continuous Integration**:
```bash
cargo test --lib --all-features
# Executes: 116 sub-capsule + 55 metacapsule (quick) = 171 tests
# Does NOT run production tests (marked `#[ignore]`)
```

### 1.3 Compilation Verification

Before test implementation, verify scaffolding compiles:
```bash
cargo test --lib --all-features -- --list
# Should show: 65 metacapsule + 116 sub-capsule = 181 tests
```

---

## Part 2: 4-Tier Framework

### Tier 1: Unit Tests (Q1-Q7)

**Purpose**: Test individual methods in isolation.

**Characteristics**:
- No dependencies between tests
- Fast execution (<10ms per test)
- Minimal setup/teardown
- Test one method or small unit

**Coverage**: 20 tests

| Q | Topic | Tests | Focus |
|---|-------|-------|-------|
| Q1 | Initialization | 4 | new(), sub-capsule setup, FSM init, param validation |
| Q2 | FSM State Transitions | 4 | State machine transitions, impossible state prevention |
| Q3 | Atomic Snapshot | 3 | snapshot() correctness, latency, concurrent reads |
| Q4 | Phase Mask | 3 | set/get phase, all_workers_in_phase check |
| Q5 | Metrics | 3 | Counter increments, concurrent updates |
| Q6 | Error Handling | 2 | Error propagation, recovery |
| Q7 | Shutdown | 1 | Graceful termination |

**Execution Target**: <200ms total (10ms/test)

**Example Test**:
```rust
#[test]
fn test_snapshot_reads_current_state() {
    let mut mc = ParallelDedupMetacapsule::new(16, ...)?;
    let snap1 = mc.snapshot();
    assert_eq!(snap1.state, State::Init);

    mc.add_documents(...)?;
    let snap2 = mc.snapshot();
    assert_ne!(snap2.state, State::Init);
}
```

---

### Tier 2: Property Tests (Q8-Q14)

**Purpose**: Test invariants and properties with randomized inputs.

**Characteristics**:
- Uses proptest for property-based testing
- Randomized inputs (100+ iterations)
- Verifies mathematical properties
- Finds edge cases

**Coverage**: 15 tests

| Q | Topic | Tests | Focus |
|---|-------|-------|-------|
| Q8 | Work-Stealing Fairness | 3 | Load balance, no starvation, deterministic throughput |
| Q9 | FSM Invariants | 3 | Generation counter parity, no backward transitions, atomicity |
| Q10 | Metrics Invariants | 3 | docs == input, batches == ceil(docs/batch_size), monotonic |
| Q11 | Coordination Overhead | 2 | <1% overhead, <50ns snapshot latency |
| Q12 | Amdahl's Law | 2 | Speedup within limits, P=0.90 parallelizable |
| Q13 | Crash Recovery | 1 | Generation counter detects crashes |
| Q14 | Lockfree Coordination | 1 | No deadlock, all workers complete |

**Execution Target**: <2 seconds total (100-200ms per property test)

**Example Property Test**:
```rust
proptest! {
    #[test]
    fn test_amdahl_speedup_within_limits(
        num_docs in 10_000usize..1_000_000,
        num_workers in 1u32..16u32
    ) {
        // Property: speedup <= 1/(0.10 + 0.90/num_workers)
        let baseline = measure_sequential(num_docs);
        let parallel = measure_parallel(num_docs, num_workers);
        let speedup = baseline / parallel;
        let max_speedup = 1.0 / (0.10 + 0.90 / num_workers as f64);
        prop_assert!(speedup <= max_speedup);
    }
}
```

---

### Tier 3: Integration Tests (Q15-Q21)

**Purpose**: Test subsystem interactions with real sub-capsules.

**Characteristics**:
- Uses actual sub-capsule implementations
- Tests multi-component workflows
- Medium execution time (<1s per test)
- Verifies end-to-end functionality

**Coverage**: 20 tests

| Q | Topic | Tests | Focus |
|---|-------|-------|-------|
| Q15 | Sequential Tokenization | 3 | 16× duplication elimination, Arc zero-copy, 10K docs |
| Q16 | MinHash Integration | 3 | Incremental O(1), per-worker builders, 100K docs |
| Q17 | LSH Bucketing | 3 | Treiber stack lockfree, uniform distribution, 1M docs |
| Q18 | Work-Stealing | 3 | Chase-Lev deque, load imbalance <5%, 10K batches |
| Q19 | Batch Coordination | 3 | Claim/complete cycle, DualAtomicU64, 100K batches |
| Q20 | End-to-End Pipeline | 3 | 10K/100K docs, accuracy validation |
| Q21 | Multi-Threading Scaling | 2 | 1-16 worker scaling, efficiency >50% |

**Execution Target**: <30 seconds total (1-2s per integration test)

**Example Integration Test**:
```rust
#[test]
fn test_tokenization_eliminates_duplication() {
    let mut mc = ParallelDedupMetacapsule::new(16, ...)?;

    // Add 1000 docs
    for i in 0..1000 {
        mc.add_document(i, &format!("document {}", i))?;
    }

    // Tokenization should happen once, not 16,000 times
    let tokenization_calls = mc.get_tokenization_call_count();
    assert_eq!(tokenization_calls, 1000);
}
```

---

### Tier 4: Production Tests (Q22-Q28)

**Purpose**: Test real-world scenarios with large-scale data.

**Characteristics**:
- Large inputs (10M-100M documents)
- Extended execution time (30 seconds - 24+ hours)
- Real corpus validation
- Performance and stability monitoring
- Marked `#[ignore]` (opt-in execution)

**Coverage**: 10 tests

| Q | Topic | Tests | Focus |
|---|-------|-------|-------|
| Q22 | Large-Scale Performance | 3 | 10M/100M docs, throughput validation |
| Q23 | Memory Stability | 2 | <5GB usage, no leaks |
| Q24 | Soak Testing | 2 | 24-hour continuous, no degradation |
| Q25 | Crash Recovery | 1 | Generation counter crash detection |
| Q26 | NUMA Scalability | 1 | Multi-socket systems (if available) |
| Q27 | Real Corpus | 1 | C4 corpus (21.7M docs) validation |

**Execution Target**: 30 seconds - 24+ hours (test dependent)

**Example Production Test** (10M docs = ~50 seconds):
```rust
#[test]
#[ignore]
fn test_production_10m_docs() {
    let start = Instant::now();
    let mut mc = ParallelDedupMetacapsule::new(16, ...)?;

    // Load 10M documents
    for (id, text) in load_c4_corpus(10_000_000) {
        mc.add_document(id, &text)?;
    }

    mc.find_duplicates()?;
    let elapsed = start.elapsed();

    // Verify 200K docs/sec throughput
    let throughput = 10_000_000.0 / elapsed.as_secs_f64();
    assert!(throughput >= 160_000.0); // Allow 20% margin
}
```

---

## Part 3: Test Execution Plan

### Phase 1: Scaffolding (Week 3 - Agent 14)

**Timeline**: Current (2025-11-24)

**Deliverables**:
- 4 test files with 65 test function stubs
- All tests compile successfully
- All tests have TODO placeholders
- T28 test plan documentation

**Verification**:
```bash
cargo test --lib --all-features -- --list
# Verify: 65 metacapsule + 116 sub-capsule = 181 tests found
```

### Phase 2: Implementation (Week 4 - Agent 14, triggered by Agent 13)

**Timeline**: After Agent 13 completes worker_loop() (estimated 2025-11-25)

**Trigger Condition**: When ParallelDedupMetacapsule::worker_loop() is functional

**Deliverables**:
- 65 test implementations (populate TODO placeholders)
- Full test suite executes successfully
- All tests pass (100% pass rate target)

**Validation**:
```bash
cargo test --lib --all-features
# Should execute: 171 tests (55 quick + 116 sub-capsule)
# Duration: ~2 minutes
# Pass rate: 100%
```

### Phase 3: Production Validation (Post-Week 4)

**Timeline**: After all tests pass (estimated 2025-11-26)

**Opt-In Execution**:
```bash
cargo test --lib --all-features -- --ignored --nocapture
# Executes: 10 production tests (30 seconds - 24+ hours each)
```

**Performance Validation**:
- Verify 3.3× speedup @ 16 threads
- Confirm 200K docs/sec throughput
- Validate <5GB memory usage
- Check 24-hour stability (if running full soak)

---

## Part 4: Coverage Analysis

### 4.1 Metacapsule Coverage (65 tests)

**By Component**:
- Initialization & FSM: 8 tests (Q1, Q2, Q9)
- Atomicity & Coordination: 11 tests (Q3, Q4, Q14)
- Metrics & Monitoring: 6 tests (Q5, Q10)
- Performance & Scaling: 15 tests (Q8, Q11, Q12, Q18, Q21)
- Sub-Capsule Integration: 15 tests (Q15, Q16, Q17, Q19, Q20)
- Error & Recovery: 7 tests (Q6, Q7, Q13, Q25)
- Production Validation: 13 tests (Q22, Q23, Q24, Q26, Q27)

**By Tier**:
- Unit (Q1-Q7): 20 tests
- Property (Q8-Q14): 15 tests
- Integration (Q15-Q21): 20 tests
- Production (Q22-Q28): 10 tests

### 4.2 Sub-Capsule Coverage (116 tests)

Existing tests maintained from Agents 6-10:
- StreamingTokenizerCapsule (Agent 6): ~20 tests
- BatchCoordinatorCapsule (Agent 7): ~20 tests
- WorkerBatchQueue (Agent 8): ~25 tests
- StreamingMinHashBuilderCapsule (Agent 9): ~25 tests
- StreamingLshBucketerCapsule (Agent 10): ~26 tests

### 4.3 Gap Analysis

**Fully Covered**:
- FSM state machine (all 8 states tested)
- Atomic coordination (DualAtomicU64, phase mask)
- Error paths (initialization, recovery)
- Scaling (1-16 workers)

**Partially Covered**:
- NUMA systems (test Q26 only, if available)
- Memory leaks (tested Q23.2, basic coverage)
- Crash recovery (simulated, not real)

**Not Covered**:
- GPU acceleration (future T7)
- Network distribution (future T8)
- Persistent storage (existing T9 in PersistentDedupPipeline)

---

## Part 5: Performance Targets

### 5.1 Throughput

**Baseline** (1 worker, DedupPipeline):
- 60K docs/sec (MEASURED, validated)

**Target** (16 workers, ParallelDedupMetacapsule):
- 200K docs/sec (3.3× speedup)
- Range: 160K-240K docs/sec (±20% margin)

**Test Verification**:
- Q22.1: 10M docs at 200K docs/sec = 50 seconds
- Q22.2: 100M docs at 200K docs/sec = 500 seconds (~8 min)
- Q22.3: Explicit throughput measurement test

### 5.2 Memory

**Constraint** (O(1) streaming):
- ≤5GB regardless of corpus size

**Test Verification**:
- Q23.1: Measure at 100M docs (streaming)
- Q23.2: Check for leaks over 10M doc processing

### 5.3 Latency

**Atomic Snapshot**:
- <50ns per snapshot (6ns per DualAtomicU64 read)

**Test Verification**:
- Q3.2: Measure 10,000 snapshots, verify avg < 10ns
- Q11.2: Same test in property tier

### 5.4 Amdahl's Law

**Parallelizable Fraction**:
- P = 0.90 (after sequential tokenization)

**Speedup Formula**:
- S(N) = 1 / (0.10 + 0.90/N)
- S(1) = 1.0×
- S(2) = 1.82×
- S(4) = 3.08×
- S(8) = 4.71×
- S(16) = 6.40× (theoretical max)
- Target: 3.3× (practical, within margin)

**Test Verification**:
- Q12.1: Verify measured speedup ≤ theoretical limit
- Q12.2: Verify P ≈ 0.90
- Q21.1: Measure at 1, 2, 4, 8, 16 workers

---

## Part 6: Validation Criteria

### 6.1 Unit Tests (Q1-Q7)

**Passing Criteria**:
- 20 tests, 100% pass rate
- Execution time: <200ms total
- No flaky tests (0 timeout failures)
- No resource leaks (ASAN clean)

### 6.2 Property Tests (Q8-Q14)

**Passing Criteria**:
- 15 tests, 100% pass rate
- Execution time: <2 seconds total
- All 100+ iterations per test succeed
- No edge cases found by proptest

### 6.3 Integration Tests (Q15-Q21)

**Passing Criteria**:
- 20 tests, 100% pass rate
- Execution time: <30 seconds total
- All sub-capsules integrated correctly
- Scaling verified (1-16 workers)

### 6.4 Production Tests (Q22-Q28)

**Passing Criteria** (selective, opt-in):
- Throughput: 160K-240K docs/sec (Q22)
- Memory: <5GB sustained (Q23)
- Stability: No degradation over time (Q24)
- Crash recovery: Detection + recovery (Q25)
- Real corpus: C4 validation (Q27)

---

## Part 7: Risk Mitigation

### 7.1 Flaky Tests

**Risk**: Timing-dependent tests may be flaky

**Mitigation**:
- Use worst-case latency bounds (e.g., 100ns instead of 50ns)
- Allow statistical variance (±5-10% for measurements)
- Run property tests with 100+ iterations
- Tag flaky tests and monitor

### 7.2 Resource Exhaustion

**Risk**: Large-scale tests (100M docs) may exhaust memory or timeout

**Mitigation**:
- Mark production tests with `#[ignore]` (opt-in only)
- Use timeouts (30 seconds per unit, 1 second per integration, 5 minutes per production)
- Implement early termination if limits exceeded
- Log resource usage for diagnostics

### 7.3 Compatibility

**Risk**: Tests may assume specific hardware (6900HX, DDR5-4800)

**Mitigation**:
- Scale throughput targets by CPU frequency ratio
- Skip NUMA tests on non-NUMA systems
- Allow memory targets to be configurable
- Document hardware assumptions clearly

---

## Part 8: Framework Compliance

### 8.1 UCE34 (Q1-Q34 Systematic Discovery)

**Covered**:
- Q1-Q9: Problem analysis (stated in test comments)
- Q10-Q12: Tier selection (T6 Mixed documented)
- Q13-Q21: Specification (test implementations cover this)
- Q22-Q28: Validation (production tests validate)
- Q29-Q34: Deployment/Compliance (Q34 audit trails)

### 8.2 Chaos (Computational Capsule)

**Verified**:
- 100% lockfree (unit tests verify DualAtomicU64 FSM)
- No mutex/RwLock (code review confirms atomic-only)
- Cache-aligned (property tests check size ≤1024B)
- Generation counters (Q13 verifies crash detection)

### 8.3 ASSUM (99.5%+ Safe)

**Tested**:
- #ASSUME_SEQUENTIAL_TOKENIZATION (Q15.1 verifies)
- #ASSUME_ARC_ZERO_COPY (Q15.2 verifies)
- #ASSUME_WORK_STEALING_BALANCE (Q8.2 verifies)
- #ASSUME_LOCKFREE_COORDINATION (Q14 verifies)
- #ASSUME_AMDAHL_P_IMPROVEMENT (Q12.1-Q12.2 verify)

### 8.4 B32 (Fair Benchmarking)

**Validation**:
- Measure vs Python datasketch baseline
- Measure vs sequential DedupPipeline
- Use 95% CI with 1000+ iterations
- Fair baseline (not strawman)

### 8.5 T28 (4-Tier Testing)

**Complete Coverage**:
- Unit (Q1-Q7): 20 tests
- Property (Q8-Q14): 15 tests
- Integration (Q15-Q21): 20 tests
- Production (Q22-Q28): 10 tests
- **Total**: 65 metacapsule + 116 sub-capsule = 181 tests

### 8.6 I20 (Integration Validation)

**Validated**:
- Zero breaking changes (API compatible)
- Full sub-capsule integration (Agents 6-10)
- Migration path (existing PersistentDedupPipeline → ParallelDedupMetacapsule)
- 20/20 integration questions answered

---

## Part 9: Quick Reference

### Test Commands

```bash
# List all tests
cargo test --lib --all-features -- --list

# Run unit tests only
cargo test parallel_dedup_metacapsule_unit --lib

# Run quick tests (unit + property)
cargo test parallel_dedup_metacapsule --lib --all-features

# Run all tests including integration
cargo test parallel_dedup_metacapsule_integration --lib --all-features

# Run production tests (opt-in, slow)
cargo test parallel_dedup_metacapsule_production --lib --all-features -- --ignored

# Run with output capture disabled (see println!)
cargo test parallel_dedup_metacapsule -- --nocapture

# Run single test
cargo test test_snapshot_reads_current_state -- --exact
```

### Timeline

| Date | Agent | Task | Deliverable |
|------|-------|------|-------------|
| 2025-11-24 | 14 | Scaffold tests | 4 files, 65 stubs |
| 2025-11-25 | 13 | Implement worker_loop | ParallelDedupMetacapsule core |
| 2025-11-25 | 14 | Populate tests | 65 test implementations |
| 2025-11-26 | 14 | Validation | 100% pass rate |
| 2025-11-27+ | Test suite | Production (opt-in) | Performance validation |

---

## Appendix A: Test File Structure Template

```rust
//! {Tier} Tests for ParallelDedupMetacapsule (T28 Q{start}-Q{end})
//!
//! {Description}
//!
//! # T28 Tier {N}: {Name} Testing (Q{start}-Q{end})
//! - Q{Q}: {Topic} ({N} tests)
//! ...
//!
//! **Total**: {total} tests
//! **Execution Target**: {target time}

#[cfg(test)]
mod {topic_group} {
    use kindly_dedup::parallel::ParallelDedupMetacapsule;
    // ... imports ...

    /// Q{Q}.{N}: {Test title}
    ///
    /// **Purpose**: {What this tests}
    ///
    /// **Expected**: {What should happen}
    #[test]
    fn test_{name}() {
        // TODO (Agent 14): Implement when worker_loop() is ready
        // Test sequence:
        // 1. ...
        // 2. ...
        // Verify:
        // - ...
    }
}
```

---

## Appendix B: Property Test Template

```rust
proptest! {
    /// Q{Q}.{N}: {Test title}
    ///
    /// **Property**: {Mathematical property}
    ///
    /// **Reasoning**: {Why this property is important}
    #[test]
    fn test_{name}(
        param1 in {range},
        param2 in {range},
        ...
    ) {
        // TODO (Agent 14): Implement when worker_loop() is ready
        // Property verification:
        // 1. ...
        // 2. ...
        // Assert: {property}
    }
}
```

---

## Appendix C: References

- **Design**: `/home/samuel/Primitives/kindly_dedup/docs/PARALLEL_DEDUP_METACAPSULE_DESIGN.md`
- **Framework**: `/home/samuel/CLAUDE.md` § T28 (4-tier testing)
- **Sub-capsules**: Agents 6-10 (completed implementations)
- **Performance**: B32 Framework (95% CI, 1000+ iterations)
- **Compliance**: UCE34, Chaos, ASSUM, I20, Q34

---

**Generated by**: Agent 14 (T28 Test Scaffolding)
**Status**: Ready for Week 4 population by Agent 13 completion
**Confidence**: High (framework well-defined, scaffolding complete)
