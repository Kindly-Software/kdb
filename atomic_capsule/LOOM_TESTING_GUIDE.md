# Loom Memory Model Verification Guide

## Overview

This document describes how to run and interpret Loom tests for the `atomic_capsule` project. Loom is a model checker that exhaustively explores thread interleavings to catch memory ordering bugs that are impossible to reproduce with standard runtime tests.

## Why Loom?

### x86 vs ARM Memory Models

| CPU Architecture | Memory Model | Characteristics | Risk |
|------------------|--------------|-----------------|------|
| **x86/x86_64** | Total Store Order (TSO) | Strong ordering, stores appear in program order | Code works on x86 but FAILS on ARM |
| **ARM/RISC-V** | Weak Memory | Loads/stores can reorder without barriers | Requires explicit Acquire/Release |
| **PowerPC** | Weak Memory | Similar to ARM, aggressive reordering | Production systems may fail |

**Critical Insight**: Code that works perfectly on x86 (your development laptop) may have data races on ARM servers (AWS Graviton, Apple M1/M2/M3). Loom catches these bugs **before** production deployment.

## What Loom Tests Validate

### 1. Memory Ordering Bugs (Most Critical)

**Bug**: Missing Acquire/Release ordering
```rust
// ❌ WRONG (works on x86, breaks on ARM)
data.store(42, Ordering::Relaxed);
flag.store(1, Ordering::Relaxed);  // No synchronization!

// ✅ CORRECT (works on all CPUs)
data.store(42, Ordering::Relaxed);
flag.store(1, Ordering::Release);  // Synchronizes data write
```

**Loom detects**: Reader may see `flag=1` but `data=0` (stale read on ARM).

### 2. TOCTOU (Time-Of-Check-To-Time-Of-Use) Races

**Bug**: Reading value twice without synchronization
```rust
// ❌ WRONG
if map.get(key).is_some() {
    // Another thread may remove key HERE
    let value = map.get(key).unwrap(); // Panic!
}

// ✅ CORRECT (generation counter)
let gen1 = entry.generation.load(Ordering::Acquire);
let value = entry.value.load(Ordering::Acquire);
let gen2 = entry.generation.load(Ordering::Acquire);
if gen1 == gen2 { /* value is consistent */ }
```

**Loom detects**: Concurrent modification between check and use.

### 3. Torn Reads (Multi-field Atomicity)

**Bug**: Reading multi-field struct without atomicity
```rust
// ❌ WRONG (two separate atomic loads)
let a = struct.field_a.load(Ordering::Acquire);
let b = struct.field_b.load(Ordering::Acquire);
// Reader may see a=new, b=old (partial update)

// ✅ CORRECT (packed single atomic or generation counter)
let packed = struct.packed_value.load(Ordering::Acquire);
let (a, b) = unpack(packed); // Atomic read
```

**Loom detects**: Partial updates visible to readers.

### 4. ABA Problem Prevention

**Bug**: CAS succeeds but value changed in between
```rust
// ❌ WRONG (ABA: ptr goes A → B → A, CAS succeeds)
ptr.compare_exchange(A, new_value, ...);

// ✅ CORRECT (generation counter prevents ABA)
let gen_ptr = packed(generation, ptr);
gen_ptr.compare_exchange(old_gen_ptr, new_gen_ptr, ...);
```

**Loom detects**: CAS false positives due to recycled values.

## Running Loom Tests

### Quick Start (3 Preemptions, ~10 seconds)

```bash
# Run all Loom tests (default: 3 preemptions)
RUSTFLAGS="--cfg loom" cargo test --test loom_tests

# Run specific test
RUSTFLAGS="--cfg loom" cargo test --test loom_tests loom_concurrent_map_insert_get
```

**Output Example**:
```
running 11 tests
test loom_concurrent_map_insert_get ... ok (explored 127 states)
test loom_generation_counter_toctou ... ok (explored 89 states)
test loom_linear_probing_collision ... ok (explored 215 states)
...
test result: ok. 11 passed; 0 failed
```

### Thorough Testing (10 Preemptions, ~5 minutes)

```bash
# More thorough (explores more interleavings)
LOOM_MAX_PREEMPTIONS=10 RUSTFLAGS="--cfg loom" cargo test --test loom_tests
```

**Trade-off**: Higher preemptions = more thorough but slower (exponential growth).

### CI Integration (Recommended: 5 Preemptions)

```bash
# CI mode (balance thoroughness vs time)
LOOM_MAX_PREEMPTIONS=5 LOOM_LOG=info RUSTFLAGS="--cfg loom" cargo test --test loom_tests
```

**CI Configuration** (`.github/workflows/loom.yml`):
```yaml
name: Loom Memory Model Verification

on: [push, pull_request]

jobs:
  loom:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - name: Run Loom tests
        run: |
          LOOM_MAX_PREEMPTIONS=5 \
          LOOM_LOG=info \
          RUSTFLAGS="--cfg loom" \
          cargo test --test loom_tests
```

## Understanding Loom Output

### Success (All States Explored)

```
test loom_concurrent_map_insert_get ... ok (explored 127 states)
```

**Meaning**: Loom explored 127 possible thread interleavings, all passed.

### Failure (Data Race Detected)

```
test loom_concurrent_map_insert_get ... FAILED

thread 'loom_concurrent_map_insert_get' panicked at 'data race detected'
note: unsynchronized access to memory location 0x7f8b4c0012a0
  - thread 1: store with Relaxed ordering
  - thread 2: load with Relaxed ordering
```

**Fix**: Change to Acquire/Release ordering.

### Failure (Assertion Failed)

```
test loom_generation_counter_toctou ... FAILED

assertion failed: result.is_none() || result == Some(100)
  left: Some(200)
 right: Some(100)
```

**Meaning**: Reader saw inconsistent state (TOCTOU bug).

## Test Coverage (11 Loom Tests)

| Test | Category | What It Catches | Importance |
|------|----------|-----------------|------------|
| `loom_concurrent_map_insert_get` | ConcurrentMap | Acquire/Release on insert/get | Critical |
| `loom_generation_counter_toctou` | TOCTOU | Missing generation checks | Critical |
| `loom_linear_probing_collision` | ConcurrentMap | Probe ordering bugs | High |
| `loom_ring_buffer_producer_consumer` | RingBuffer | Producer-consumer sync | Critical |
| `loom_ring_buffer_fifo` | RingBuffer | FIFO ordering violations | Critical |
| `loom_ring_buffer_wraparound` | RingBuffer | Index wrap corruption | High |
| `loom_lockfree_table_ptr_install` | LockfreeTable | AtomicPtr synchronization | Critical |
| `loom_lockfree_table_chaining` | LockfreeTable | Chained entry ordering | High |
| `loom_lockfree_table_concurrent_remove` | LockfreeTable | Double-free prevention | Critical |
| `loom_acquire_release_pairing` | Generic | Acquire/Release pairs | Critical |
| `loom_torn_read_prevention` | Generic | Multi-field atomicity | High |

**Coverage**: All 3 collections (ConcurrentMapCapsule, RingBufferBroadcast, LockfreeHashTable) + generic patterns.

## Performance Characteristics

| Preemptions | States Explored | Time | Use Case |
|-------------|-----------------|------|----------|
| 2 | ~50-200 | <5 sec | Development iteration |
| 3 (default) | ~100-500 | ~10 sec | Pre-commit hook |
| 5 | ~500-2000 | ~1 min | CI pipeline |
| 10 | ~5000-20000 | ~5 min | Nightly CI |
| 20+ | Exponential | Hours | Rare bugs (optional) |

**Recommendation**: Use 3 for development, 5 for CI, 10 for nightly builds.

## Debugging Loom Failures

### Step 1: Enable Logging

```bash
LOOM_LOG=trace RUSTFLAGS="--cfg loom" cargo test --test loom_tests loom_concurrent_map_insert_get
```

**Output**:
```
[TRACE] loom: exploring state 1 of ~127
  thread 1: executing line 42 (store with Release)
  thread 2: executing line 58 (load with Acquire)
[TRACE] loom: exploring state 2 of ~127
  thread 2: executing line 58 (load with Acquire)
  thread 1: executing line 42 (store with Release)
...
```

### Step 2: Identify Failing State

```
[ERROR] loom: state 87 failed
  thread 1: store(42, Relaxed) at line 42
  thread 2: load(Relaxed) at line 58
  result: assertion failed (expected 42, got 0)
```

**Fix**: Change `Relaxed` to `Release` (thread 1) and `Acquire` (thread 2).

### Step 3: Verify Fix

```bash
# Re-run with fix
RUSTFLAGS="--cfg loom" cargo test --test loom_tests loom_concurrent_map_insert_get
# Should pass all states
```

## Common Pitfalls

### 1. Using `std::sync` Instead of `loom::sync`

❌ **WRONG**:
```rust
use std::sync::atomic::{AtomicU64, Ordering};
```

✅ **CORRECT**:
```rust
#[cfg(loom)]
use loom::sync::atomic::{AtomicU64, Ordering};

#[cfg(not(loom))]
use std::sync::atomic::{AtomicU64, Ordering};
```

### 2. Infinite Loops (State Explosion)

❌ **WRONG**:
```rust
loop {
    // Loom explores ALL possible loop iterations (infinite)
}
```

✅ **CORRECT**:
```rust
for _ in 0..10 {
    // Bounded loop (Loom explores up to 10 iterations)
}
```

### 3. Complex Data Structures (Simplified Models)

❌ **WRONG**: Use production `ConcurrentMapCapsule` (16K slots, too many states)

✅ **CORRECT**: Use `SimpleLoomMap` (2 slots, minimal states)

## Integration with UCE34 Framework

### Q33 (Verification)

**UCE34 Q33**: "How do you verify correctness at compile-time and runtime?"

**Answer**: Loom provides exhaustive runtime verification of memory ordering correctness.

- **Compile-time**: `#[derive(ComputationalCapsule)]` (alignment, size)
- **Runtime**: Loom (memory ordering, TOCTOU, ABA)

### T28 (Testing)

**T28 Testing Framework**:
- **Tier 1 (Unit)**: Single-threaded tests
- **Tier 2 (Property)**: Multi-threaded stress tests
- **Tier 3 (Integration)**: **Loom exhaustive interleaving tests** ← This
- **Tier 4 (Production)**: Production monitoring

**Loom = Tier 3 Testing** (integration-level concurrency validation).

### ASSUM (Safety)

**ASSUM Framework Tags**:
```rust
// #ASSUME_ACQUIRE_RELEASE: Acquire/Release pairs synchronize correctly
// #VERIFY_ACQUIRE_RELEASE: Loom tests validate all pairs
```

**All ASSUM tags with memory ordering assumptions should have corresponding Loom tests.**

## Future Work

### Additional Tests (12-20 Total)

1. **SeqCst Ordering**: Test total ordering guarantees
2. **Fence Validation**: Explicit atomic fences
3. **Multi-producer Multi-consumer**: MPMC queue races
4. **Load-linked/Store-conditional**: LL/SC emulation bugs
5. **Hazard Pointers**: Safe memory reclamation

### Loom + Miri Integration

**Miri**: Detects undefined behavior (UB)
**Loom**: Detects memory ordering bugs

**Combined**:
```bash
# Miri (UB detection)
cargo +nightly miri test

# Loom (memory ordering)
RUSTFLAGS="--cfg loom" cargo test --test loom_tests
```

**Best Practice**: Run both in CI (Miri for UB, Loom for ordering).

## References

- [Loom Documentation](https://github.com/tokio-rs/loom)
- [ARM Memory Model](https://developer.arm.com/documentation/100941/0101/Memory-ordering)
- [Rust Atomics and Locks](https://marabos.nl/atomics/)
- [UCE34 Framework](../../../kindly-ecosystem/kindly-main/docs/frameworks/UCE34_FRAMEWORK.md)
- [ASSUM Safety](../../../kindly-ecosystem/kindly-main/docs/frameworks/ASSUM_SAFETY.md)

## Summary

**Loom testing is MANDATORY for all lockfree code.**

- **Why**: Catches ARM memory ordering bugs that x86 misses
- **When**: Run on every commit (CI), nightly builds (thorough)
- **How**: `RUSTFLAGS="--cfg loom" cargo test --test loom_tests`
- **Coverage**: 11 tests (ConcurrentMap, RingBuffer, LockfreeTable, Generic)
- **Cost**: ~10 seconds (3 preemptions), ~5 minutes (10 preemptions)

**Preventing production failures is worth 10 seconds of testing.**
