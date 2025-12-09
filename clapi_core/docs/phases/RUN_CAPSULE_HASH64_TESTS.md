# Running CapsuleHash64 Test Suite

**Quick Start Guide for T28 Comprehensive Test Suite**

---

## Test Suite Overview

- **Total Tests**: 95+ tests across 4 tiers
- **Total Lines**: 2,508 lines
- **Framework**: T28 Comprehensive Testing (28 questions, 4 tiers)
- **Status**: ✅ Ready for implementation validation

---

## Quick Commands

### Run All Fast Tests (<30s)
```bash
# All unit + property + integration tests
cargo test capsule_hash64 --tests
```

### Run Specific Tier
```bash
# Tier 1: Unit tests (50+ tests)
cargo test --test capsule_hash64_unit_tests

# Tier 2: Property tests (15+ tests)
cargo test --test capsule_hash64_property_tests

# Tier 3: Integration tests (20+ tests)
cargo test --test capsule_hash64_integration_tests

# Tier 4: Stress tests (10+ tests, requires --ignored)
cargo test --test capsule_hash64_stress_tests -- --ignored
```

### Run Specific Test
```bash
# Example: Run collision resistance test
cargo test --test capsule_hash64_property_tests property_no_collisions_1m -- --nocapture

# Example: Run concurrent hammering stress test
cargo test --test capsule_hash64_stress_tests stress_concurrent_hammering -- --ignored --nocapture
```

---

## Test Tiers Explained

### Tier 1: Unit Tests (50+ tests, <5s)
**Purpose**: Validate individual components in isolation

**Coverage**:
- Core behaviors (hash compute, store, load)
- Edge cases (zero, MAX, empty arrays)
- Invariants (determinism, incremental correctness)
- Code path coverage (all public methods)
- Isolation (no shared state, deterministic)
- Performance (<50ns per hash)
- Readability (arrange-act-assert)

**Run**:
```bash
cargo test --test capsule_hash64_unit_tests
```

**Expected Output**:
```
test test_capsule_size_and_alignment ... ok
test test_new_zero_hash ... ok
test test_compute_hash_single_field ... ok
...
test result: ok. 50 passed; 0 failed
```

---

### Tier 2: Property Tests (15+ tests, <10s)
**Purpose**: Validate invariants hold across input space

**Coverage**:
- Universal properties (collision resistance, bit flip detection)
- Concurrent properties (no race conditions, atomic consistency)
- Edge case properties (boundary values, large arrays)
- ASSUM verification (Relaxed ordering safe, XOR invertible)
- Composition (hash chain correctness)
- Statistical properties (entropy, Hamming distance)

**Run**:
```bash
cargo test --test capsule_hash64_property_tests
```

**Expected Output**:
```
test property_no_collisions_1m ... ok
✅ No collisions in 1000000 iterations (64-bit hash space)
test property_bit_flip_detection ... ok
✅ Detected 256/256 bit flips (100% detection rate)
...
test result: ok. 15 passed; 0 failed
```

---

### Tier 3: Integration Tests (20+ tests, <15s)
**Purpose**: Validate components work together

**Coverage**:
- Critical integration points (hash updates, verification)
- Error propagation (hash mismatches, corruption detection)
- Performance budgets (<100ns verification)
- Production load (10K operations)
- Rollback scenarios (manual reset, fallback)
- I20 validation (backward compatibility)
- Monitoring integration (mismatch tracking)

**Run**:
```bash
cargo test --test capsule_hash64_integration_tests
```

**Expected Output**:
```
test integration_verify_integrity_performance ... ok
Integrity verification: 80ns average
test integration_10k_deductions_with_hash ... ok
✅ 10K operations with hash verification (100% integrity)
...
test result: ok. 20 passed; 0 failed
```

---

### Tier 4: Stress Tests (10+ tests, 1-5 minutes)
**Purpose**: Ensure production readiness under extreme conditions

**Coverage**:
- Stress tests (1M+ operations, 100 threads)
- Adversarial tests (collision attempts, extreme patterns)
- B32 benchmark validation (fair baselines)
- ASSUM validation under stress (Relaxed ordering)
- TODO resolution (no outstanding issues)
- Documentation completeness
- Maintainability (CI/CD readiness)

**Run** (requires `--ignored` flag):
```bash
# Run all stress tests
cargo test --test capsule_hash64_stress_tests -- --ignored

# Run with output
cargo test --test capsule_hash64_stress_tests -- --ignored --nocapture
```

**Expected Output**:
```
test stress_concurrent_hammering ... ok
Starting concurrent hammering: 100 threads × 10000 ops
✅ Stress test passed: 1000000 ops in 1.234s
   Throughput: 810372 ops/sec

test stress_hash_chain_1m_operations ... ok
✅ Hash chain stress: 1000000 iterations in 0.876s

...
test result: ok. 10 passed; 0 failed
```

---

## CI/CD Integration

### Fast CI Pipeline (<30s)
```yaml
# .github/workflows/ci.yml
name: Fast Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run fast tests
        run: cargo test capsule_hash64 --tests
```

### Nightly Stress Testing (1-5 minutes)
```yaml
# .github/workflows/nightly.yml
name: Stress Tests
on:
  schedule:
    - cron: '0 2 * * *'  # 2 AM daily
jobs:
  stress:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run stress tests
        run: cargo test capsule_hash64 -- --ignored --nocapture
```

---

## Test Output Examples

### Successful Test Run
```
running 95 tests
test test_capsule_size_and_alignment ... ok
test test_new_zero_hash ... ok
test test_compute_hash_single_field ... ok
test property_no_collisions_1m ... ok
✅ No collisions in 1000000 iterations
test integration_verify_integrity_performance ... ok
Integrity verification: 80ns average
...

test result: ok. 95 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Stress Test Output (with `--nocapture`)
```
test stress_concurrent_hammering ...
Starting concurrent hammering: 100 threads × 10000 ops
✅ Stress test passed: 1000000 ops in 1.234s
   Throughput: 810372 ops/sec
ok

test stress_hash_chain_1m_operations ...
Starting hash chain stress: 1000000 iterations
✅ Hash chain stress: 1000000 iterations in 0.876s
   Final hash: 0x123456789ABCDEF0
ok
```

---

## Troubleshooting

### Issue: Tests fail with "cannot find module `capsule_hash64`"
**Solution**: Implement `CapsuleHash64` in `src/capsule_hash64.rs` first
```bash
# Check if implementation exists
ls src/capsule_hash64.rs
```

### Issue: Stress tests take too long
**Solution**: Stress tests are ignored by default, run selectively
```bash
# Run only specific stress test
cargo test --test capsule_hash64_stress_tests stress_concurrent_hammering -- --ignored
```

### Issue: Property tests fail with collisions
**Solution**: This would indicate a hash algorithm bug (expected: 0 collisions)
```bash
# Re-run with output to see collision details
cargo test --test capsule_hash64_property_tests property_no_collisions_1m -- --nocapture
```

---

## Performance Expectations

### Fast Tests (Tier 1-3)
- **Unit tests**: <5 seconds (50+ tests)
- **Property tests**: <10 seconds (15+ tests, includes 1M collision check)
- **Integration tests**: <15 seconds (20+ tests)
- **Total**: <30 seconds (suitable for CI)

### Stress Tests (Tier 4)
- **Concurrent hammering**: ~1-2 seconds (1M ops)
- **Hash chain stress**: ~1 second (1M sequential ops)
- **Incremental stress**: ~0.5 seconds (1M updates)
- **Memory stress**: ~10-20 seconds (10K capsules × 10K ops)
- **Long-running**: ~30-60 seconds (10M ops)
- **Total**: 1-5 minutes (nightly CI only)

---

## Test Validation Checklist

Before deployment, ensure:

- [ ] All unit tests pass (50/50)
- [ ] All property tests pass (15/15)
- [ ] All integration tests pass (20/20)
- [ ] No collisions detected in 1M inputs
- [ ] 100% bit flip detection (256/256)
- [ ] Performance budgets met (<100ns verification)
- [ ] Stress tests pass (10/10, run with `--ignored`)
- [ ] No panics in concurrent tests (100 threads × 10K ops)
- [ ] Documentation complete (all public APIs)

---

## Additional Resources

- **Delivery Summary**: [CAPSULE_HASH64_TEST_SUITE_DELIVERY.md](./CAPSULE_HASH64_TEST_SUITE_DELIVERY.md)
- **Design Document**: [UCE33_CAPSULE_HASH64_ANALYSIS.md](./UCE33_CAPSULE_HASH64_ANALYSIS.md)
- **T28 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/T28_TESTING_FRAMEWORK.md`
- **Reference Tests**: [tests/budget_metacapsule_unit_tests.rs](./tests/budget_metacapsule_unit_tests.rs)

---

**Test Suite**: CapsuleHash64 T28 Comprehensive Testing
**Total Tests**: 95+ (50 unit, 15 property, 20 integration, 10 stress)
**Total Lines**: 2,508 lines (4 test files)
**Framework**: T28 (28 questions, 4 tiers)
**Status**: ✅ Ready for implementation validation
