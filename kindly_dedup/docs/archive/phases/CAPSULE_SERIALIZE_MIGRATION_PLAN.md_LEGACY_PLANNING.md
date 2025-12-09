# CapsuleSerialize Test Migration Plan - kindly_dedup

**Status**: PENDING (Awaiting Agent 1-4 Type Migration)

**Tier**: T0 (Auditable Foundation)

**Framework**: UCE34 (Q1-Q34), ASSUM (99.99% safe), I20 (integration validation)

## Summary

This document outlines the complete migration strategy for all 13 test files in `/home/samuel/Primitives/kindly_dedup/tests/` from `serde_json` serialization to `CapsuleSerialize` trait.

**Blocker**: Agents 1-4 must complete type migration to `#[derive(CapsuleSerialize)]` FIRST.

**Once Types are Ready**:
- Expected migration time: 45 minutes (parallel application across 13 files)
- Expected test impact: 0 failures (pure serialization API change)
- Expected loc changes: ~250 lines (serde_json → capsule serialize)

## Current Serialization Usage

### Test Files Using serde_json (13 total)

1. `tests/audit_unit_tests.rs` - 11 serde_json calls
2. `tests/audit_property_tests.rs` - 6 serde_json calls
3. `tests/audit_integration_tests.rs` - 8 serde_json calls
4. `tests/audit_production_tests.rs` - 4 serde_json calls
5. `tests/cli_unit_tests.rs` - 3 serde_json calls
6. `tests/integration_tests.rs` - 5 serde_json calls
7. `tests/server_tests.rs` - 7 serde_json calls
8. `tests/demo_production_tests.rs` - 2 serde_json calls
9. `tests/v1_0_benchmark_tests.rs` - 4 serde_json calls
10. `tests/protection_integration_tests.rs` - 3 serde_json calls
11. `tests/dataset_manager_tests.rs` - 2 serde_json calls
12. `tests/week2_format_integration_tests.rs.skip` - 5 serde_json calls (skipped)
13. `tests/b32_runner_tests.rs` - 3 serde_json calls

**Total serde_json usage**: ~63 calls across 13 files

### Types Requiring Migration

Types that will have `#[derive(CapsuleSerialize)]` added by Agents 1-4:

1. **BenchmarkAuditEntry** (audit_logger.rs)
   - Fields: benchmark_id, timestamp, environment, config, input_hash, result, result_hash, prev_audit_hash, audit_hash
   - Current serialization: 11 calls
   - Migration pattern: `serde_json::to_string(&entry)` → `entry.to_json()?`

2. **BenchmarkConfig** (audit_logger.rs)
   - Fields: dataset, threads, features, warmup_iterations, measurement_iterations
   - Current serialization: 8 calls
   - Migration pattern: `serde_json::to_string(&config)` → `config.to_json()?`

3. **BenchmarkResult** (audit_logger.rs)
   - Fields: throughput_docs_per_sec, latency_p50_us, latency_p95_us, latency_p99_us, latency_mean_us, latency_stddev_us, ci_95_lower_us, ci_95_upper_us, accuracy
   - Current serialization: 15 calls
   - Migration pattern: `serde_json::to_string(&result)` → `result.to_json()?`

4. **AccuracyMetrics** (audit_logger.rs)
   - Fields: recall, precision, f1, true_positives, false_positives, true_negatives, false_negatives
   - Current serialization: 8 calls
   - Migration pattern: `serde_json::to_string(&accuracy)` → `accuracy.to_json()?`

5. **EnvironmentInfo** (environment.rs)
   - Fields: rustc_version, cpu_model, cpu_cores, os_version, feature_flags, git_commit, git_dirty
   - Current serialization: 7 calls
   - Migration pattern: `serde_json::to_string(&env)` → `env.to_json()?`

## Migration Pattern

### Before (serde_json)

```rust
// Serialization (to string)
let json = serde_json::to_string(&entry)?;
assert_eq!(json, expected);
assert!(json.contains("benchmark_id"));

// Serialization (to bytes)
let bytes = serde_json::to_vec(&config)?;

// Deserialization
let deserialized: BenchmarkConfig = serde_json::from_str(&json)?;
```

### After (CapsuleSerialize)

```rust
// Serialization (to JSON string - NEW)
let json = entry.to_json()?;
assert_eq!(json, expected);
assert!(json.contains("\"benchmark_id\""));

// Serialization (to deterministic bytes)
let bytes = config.serialize_deterministic();

// Deserialization
let deserialized = BenchmarkConfig::deserialize_from_bytes(&bytes)?;

// Hash integration (bonus - not in serde_json)
let hash = result.serialize_for_hash();
```

## File-by-File Migration Schedule

### Phase 1: Core Audit Tests (5 files)

1. **tests/audit_unit_tests.rs** - 11 serde_json → 11 CapsuleSerialize
   - Lines affected: 82-88 (serialization), 94-97 (deserialization), 151 (assertion), 173-174, 187-188, 220-221, 237, 338-339, 365-366, 415-416, 503

2. **tests/audit_property_tests.rs** - 6 serde_json → 6 CapsuleSerialize
   - Lines affected: ~50-100 (property tests)

3. **tests/audit_integration_tests.rs** - 8 serde_json → 8 CapsuleSerialize
   - Lines affected: Hash chain verification, concurrent logging

4. **tests/audit_production_tests.rs** - 4 serde_json → 4 CapsuleSerialize
   - Lines affected: Large-scale audit trail validation

5. **tests/b32_runner_tests.rs** - 3 serde_json → 3 CapsuleSerialize
   - Lines affected: B32 benchmark validation

### Phase 2: Integration Tests (4 files)

6. **tests/integration_tests.rs** - 5 serde_json → 5 CapsuleSerialize
   - Lines affected: Core pipeline integration

7. **tests/server_tests.rs** - 7 serde_json → 7 CapsuleSerialize
   - Lines affected: HTTP server response validation

8. **tests/demo_production_tests.rs** - 2 serde_json → 2 CapsuleSerialize
   - Lines affected: Demo validation

9. **tests/cli_unit_tests.rs** - 3 serde_json → 3 CapsuleSerialize
   - Lines affected: CLI output tests

### Phase 3: Special Cases (3 files)

10. **tests/protection_integration_tests.rs** - 3 serde_json → 3 CapsuleSerialize
    - Lines affected: Protection mechanism validation

11. **tests/dataset_manager_tests.rs** - 2 serde_json → 2 CapsuleSerialize
    - Lines affected: Dataset serialization

12. **tests/v1_0_benchmark_tests.rs** - 4 serde_json → 4 CapsuleSerialize
    - Lines affected: Benchmark result validation

13. **tests/week2_format_integration_tests.rs.skip** - 5 serde_json → 5 CapsuleSerialize
    - Status: Currently skipped, migrate for future activation

## Expected Changes Summary

| Category | Before | After | Delta |
|----------|--------|-------|-------|
| Total serde_json calls | 63 | 0 | -63 |
| Total to_json() calls | 0 | ~45 | +45 |
| Total serialize_deterministic() calls | 0 | ~10 | +10 |
| Total serialize_for_hash() calls | 0 | ~8 | +8 |
| Test files affected | 13 | 13 | 0 |
| Expected LOC changes | - | ~250 | ±0 (neutral) |
| Expected test failures | - | 0 | 0 |
| Expected speedup | - | 2-5× (binary serialization) | TBD |

## Key Migration Decisions

### 1. Dual Serialization (serde + CapsuleSerialize)

Both will coexist during transition:
- **serde_json**: HTTP APIs, CLI output (text-based, human-readable)
- **CapsuleSerialize**: Hash chains, audit trails, deterministic serialization

Most types will have BOTH `#[derive(Serialize, Deserialize, CapsuleSerialize)]`

### 2. Error Handling Consistency

Current:
```rust
let json = serde_json::to_string(&entry)?;  // Returns Result<String, Error>
```

After:
```rust
let json = entry.to_json()?;  // Returns Result<String, Error>
```

Error types should be compatible (both return `Result`).

### 3. Test Assertions

**String content checks** (currently common):
```rust
// Before
assert!(json.contains("benchmark_id"));

// After
assert!(json.contains("\"benchmark_id\""));  // JSON format with quotes
```

### 4. Performance Validation (B32 Framework)

Once migration complete, validate:
- `to_json()` call latency: <10μs per type (vs serde_json baseline)
- `serialize_deterministic()` throughput: >100K ops/sec
- Hash chain generation: <1ns per capsule

## Blockers & Dependencies

### Hard Blocker
- **Agent 1-4 Type Migration**: MUST complete `#[derive(CapsuleSerialize)]` on all 5 types before test migration

### Soft Dependencies
- None (test changes are purely additive, no breaking changes)

## Validation Checklist

Once type migration complete:

- [ ] All 13 test files compile without warnings
- [ ] All 63 serde_json → CapsuleSerialize migrations complete
- [ ] Zero test failures introduced
- [ ] B32 performance targets achieved (<10μs per to_json())
- [ ] T28 test coverage unchanged (same assertions, different API)
- [ ] I20 integration validation (20/20 questions) passed
- [ ] ASSUM safety verified (99.99% target maintained)
- [ ] Git commit created with [TRADE SECRET] tag

## Implementation Steps (Once Types Ready)

1. **Verify Type Readiness** (2 min)
   ```bash
   grep -r "derive.*CapsuleSerialize" src/
   # Should show: BenchmarkAuditEntry, BenchmarkConfig, BenchmarkResult, AccuracyMetrics, EnvironmentInfo
   ```

2. **Add Import** (1 min per file)
   ```rust
   use atomic_capsule::serialize::CapsuleSerialize;
   ```

3. **Migrate serde_json Calls** (40 min total)
   - Replace `serde_json::to_string(&x)` with `x.to_json()?`
   - Replace `serde_json::to_vec(&x)` with `x.serialize_deterministic()`
   - Replace `serde_json::from_str(&json)` with `Type::deserialize_from_bytes(&bytes)?`

4. **Update Assertions** (2 min per file)
   - Add quotes to JSON string checks: `.contains("field")` → `.contains("\"field\"")`

5. **Verify Tests** (3 min)
   ```bash
   cargo test --lib --all-features
   ```

6. **Commit** (1 min)
   ```bash
   git add tests/
   git commit -m "[TRADE SECRET] refactor(serialize): Migrate tests to CapsuleSerialize"
   ```

## Success Criteria

- All 13 test files migrated
- All 63 serde_json calls replaced
- 0 test failures
- 0 new compiler warnings
- <2× slowdown on test suite (expected: same speed due to binary serialization)
- Git commit created

## Timeline Estimate

Once types are ready:
- **Parallel execution**: 45 minutes (all 13 files simultaneously)
- **Sequential execution**: 90 minutes (file-by-file)
- **Verification**: 15 minutes (compile, test, review)
- **Total**: 1.5-2.5 hours end-to-end

## Post-Migration Opportunities

Once CapsuleSerialize is integrated:

1. **Hash Chain Audit Trails** (Q34 compliance)
   - Use `serialize_for_hash()` to create deterministic audit chains
   - Add tampering detection in property tests

2. **Binary Compression** (T9 Persistent)
   - Replace JSON audit logs with binary format
   - Expected: 50-70% size reduction

3. **Performance Benchmarking** (B32 validation)
   - Measure serialization latency vs serde_json
   - Expected: 2-5× faster for deterministic binary

4. **Concurrent Audit Logging** (Chaos patterns)
   - Combine atomic snapshots with CapsuleSerialize
   - Enable lock-free audit trail generation

## References

- **CapsuleSerialize Trait**: `/home/samuel/Primitives/atomic_capsule/src/serialize/mod.rs`
- **Types Location**: `/home/samuel/Primitives/kindly_dedup/src/benchmarking/`
- **Test Files**: `/home/samuel/Primitives/kindly_dedup/tests/`
- **Framework**: UCE34 (Q1-Q34 systematic discovery)
- **Safety Target**: ASSUM 99.99% (all assumptions documented)

---

**Status**: WAITING FOR AGENT 1-4 TYPE MIGRATION

**Next Step**: Once types have `#[derive(CapsuleSerialize)]`, execute migration plan above.
