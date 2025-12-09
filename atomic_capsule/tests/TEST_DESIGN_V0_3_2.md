# Comprehensive Test Suite Design - v0.3.2
## T28 4-Tier Test Pyramid for Phase 2 Features

**Status**: Design Complete | **Implementation**: In Progress
**Target**: 180+ tests across all 4 tiers | **Coverage**: 100% pass rate goal

---

## Executive Summary

This document outlines the comprehensive testing strategy for v0.3.2, covering:
1. **Parallel Queue Fix**: Chase-Lev steal() semantics relaxed (10 tests)
2. **Serialization**: FixedPointSerialize trait (binary/decimal/hash) (50 tests)
3. **Persistent Storage**: PersistentMap + PersistentLog with audit trails (70 tests)

**Total Test Count**: 180+ tests across Unit/Property/Integration/Production tiers

---

## TIER 1: UNIT TESTS (Q1-Q7) - 60 tests

### Focus
- Individual component correctness
- Layout and alignment validation
- Atomic field properties
- Hash chain computation

### Parallel Queue Fix (10 tests)

| Test | Purpose | Expected Outcome |
|------|---------|------------------|
| `test_queue_steal_last_element` | Verify steal() allows last element | Success (Chase-Lev relaxed) |
| `test_queue_pop_still_works` | Verify pop() unaffected by fix | pop() succeeds |
| `test_queue_alignment` | Verify 128B alignment | Assert 128B alignment |
| `test_queue_padding` | Verify 64B padding between head/tail | Size ≥ 128B |
| `test_generation_counter_increment` | Verify generation increments | Monotonic increase |
| `test_atomic_ordering_acquire_release` | Verify Acquire/Release semantics | All tasks execute |
| `test_push_when_full` | Verify deterministic failure | QueueFull error |
| `test_steal_from_empty_queue` | Verify empty queue behavior | None returned |
| `test_pop_from_empty_queue` | Verify empty queue behavior | None returned |
| `test_fifo_order_for_steal` | Verify FIFO order for steal() | Sequential IDs 0..10 |

### Serialization (20 tests)

#### Roundtrip Tests (5 tests)
| Test | Type | Expected |
|------|------|----------|
| `test_q8_8_binary_roundtrip` | Q8_8 | Value preserved |
| `test_q16_16_binary_roundtrip` | Q16_16 | Value preserved |
| `test_q32_32_binary_roundtrip` | Q32_32 | Value preserved |
| `test_decimal_roundtrip_q16_16` | Decimal | Value preserved |
| `test_hash_determinism` | Hash | Same hash twice |

#### Precision Limits (5 tests)
- Q8.8 max: 127.996... (saturation)
- Q8.8 min: -128.0 (saturation)
- Q16.16 precision: 1/65536 ≈ 0.0000153
- Q32.32 large values: 1M+ preserved
- Fractional precision: Banker's rounding

#### Negative Numbers (3 tests)
- Negative Q8_8 roundtrip
- Negative decimal format (minus sign)
- Negative zero handling

#### Overflow Saturation (4 tests)
- Q8_8 overflow → 127 max
- Q8_8 underflow → -128 min
- Q16_16 large overflow → 32767 max
- Q32_32 no overflow (huge range)

#### Banker's Rounding (3 tests)
- Half-to-even rounding (2.5 → 2, 3.5 → 4)
- Decimal format Q8_8 ("10.25")
- Negative decimal format ("-42.5")

### Persistent Storage Headers (30 tests)

#### PersistentMapHeader Layout (10 tests)
| Field | Test | Validation |
|-------|------|------------|
| Size | `test_map_header_size` | 256 bytes exact |
| Alignment | `test_map_header_alignment` | 256-byte aligned |
| generation | `test_map_header_generation_field` | Initial 0 |
| entry_count | `test_map_header_entry_count_field` | Initial 0 |
| bucket_count | `test_map_header_bucket_count_field` | Matches init |
| load_factor | `test_map_header_load_factor_field` | Initial 0 |
| hash_prev | `test_map_header_hash_prev_field` | Initial 0 |
| Atomic ops | `test_map_header_atomic_fields` | Updates work |
| Offsets | `test_map_header_field_offsets` | Correct order |
| Padding | `test_map_header_padding` | 216 bytes |

#### PersistentLogHeader Layout (10 tests)
| Field | Test | Validation |
|-------|------|------------|
| Size | `test_log_header_size` | 256 bytes exact |
| Alignment | `test_log_header_alignment` | 256-byte aligned |
| generation | `test_log_header_generation_field` | Initial 0 |
| head | `test_log_header_head_field` | Initial 0 |
| capacity | `test_log_header_capacity_field` | Matches init |
| entry_count | `test_log_header_entry_count_field` | Initial 0 |
| segment_size | `test_log_header_segment_size_field` | Matches init |
| Atomic ops | `test_log_header_atomic_operations` | Allocate advances head |
| Offsets | `test_log_header_field_offsets` | Correct order |
| Padding | `test_log_header_padding` | 208 bytes |

#### Hash Chain Tests (5 tests)
- Map hash computation (non-zero)
- Log hash computation (non-zero)
- Map hash determinism (same twice)
- Log hash determinism (same twice)
- Hash changes on update

#### Atomic Field Properties (5 tests)
- Map generation monotonic (increments)
- Log head advances (cumulative)
- Map entry count accuracy (1..100)
- Log entry count accuracy (manual check)
- Load factor computation (512/1024 = 50%)

---

## TIER 2: PROPERTY TESTS (Q8-Q14) - 50 tests

### Focus
- Invariants under concurrent/repeated operations
- Randomized testing (100+ cases per property)
- Memory ordering correctness
- ABA prevention via generation counters

### Parallel Queue (10 tests)

| Property | Iterations | Assertion |
|----------|-----------|-----------|
| Concurrent steal() no loss | 1000 | All tasks executed |
| No double-steals | 1000 | Each task executed once |
| FIFO order preserved | 1000 | Sequential per thread |
| No buffer overflows | 1000 | Bounds checked |
| Generation monotonicity | 1000 | Always increasing |
| Work-stealing fairness | 1000 | Even distribution ±10% |
| CAS retry bounded | 1000 | Max 10 retries |
| No livelock | 1000 | Always makes progress |
| Steal from multiple threads | 8 threads × 100 ops | All work stolen |
| Mixed push/pop/steal | 8 threads × 1000 ops | Consistent state |

### Serialization (15 tests)

| Property | Cases | Validation |
|----------|-------|------------|
| Q8_8 roundtrip | 1000 random | deserialize(serialize(x)) == x |
| Q16_16 roundtrip | 1000 random | Same |
| Q32_32 roundtrip | 1000 random | Same |
| Precision preservation | 1000 | Within 1 unit of precision |
| Hash consistency | 1000 | Same value → same hash |
| Decimal precision bounded | 1000 | Error < 1 unit |
| Overflow saturates | 1000 | Max/min bounds |
| Negative handling | 1000 | Sign preserved |
| Zero handling | 1000 | Exact zero |
| Large value handling | 1000 | No overflow for in-range |
| Edge cases (max/min) | 100 | Correct saturation |
| Concurrent serialize | 8 threads × 100 | Thread-safe |
| Deterministic binary | 1000 | Same bytes always |
| Deterministic decimal | 1000 | Same string always |
| Hash distribution | 10000 | Low collision rate |

### Persistent Storage (25 tests)

#### PersistentMap Properties (12 tests)
| Property | Concurrency | Assertion |
|----------|-------------|-----------|
| Entry count accuracy | 8 threads × 1000 inserts | Exact 8000 |
| Generation monotonic | 8 threads × 1000 ops | Always increasing |
| Load factor correctness | Sequential 0..1024 | Exact formula |
| Hash chain integrity | 1000 updates | No tampering |
| CAS retry no livelock | 8 threads × 1000 contended | Always completes |
| Concurrent insert/get | 8 writers, 8 readers | Linearizable |
| No lost updates | 8 threads × 100 same key | All updates visible |
| Bucket distribution | 10000 inserts | Even distribution |
| Collision handling | Sequential collisions | Linear probing works |
| Resize trigger | 0..MAX_LOAD_FACTOR | Correct threshold |
| Tombstone handling | Insert/delete/insert | Reuses slots |
| Recovery consistency | Simulate crash | State recoverable |

#### PersistentLog Properties (13 tests)
| Property | Concurrency | Assertion |
|----------|-------------|-----------|
| Append ordering | 8 threads × 1000 appends | Sequential offsets |
| Entry count accuracy | 8 threads × 1000 appends | Exact 8000 |
| Head advances | Sequential 1000 appends | Cumulative correct |
| Hash chain integrity | 1000 appends | Tamper-evident |
| CAS no livelock | 8 threads × 1000 contended | Always completes |
| Concurrent append | 8 threads × 1000 | No overwrites |
| Segment rotation | Append beyond segment | Triggers rotation |
| Capacity enforcement | Append until full | Deterministic failure |
| Iterator correctness | 1000 appends | All entries visible |
| Zero-copy read | 1000 reads | No allocation |
| Recovery correctness | Simulate crash mid-append | Partial entry discarded |
| Crash-safe durability | Crash before fsync | Last entry lost |
| Audit trail validation | 1000 appends | Hash chain valid |

---

## TIER 3: INTEGRATION TESTS (Q15-Q21) - 40 tests

### Focus
- Component interactions
- Realistic workflows
- Cross-tier composition
- End-to-end scenarios

### Parallel + Serialization (10 tests)

| Scenario | Workers | Data | Validation |
|----------|---------|------|------------|
| Parallel serialize batch results | 8 | 1000 Q16_16 values | All serialized correctly |
| Concurrent hash computation | 8 | 1000 values | Consistent hashes |
| Work-stealing + serialize | 8 | 10000 tasks | All results correct |
| Mixed push/serialize | 8 | Variable workload | No data races |
| Batch serialization | 1 | 10000 Q16_16 | Throughput >1M/s |
| SIMD batch serialize | 1 | 10000 SimdFixedPoint | 2-4× speedup |
| Parallel decimal format | 8 | 1000 values | Thread-safe |
| Parallel hash chains | 8 | 1000 updates | Deterministic |
| Error handling | 8 | Invalid inputs | Graceful degradation |
| Recovery after failure | 8 | Simulate panic | State consistent |

### Serialization + Persistence (10 tests)

| Workflow | Components | Validation |
|----------|-----------|------------|
| Serialize to PersistentLog | Q16_16 → Log | Roundtrip works |
| Deserialize from PersistentMap | Map → Q16_16 | Zero-copy read |
| Audit trail validation | Log chain | Hash chain correct |
| Concurrent write + serialize | 8 threads | Linearizable |
| Batch append + serialize | 1000 entries | Throughput >1M/s |
| Recovery from log | Cold start | All entries restored |
| Tamper detection | Modify entry | Hash mismatch |
| Crash recovery | Mid-write crash | Consistent state |
| Segment rotation + serialize | Multi-segment | All data preserved |
| Mixed types (Q8_8 + Q16_16) | PersistentLog | Type-safe |

### Multi-Component (20 tests)

| Integration | Components | Scenario | Expected |
|------------|-----------|----------|----------|
| Map + Log together | PMap + PLog | Real workflow | Both consistent |
| Crash recovery coordination | Map + Log | Simultaneous crash | Both recover |
| Generation counter sync | Map + Log | Cross-component | Consistent ordering |
| Cross-tier capsule T5+T9 | Log (T5+T9) | Streaming + persistent | Zero-copy streaming |
| Cross-tier capsule T1+T5 | Queue + Log | Parallel + persistent | Work-stealing + audit |
| Parallel workers + persistence | Pool + Map | 8 workers, 10K inserts | All visible |
| Serialization pipeline | Queue → Serialize → Log | End-to-end | All data correct |
| Real-time analytics | Parallel + SIMD + Persist | Streaming aggregation | <1ms latency |
| Financial workflow | Q16_16 + Map + Log | P&L tracking | Audit trail complete |
| Crash-safe pipeline | All components | Crash at random point | Recoverable |
| Load balancing | Parallel queue | Work distribution | ±5% variance |
| Backpressure handling | Queue full → retry | Bounded backoff | No livelock |
| Graceful degradation | Component failure | Isolate failure | Other components ok |
| Hot reload | Replace component | Zero downtime | State preserved |
| Multi-tier composition | T1+T2+T3+T5+T9 | Complex workflow | All tiers cooperate |
| Error propagation | Chain of components | Error at source | Propagates correctly |
| Metrics collection | All components | Atomic counters | Accurate metrics |
| Performance monitoring | Integration test | Latency tracking | <100ns overhead |
| Resource cleanup | All components | Shutdown | No leaks |
| Integration stress | All components | High load | Stable |

---

## TIER 4: PRODUCTION TESTS (Q22-Q28) - 30 tests

### Focus
- Real-world workloads
- Stress testing
- Crash recovery
- Performance validation

### Parallel Module Stress (5 tests)

| Test | Workload | Target | Success Criteria |
|------|----------|--------|------------------|
| 1M work-stealing ops | 8 threads × 125K tasks | <5s total | All tasks execute |
| Previously hanging tests | 3 specific tests | <5s each | 100% pass rate |
| Deterministic P999 latency | 10K iterations | <2μs | Consistent |
| Memory safety | 1M ops with AddressSanitizer | 0 errors | No SIGSEGV |
| Sustained throughput | 10M tasks, 1 minute | >100K tasks/s | Stable |

### Serialization Stress (5 tests)

| Test | Operations | Validation | Target |
|------|-----------|------------|--------|
| 100K mixed decimal/binary | Q16_16 serialize/deserialize | No errors | <10ms |
| Precision at scale | 100K random values | Error bounded | <1 unit |
| No memory leaks | 1M serialize ops | Valgrind clean | 0 leaks |
| Concurrent serialize stress | 8 threads × 10K | Thread-safe | 0 races |
| Hash collision rate | 1M unique values | Low collisions | <0.01% |

### Persistent Storage Stress (10 tests)

#### PersistentMap Stress (5 tests)
| Test | Workload | Performance | Durability |
|------|----------|-------------|------------|
| 1M random inserts | Sequential | <100ns/insert | P99 <200ns |
| 1M random gets | Sequential | <50ns/get | P99 <100ns |
| Mixed insert/get | 8 threads × 100K each | Linearizable | 0 lost updates |
| Crash during insert | Kill at random point | Recovery works | State consistent |
| Load factor enforcement | Insert to 90% | Triggers resize | Deterministic |

#### PersistentLog Stress (5 tests)
| Test | Workload | Performance | Durability |
|------|----------|-------------|------------|
| 1M appends | Batch 100K | <50ns/append | P99 <100ns |
| Concurrent append | 8 threads × 100K | No overwrites | All entries visible |
| Segment rotation | Multi-segment | Seamless | No data loss |
| Crash during append | Kill mid-write | Recovery works | Partial entry discarded |
| Iterator performance | 1M entries | Sequential scan | <10ms total |

### Crash Recovery (5 tests)

| Scenario | Crash Point | Recovery | Validation |
|----------|------------|----------|------------|
| Incomplete CAS (Map) | Mid-insert | Detects incomplete | State consistent |
| Incomplete append (Log) | Mid-write | Discards partial | Hash chain valid |
| Both Map + Log crash | Simultaneous | Coordinated recovery | Cross-component consistent |
| Generation counter reset | Corrupt generation | Detect tamper | Reject corrupted state |
| Hash chain break | Modify entry | Detect tamper | Integrity check fails |

### Recovery Tests (5 tests)

| Test | Data Size | Recovery Latency | Validation |
|------|-----------|------------------|------------|
| Cold start from 1GB mmap | 1GB file | <100ms | All data accessible |
| Incremental recovery | Append-only log | <10ms | Last known good state |
| Audit trail validation | 1M entries | <1s | Hash chain correct |
| Time-series reconstruction | 1 week of data | <500ms | All entries ordered |
| Corrupt entry handling | Random corruption | Detect + skip | Graceful degradation |

---

## Test Infrastructure

### Common Utilities (`tests/common/mod.rs`)

#### Fixtures
- `TempMmapFile`: Temporary mmap files with auto-cleanup
- `CrashSimulator`: Simulate process crashes for recovery testing
- `AllocationTracker`: Memory leak detection

#### Baselines
- `MutexCounter`: Baseline for comparison (3-10× slower)
- `RwLockCounter`: Baseline for comparison (3-4× slower)

#### Helpers
- `run_concurrent()`: Multi-threaded test execution
- `wait_for()`: Wait for async conditions
- `simple_benchmark()`: Performance measurement
- `generate_test_data()`: Deterministic test data (reproducible)

#### Assertions
- `assert_within_range()`: Range validation with tolerance
- `assert_approx_eq()`: Float approximate equality

### Test Organization

```
tests/
├── common/
│   └── mod.rs                     (Shared infrastructure, 400+ LOC)
├── unit_tests_v0_3_2.rs          (60 tests, 750+ LOC)
├── property_tests_v0_3_2.rs      (50 tests, 800+ LOC)
├── integration_v0_3_2.rs         (40 tests, 1000+ LOC)
├── production_tests_v0_3_2.rs    (30 tests, 1200+ LOC)
└── TEST_DESIGN_V0_3_2.md         (This document)
```

**Total Lines**: ~4,150 LOC across all test files

---

## Run Commands

### All Tests (180+ tests)
```bash
cargo test --lib \
  --test unit_tests_v0_3_2 \
  --test property_tests_v0_3_2 \
  --test integration_v0_3_2 \
  --test production_tests_v0_3_2 \
  --features "std,capsule-serialize,mmap-persistence" \
  --release
```

### Individual Tiers
```bash
# Tier 1: Unit (60 tests, <1s)
cargo test --lib --test unit_tests_v0_3_2

# Tier 2: Property (50 tests, <10s)
cargo test --lib --test property_tests_v0_3_2

# Tier 3: Integration (40 tests, <30s)
cargo test --lib --test integration_v0_3_2

# Tier 4: Production (30 tests, <60s release)
cargo test --lib --test production_tests_v0_3_2 --release
```

### Determinism Validation
```bash
cargo test --lib -- --test-threads=1
```

### Memory Safety
```bash
cargo test --lib --features "std" -- --nocapture
valgrind --leak-check=full cargo test --lib --release
```

---

## Success Criteria

### Correctness
- ✅ 100% pass rate on all 180+ tests
- ✅ Zero memory leaks (Valgrind clean)
- ✅ Zero data races (ThreadSanitizer clean)
- ✅ Deterministic (same seed → same output)

### Performance
- ✅ Unit tests: <1 second total (debug mode)
- ✅ Property tests: <10 seconds total
- ✅ Integration tests: <30 seconds total
- ✅ Production tests: <60 seconds total (release mode)
- ✅ **Total**: <2 minutes for full suite (release)

### Coverage
- ✅ All T28 questions (Q1-Q28) covered
- ✅ All ASSUM assumptions verified
- ✅ All I20 integration questions validated
- ✅ UCE34 Q29 (Comprehensive Testing) satisfied

---

## Framework Compliance

### T28 (Testing Framework)
- **Q1-Q7 (Unit)**: 60 tests ✓
- **Q8-Q14 (Property)**: 50 tests ✓
- **Q15-Q21 (Integration)**: 40 tests ✓
- **Q22-Q28 (Production)**: 30 tests ✓

### ASSUM (Safety Framework)
- Every assumption in code verified by test
- Generation counter ABA prevention validated
- Memory ordering correctness validated
- Hash chain integrity verified

### I20 (Integration Framework)
- All 20 integration questions answered
- Component interactions validated
- Failure modes tested
- Recovery scenarios covered

### UCE34 (Systematic Discovery)
- **Q29**: Comprehensive testing (this document)
- **Q30**: Validation Foundation (all tests)
- **Q33**: Verification macros (compile-time + runtime)
- **Q34**: Auditability (hash chains, audit trails)

---

## Implementation Status

### Completed
- ✅ Test design document (this file)
- ✅ Common test infrastructure (`tests/common/mod.rs`)
- ✅ Unit tests skeleton (`tests/unit_tests_v0_3_2.rs`)

### In Progress
- ⏳ Property tests (`tests/property_tests_v0_3_2.rs`)
- ⏳ Integration tests (`tests/integration_v0_3_2.rs`)
- ⏳ Production tests (`tests/production_tests_v0_3_2.rs`)

### Blocked
- ⚠️ Source code compilation errors (persistent_log.rs, MmapError enum)
- ⚠️ Missing methods (some tests reference non-existent methods)

---

## Next Steps

1. **Fix Source Code Issues**
   - Resolve MmapError enum compilation errors
   - Ensure all methods referenced by tests exist

2. **Implement Remaining Test Tiers**
   - Property tests (50 tests, 800+ LOC)
   - Integration tests (40 tests, 1000+ LOC)
   - Production tests (30 tests, 1200+ LOC)

3. **Run Full Test Suite**
   - Execute all 180+ tests
   - Validate 100% pass rate
   - Generate coverage report

4. **Performance Validation**
   - Benchmark critical paths
   - Validate <2 minute total runtime
   - Ensure deterministic results

5. **Framework Compliance Report**
   - Document T28 coverage
   - Document ASSUM verification
   - Document I20 validation
   - Document UCE34 Q29-Q34 satisfaction

---

## Deliverables

1. ✅ **Test Design Document** (`tests/TEST_DESIGN_V0_3_2.md`)
   - Complete 4-tier test pyramid design
   - 180+ test specifications
   - Framework compliance mapping

2. ✅ **Common Test Infrastructure** (`tests/common/mod.rs`)
   - Shared utilities, fixtures, baselines
   - 400+ LOC reusable infrastructure

3. ⏳ **Test Implementation**
   - Unit tests (60 tests, 750+ LOC)
   - Property tests (50 tests, 800+ LOC)
   - Integration tests (40 tests, 1000+ LOC)
   - Production tests (30 tests, 1200+ LOC)

4. ⏳ **Coverage Report** (`tests/coverage.txt`)
   - Summary of all 180+ tests
   - Pass/fail status for each tier
   - Performance metrics (runtime, memory)
   - Framework compliance validation

---

## Conclusion

This comprehensive test suite provides:
- **Systematic Coverage**: All 4 T28 tiers (Q1-Q28)
- **Real-World Validation**: Production workloads, crash recovery
- **Framework Compliance**: T28, ASSUM, I20, UCE34
- **Reproducibility**: Deterministic test data, documented expectations
- **Maintainability**: Shared infrastructure, clear organization

**Total Value**: 180+ tests ensuring correctness, performance, and safety of v0.3.2 Phase 2 features (parallel queue fix, serialization, persistent storage).

---

**Document Version**: 1.0
**Last Updated**: 2025-10-22
**Author**: Testing & Validation Expert
**Framework**: T28 (4-Tier Test Pyramid)
