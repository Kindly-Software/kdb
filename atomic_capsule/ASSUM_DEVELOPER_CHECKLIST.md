# ASSUM Developer Checklist - atomic_capsule

Quick reference for developers to ensure ASSUM compliance when writing new code.

---

## ✅ Pre-Commit Checklist

Run this checklist before committing ANY code:

### 1. Unsafe Code (0 tolerance for undocumented unsafe)

```rust
// ❌ BAD: Undocumented unsafe
unsafe { *ptr = value; }

// ✅ GOOD: Documented unsafe
// SAFETY:
//   1. Pointer is valid: allocated in line 42, not freed
//   2. Pointer is aligned: guaranteed by allocator (8-byte align)
//   3. No data races: exclusive &mut access, no other references
//   4. Verification: Miri clean, no UB detected
unsafe { *ptr = value; }
```

**Checklist**:
- [ ] Every `unsafe { }` block has `// SAFETY:` comment above it
- [ ] SAFETY comment explains: (1) Why unsafe, (2) What invariants, (3) How verified
- [ ] Run `cargo miri test` on unsafe code
- [ ] Add `#ASSUME_TYPE_SAFE` + `#VERIFY_UNSAFE_INVARIANTS` tags

---

### 2. Error Handling (0 unwrap() in production code)

```rust
// ❌ BAD: Panics in production
let value = map.get(&key).unwrap();

// ✅ GOOD: Proper error handling
let value = map.get(&key).ok_or(MapError::KeyNotFound)?;

// ✅ ACCEPTABLE: Justified unwrap() (rare)
// #ASSUME_PANIC_SAFE: Key guaranteed by constructor invariant
// #VERIFY_NO_PANIC: Test covers all code paths, key always present
let value = map.get(&KNOWN_KEY).unwrap();
```

**Checklist**:
- [ ] Zero `unwrap()` calls in non-test code (use `?`, `ok_or()`, `expect()` with ASSUM)
- [ ] All `expect()` calls justified with clear message
- [ ] Test code marked with `#[cfg(test)]` or in `tests/` module
- [ ] Add `#ASSUME_PANIC_SAFE` + `#VERIFY_NO_PANIC` for justified unwrap()

---

### 3. ASSUM Framework (Every assumption needs verification)

```rust
// ❌ BAD: Missing #VERIFY
// #ASSUME_GENERATION_COUNTER: Counter prevents TOCTOU races
self.generation.fetch_add(1, Ordering::Release);

// ✅ GOOD: Complete ASSUM tags
// #ASSUME_GENERATION_COUNTER: 64-bit counter prevents TOCTOU (wraps after 2^64 ops)
// #VERIFY_GENERATION: Property test validates race detection (10K concurrent ops)
self.generation.fetch_add(1, Ordering::Release);
```

**Checklist**:
- [ ] Every `#ASSUME_*` tag has corresponding `#VERIFY_*` tag
- [ ] Verification is testable (property test, Miri, Loom, benchmarks)
- [ ] ASSUM category matches pattern (see 10 categories below)
- [ ] Verification actually runs (add to test suite)

---

### 4. Memory Ordering (Justify Relaxed, default to Acquire/Release)

```rust
// ❌ BAD: Unjustified Relaxed
counter.fetch_add(1, Ordering::Relaxed);

// ✅ GOOD: Justified Relaxed
// #ASSUME_MEMORY_ORDERING: Relaxed sufficient for independent statistics counter
// #VERIFY_ORDERING_SUFFICIENT: 15ns Relaxed vs 25ns SeqCst (40% faster, B32 validated)
counter.fetch_add(1, Ordering::Relaxed);

// ✅ SAFE DEFAULT: Acquire/Release when in doubt
state.store(new_value, Ordering::Release);
let value = state.load(Ordering::Acquire);
```

**Checklist**:
- [ ] All `Ordering::Relaxed` have performance justification (B32 benchmark)
- [ ] Load-then-store uses Acquire/Release or CAS loop (TOCTOU prevention)
- [ ] Add `#ASSUME_MEMORY_ORDERING` + `#VERIFY_ORDERING_SUFFICIENT` for Relaxed

---

### 5. Capsule Verification (Mandatory for all capsules)

```rust
// ❌ BAD: No verification
#[repr(C, align(64))]
pub struct MyCapsule {
    field: AtomicU64,
    _padding: [u8; 56],
}

// ✅ GOOD: Automatic verification (preferred)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct MyCapsule {
    field: AtomicU64,
    _padding: [u8; 56],
}

// ✅ GOOD: Manual verification (if derive not available)
#[repr(C, align(64))]
pub struct MyCapsule {
    field: AtomicU64,
    _padding: [u8; 56],
}
verify_capsule_properties!(MyCapsule, 64, 64);
```

**Checklist**:
- [ ] All capsules use `#[derive(ComputationalCapsule)]` OR `verify_capsule_properties!`
- [ ] Alignment matches tier: 64B (T1 hot), 128B (T1 warm), 256B (T1 cold)
- [ ] Size = alignment (padding to cache line boundary)

---

### 6. Send/Sync Safety (100% documentation required)

```rust
// ❌ BAD: Undocumented Send/Sync
unsafe impl Send for MyCapsule {}
unsafe impl Sync for MyCapsule {}

// ✅ GOOD: Documented Send/Sync
// #ASSUME_SEND_SYNC: Interior mutability via atomics only
//   - All fields are AtomicU64 (inherently Sync)
//   - No raw pointers to thread-local data
//   - No interior mutability beyond atomics
// #VERIFY_THREAD_SAFE: Property test with 8 threads, 10K iterations, no data races
unsafe impl Send for MyCapsule {}
unsafe impl Sync for MyCapsule {}
```

**Checklist**:
- [ ] All `unsafe impl Send/Sync` have `#ASSUME_SEND_SYNC` + `#VERIFY_THREAD_SAFE`
- [ ] Justify why safe (atomics, no raw pointers, etc.)
- [ ] Add concurrent property test (8+ threads)

---

### 7. TOCTOU Prevention (Generation counters required)

```rust
// ❌ BAD: TOCTOU race
let value = atomic.load(Ordering::Acquire);
// Another thread could change value here!
atomic.store(value + 1, Ordering::Release);

// ✅ GOOD: CAS loop prevents TOCTOU
// #ASSUME_TOCTOU_SAFE: CAS loop ensures atomic update
// #VERIFY_TOCTOU_PREVENTED: Property test with 8 threads, no lost updates
loop {
    let current = atomic.load(Ordering::Acquire);
    match atomic.compare_exchange(
        current, current + 1,
        Ordering::Release, Ordering::Relaxed
    ) {
        Ok(_) => break,
        Err(_) => continue,
    }
}

// ✅ BETTER: Generation counter pattern (DualAtomicU64)
// #ASSUME_TOCTOU_SAFE: Generation counter detects concurrent writes
// #VERIFY_TOCTOU_PREVENTED: Property test validates race detection
let gen1 = dual.load_secondary(Ordering::Acquire);
let value = dual.load_primary(Ordering::Acquire);
let gen2 = dual.load_secondary(Ordering::Acquire);
if gen1 == gen2 { /* consistent */ }
```

**Checklist**:
- [ ] No load-then-store patterns without CAS or generation counters
- [ ] Add `#ASSUME_TOCTOU_SAFE` + `#VERIFY_TOCTOU_PREVENTED`
- [ ] Property test validates race detection

---

## 📚 ASSUM Category Reference

### 10 Categories (from ASSUM_SAFETY.md)

1. **PANIC_SAFETY** - `unwrap()`, `expect()`
   - Tags: `#ASSUME_PANIC_SAFE`, `#VERIFY_NO_PANIC`
   - Verification: Test all code paths

2. **TYPE_SAFETY** - `unsafe { }`
   - Tags: `#ASSUME_TYPE_SAFE`, `#VERIFY_UNSAFE_INVARIANTS`
   - Verification: Miri, ThreadSanitizer, manual proof

3. **TOCTOU_PREVENTION** - Load-then-store patterns
   - Tags: `#ASSUME_TOCTOU_SAFE`, `#VERIFY_TOCTOU_PREVENTED`
   - Verification: Property tests, Loom model checking

4. **MEMORY_ORDERING** - `Ordering::Relaxed`
   - Tags: `#ASSUME_MEMORY_ORDERING`, `#VERIFY_ORDERING_SUFFICIENT`
   - Verification: B32 benchmarks (performance), property tests (correctness)

5. **SEND_SYNC_TRAITS** - `unsafe impl Send/Sync`
   - Tags: `#ASSUME_SEND_SYNC`, `#VERIFY_THREAD_SAFE`
   - Verification: Concurrent property tests, ThreadSanitizer

6. **STATE_TRANSITIONS** - State machines
   - Tags: `#ASSUME_STATE_VALID`, `#VERIFY_STATE_MACHINE`
   - Verification: Model checking (TLA+), exhaustive tests

7. **METRIC_ATOMICITY** - Atomic counters
   - Tags: `#ASSUME_METRIC_ATOMIC`, `#VERIFY_COUNTER_ACCURACY`
   - Verification: Concurrent stress tests (sum validation)

8. **LIFETIME_SAFETY** - Lifetime assumptions in unsafe
   - Tags: `#ASSUME_LIFETIME_VALID`, `#VERIFY_LIFETIME_BOUNDS`
   - Verification: Borrow checker, Valgrind, ASAN

9. **INVARIANT_MAINTENANCE** - `assert!`, `debug_assert!`
   - Tags: `#ASSUME_INVARIANT`, `#VERIFY_INVARIANT`
   - Verification: Property tests, exhaustive tests

10. **RESOURCE_CLEANUP** - `impl Drop`
    - Tags: `#ASSUME_RESOURCE_CLEANUP`, `#VERIFY_DROP_SAFE`
    - Verification: Valgrind leak check, panic tests

---

## 🔍 Quick Self-Audit

Run these commands before committing:

```bash
# 1. Check for undocumented unsafe
grep -r "unsafe\s*{" src/ | grep -v "// SAFETY:" | wc -l
# Target: 0

# 2. Check for production unwrap()
grep -r "\.unwrap()" src/ --include="*.rs" | grep -v "tests/" | grep -v "#\[test\]" | wc -l
# Target: <10

# 3. Check ASSUM coverage
ASSUME=$(grep -r "#ASSUME_" src/ | wc -l)
VERIFY=$(grep -r "#VERIFY_" src/ | wc -l)
echo "VERIFY Coverage: $((VERIFY * 100 / ASSUME))%"
# Target: >99%

# 4. Run Miri on unsafe code
cargo miri test
# Target: All tests pass, no UB detected

# 5. Run property tests
cargo test --features proptest
# Target: All tests pass

# 6. Verify capsules
cargo test --lib | grep "verify_capsule"
# Target: All verifications pass
```

---

## 📖 Code Review Checklist

When reviewing PRs, check:

### Must-Have (Block PR)
- [ ] Zero undocumented unsafe blocks
- [ ] Zero production unwrap() without ASSUM tags
- [ ] All #ASSUME tags have #VERIFY tags
- [ ] All capsules verified (derive or manual macro)
- [ ] All unsafe Send/Sync documented
- [ ] Miri clean (if unsafe code present)

### Should-Have (Request Changes)
- [ ] All Relaxed orderings justified
- [ ] All expect() calls have clear messages
- [ ] Property tests for concurrent code
- [ ] TOCTOU prevention documented

### Nice-to-Have (Comment)
- [ ] Performance benchmarks (B32)
- [ ] Examples in doc comments
- [ ] Integration tests

---

## 🎯 Examples by Module

### Core Patterns (99%+ safe examples)
- ✅ `patterns/dual_atomic.rs` - Perfect ASSUM compliance
- ✅ `hash/const_hash.rs` - 100% safe, zero unsafe
- ✅ `collections/stats_capsule.rs` - 100% safe atomics

### Collections (Good examples, some work needed)
- ⚠️ `collections/concurrent_map.rs` - Needs unwrap() cleanup
- ⚠️ `collections/lockfree_table.rs` - Needs SAFETY comments

### Parallel (Needs work)
- ❌ `parallel/iter.rs` - High unsafe, needs documentation
- ❌ `parallel/chunked.rs` - High unwrap(), needs cleanup

**Recommendation**: Study `dual_atomic.rs` and `const_hash.rs` for perfect examples.

---

## 🚨 Common Mistakes

### Mistake 1: Missing SAFETY comment
```rust
// ❌ BAD
unsafe { *ptr = value; }

// ✅ GOOD
// SAFETY: ptr is valid (allocated line 42), aligned (8 bytes), exclusive access
unsafe { *ptr = value; }
```

### Mistake 2: Unwrap without justification
```rust
// ❌ BAD
let value = map.get(&key).unwrap();

// ✅ GOOD
let value = map.get(&key)?;
```

### Mistake 3: ASSUM without VERIFY
```rust
// ❌ BAD
// #ASSUME_TOCTOU_SAFE: Generation counter prevents races
self.generation.fetch_add(1, Ordering::Release);

// ✅ GOOD
// #ASSUME_TOCTOU_SAFE: Generation counter prevents races
// #VERIFY_TOCTOU_PREVENTED: Property test line 456 validates
self.generation.fetch_add(1, Ordering::Release);
```

### Mistake 4: Unjustified Relaxed ordering
```rust
// ❌ BAD
counter.fetch_add(1, Ordering::Relaxed);

// ✅ GOOD
// #ASSUME_MEMORY_ORDERING: Relaxed sufficient (independent counter)
// #VERIFY_ORDERING_SUFFICIENT: 15ns vs 25ns SeqCst (40% faster)
counter.fetch_add(1, Ordering::Relaxed);
```

### Mistake 5: Unverified capsule
```rust
// ❌ BAD
#[repr(C, align(64))]
pub struct MyCapsule { ... }

// ✅ GOOD
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct MyCapsule { ... }
```

---

## 📞 Questions?

**ASSUM Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/ASSUM_SAFETY.md`
**Audit Report**: `ASSUM_SAFETY_AUDIT_REPORT.md`
**Examples**: `patterns/dual_atomic.rs`, `hash/const_hash.rs`

**Remember**: Every assumption needs verification. When in doubt, ask!

---

**Last Updated**: 2025-10-31
**Maintainer**: Security Expert (ASSUM Framework)
