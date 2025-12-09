# CompactDualAtomicU64 Generation Livelock Fix

**Date**: 2025-11-27
**File**: `src/patterns/dual_atomic_u128.rs`
**Issue**: 4 tests hanging indefinitely (>60 seconds)
**Root Cause**: Infinite CAS retry loop in `write_with_generation()`
**Status**: ✅ FIXED

---

## Affected Tests

These 4 tests were hanging indefinitely:

1. `test_write_with_generation` - Sequential writes with generation increment
2. `test_read_consistent` - Reads after writes with generation tracking
3. `test_generation_overflow` - Generation counter wrapping at u64::MAX
4. `test_concurrent_write_with_generation` - 4 threads × 250 writes (1000 total)

All tests now pass in <1 second.

---

## Root Cause Analysis

### The Bug (Line 416)

```rust
// BEFORE (BROKEN)
pub fn write_with_generation(&self, value: u64) {
    loop {
        let (_, current_gen) = self.load_both(Ordering::Acquire);
        //   ↑ DISCARDED current_value!
        let new_gen = current_gen.wrapping_add(1);

        match self.compare_exchange_both(
            (0, current_gen),  // ❌ HARDCODED 0 - WRONG!
            (value, new_gen),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => break,
            Err(_) => continue,  // Infinite retry because CAS always fails
        }
    }
}
```

### Why It Caused a Livelock

1. **First write (value=42, gen=0→1)**:
   - Load: `(current_value=0, current_gen=0)`
   - CAS: `(0, 0) → (42, 1)` ✅ **SUCCESS** (current value is indeed 0)
   - Result: `(value=42, gen=1)` stored

2. **Second write (value=100, gen=1→2)**:
   - Load: `(current_value=42, current_gen=1)` (discarded current_value!)
   - CAS: `(0, 1) → (100, 2)` ❌ **FAIL** (expected primary=0, actual primary=42)
   - Retry loop repeats forever because:
     - Expected: `(0, 1)`
     - Actual: `(42, 1)`
     - Generation matches (1), but primary doesn't (42 ≠ 0)
     - Every retry loads `(42, 1)` and tries CAS with `(0, 1)` → infinite loop

3. **Concurrent writes**: Same issue, worse contention → all threads stuck in CAS retry

### The Pattern Mismatch

The standard `DualAtomicU64` pattern (128-byte version) uses:

```rust
let (current_value, current_gen) = self.load_both(Ordering::Acquire);
match self.compare_exchange_both(
    (current_value, current_gen),  // ✅ Use actual current value
    (value, new_gen),
    // ...
) { /* ... */ }
```

The `CompactDualAtomicU64` (64-byte version using AtomicU128) incorrectly hardcoded `0` instead of using the loaded `current_value`.

---

## The Fix

### Changed Code (Line 412-416)

```rust
// AFTER (FIXED)
pub fn write_with_generation(&self, value: u64) {
    loop {
        let (current_value, current_gen) = self.load_both(Ordering::Acquire);
        //   ↑ NOW USING current_value!
        let new_gen = current_gen.wrapping_add(1);

        match self.compare_exchange_both(
            (current_value, current_gen),  // ✅ Use actual current value
            (value, new_gen),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => break,
            Err(_) => continue,  // Retry on genuine contention (expected behavior)
        }
    }
}
```

### Why It Works Now

1. **First write**: Same as before, works correctly
2. **Second write**:
   - Load: `(current_value=42, current_gen=1)`
   - CAS: `(42, 1) → (100, 2)` ✅ **SUCCESS** (matches actual state)
   - Result: `(value=100, gen=2)` stored
3. **Subsequent writes**: CAS loop retries only on genuine contention (multi-writer race), not on every single operation

---

## Verification

### Test Results

```bash
✅ Test 1: test_write_with_generation (sequential writes)
   - First write: (42, 1) ✓
   - Second write: (100, 2) ✓
   - Multiple writes: (1000, 10) ✓

✅ Test 2: test_read_consistent (reads after writes)
   - Initial read: (42, 0) ✓
   - After write: (100, 1) ✓

✅ Test 3: test_generation_overflow (u64::MAX wrapping)
   - Before: (42, u64::MAX) ✓
   - After: (100, 0) ✓ (wraps correctly)

✅ Test 4: test_concurrent_write_with_generation (4 threads)
   - 4 threads × 250 writes = 1000 total ✓
   - Final generation: 1000 ✓
   - Completion time: <1 second (was >60 seconds timeout)
```

### Performance Impact

- **Before**: Infinite loop after first write (>60s timeout)
- **After**: <1 second for all tests (1000 concurrent writes)
- **Speedup**: ∞× (literally, from never completing to instant)

---

## Lessons Learned

### Why This Bug Happened

1. **Comment Misled the Implementation**:
   - Comment said: `"Don't care about old value, only generation"`
   - Reality: You MUST care about the old value for CAS to succeed
   - The 128-bit atomic packs BOTH values, so CAS checks BOTH

2. **Pattern Copy-Paste Error**:
   - The standard `DualAtomicU64` (128-byte) uses correct pattern
   - When implementing `CompactDualAtomicU64` (64-byte), the pattern was incorrectly modified
   - Likely assumption: "Since we use a single AtomicU128, maybe we only need to check generation?"
   - Incorrect: `compare_exchange_both` checks BOTH fields in the u128

3. **Missing Tests Earlier in Development**:
   - Tests existed but weren't run frequently enough during development
   - The bug would have been caught immediately if tests ran on every commit

### Prevention Strategies

1. **Always use actual loaded values in CAS loops** (never hardcode expected values)
2. **Comments should explain WHY, not WHAT** (misleading comment caused the bug)
3. **Cross-reference implementations** (standard DualAtomicU64 had correct pattern)
4. **Test during development**, not just after (catch bugs early)

---

## Related Code

- **Standard Pattern**: `src/patterns/dual_atomic.rs` (DualAtomicU64, 128 bytes)
- **Fixed Code**: `src/patterns/dual_atomic_u128.rs` (CompactDualAtomicU64, 64 bytes)
- **Tests**: Lines 548-575 (test_write_with_generation, test_read_consistent, test_generation_overflow, test_concurrent_write_with_generation)

---

## Framework Compliance

- **UCE34 Q33**: ✅ Lockfree atomics (no mutex, correct CAS pattern)
- **ASSUM**: ✅ Safe (no unwrap, documented memory ordering)
- **T28 Unit**: ✅ All 4 tests passing
- **B32**: ✅ <1s for 1000 concurrent writes (baseline: infinite timeout)
- **Chaos**: ✅ 100% lockfree, cache-aligned (64B), generation counter pattern

---

## Sign-Off

**Reviewed**: Samuel (2025-11-27)
**Verified**: All 4 tests pass in <1 second
**Impact**: Critical bug fix (infinite loop → instant completion)
**Risk**: Zero (fix aligns with standard DualAtomicU64 pattern)
**Deployment**: Ready for production (atomic_capsule v0.9.0+)
