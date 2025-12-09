# ASSUM Concurrency Analysis
## System Responsiveness Daemon (sysrespond) v0.1.0

**Audit Date**: 2025-10-20
**Focus**: Data races, happens-before relationships, synchronization

---

## Executive Summary

**Data Race Status**: ❌ **BROKEN** - Memory ordering insufficient

**Critical Issues**:
1. Release → Relaxed: No happens-before relationship
2. Generation counter: Non-atomic read-modify-write
3. Circuit breaker: Synchronization gaps in nested CAS

**Concurrency Model**: Async (tokio) + Shared state (Arc<AtomicU64>)

---

## Shared Mutable State Inventory

### State 1: ProcessStateCapsule.state (AtomicU64)
**Sharers**: StreamingMonitor (writer) + is_hung() (readers)
**Coordination**: Atomic operations (Release/Relaxed)
**Issue**: ❌ Release → Relaxed = NO synchronization

### State 2: ProcessStateCapsule.last_updated (AtomicU64)
**Sharers**: update() (writer) + monitoring (readers)
**Coordination**: Relaxed only
**Issue**: ✅ Acceptable (monitoring-only, no correctness dependency)

### State 3: ResourceGovernorCapsule.limits (AtomicU64)
**Sharers**: record_kill() (writers) + getters (readers)
**Coordination**: CAS loop (Release/Relaxed)
**Issue**: ⚠️ Acceptable for counters, but ordering weak

### State 4: ResourceGovernorCapsule.circuit_breaker (AtomicU64)
**Sharers**: trip/reset (writers) + can_kill() (reader)
**Coordination**: CAS loops (Release/Relaxed)
**Issue**: ❌ Release → Relaxed in can_kill() = NO sync

### State 5: HashMap<u32, Arc<ProcessStateCapsule>>
**Sharers**: StreamingMonitor (single-threaded async)
**Coordination**: &mut self ownership
**Issue**: ✅ Safe (no concurrent access)

---

## Happens-Before Analysis

### Scenario 1: Process State Update → Hung Detection

**Thread A (Update)**:
```rust
// 1. Pack state
let packed = pid | cpu | runtime | gen | flags;
// 2. Store with Release
self.state.store(packed, Ordering::Release);  // ← Synchronizes-with?
```

**Thread B (Read)**:
```rust
// 3. Load with Relaxed
let state = self.state.load(Ordering::Relaxed);  // ← NO synchronization!
// 4. Use stale data for hung detection
```

**Happens-Before Chain**: **BROKEN**
- Release store establishes ordering within Thread A
- But Relaxed load in Thread B does NOT synchronize
- No happens-before relationship

**Impact**: Thread B may see arbitrarily stale state
- Could miss hung process (false negative)
- Could see torn state (unlikely on x86, but possible on ARM)

**Fix**: Change line 110 to `Ordering::Acquire`

---

### Scenario 2: Kill Counter → Circuit Breaker Trip

**Thread A (Increment Counter)**:
```rust
// 1. CAS increment with Release
self.limits.compare_exchange_weak(..., Ordering::Release, Ordering::Relaxed);
// 2. Load circuit for threshold
let circuit = self.circuit_breaker.load(Ordering::Relaxed);  // ← NO sync!
```

**Happens-Before**: **WEAK**
- Release CAS publishes counter
- But next load is Relaxed (could be reordered)
- Threshold check might use stale circuit state

**Impact**: Circuit might trip late or not at all
**Fix**: Use Acquire for circuit load in record_kill()

---

### Scenario 3: Circuit Trip → can_kill() Check

**Thread A (Trip)**:
```rust
// 1. CAS with Release
self.circuit_breaker.compare_exchange_weak(
    circuit,
    new_circuit,  // Open state
    Ordering::Release,  // ← Publishes Open
    Ordering::Relaxed,
);
```

**Thread B (Check)**:
```rust
// 2. Load with Relaxed
let circuit = self.circuit_breaker.load(Ordering::Relaxed);  // ← NO sync!
let state = (circuit & CIRCUIT_STATE_MASK) as u8;
```

**Happens-Before**: **BROKEN**
- Release CAS in A does NOT synchronize with Relaxed load in B
- Thread B might never see Open state

**Impact**: Circuit breaker ineffective (kills continue when should stop)
**Fix**: Change can_kill() to use Acquire

---

### Scenario 4: Generation Counter Increment

**Thread A**:
```rust
// 1. Load generation (Relaxed)
let old_state = self.state.load(Ordering::Relaxed);
let old_gen = extract_generation(old_state);
```

**Thread B**:
```rust
// 1. Load generation (Relaxed, same time)
let old_state = self.state.load(Ordering::Relaxed);
let old_gen = extract_generation(old_state);
```

**Both Threads**:
```rust
// 2. Increment (non-atomic)
let new_gen = (old_gen + 1) & 0xFF;
// 3. Pack new state
let new_state = pack_with_generation(new_gen);
// 4. Store (last writer wins)
self.state.store(new_state, Ordering::Release);
```

**Race Condition**: **CRITICAL**
- Both load gen=5
- Both increment to gen=6
- Both store gen=6 (should be 7)
- **Lost generation increment**

**Impact**: TOCTOU protection fails (same generation reused)
**Fix**: Use CAS loop for generation increment

---

## Memory Ordering Cheat Sheet

### Current (BROKEN) Orderings

| Operation | Current | Required | Reason |
|-----------|---------|----------|--------|
| `ProcessState::update()` store | Release | Release | ✅ Correct (publish) |
| `ProcessState::is_hung()` load | Relaxed | **Acquire** | ❌ Need sync |
| `ProcessState::pid()` load | Relaxed | **Acquire** | ❌ Need sync |
| `ProcessState::generation()` load | Relaxed | **Acquire** | ❌ CRITICAL |
| `ProcessState::set_whitelisted()` CAS | Release/Relaxed | Release/**Acquire** | ⚠️ Weak |
| `ResourceGovernor::can_kill()` load | Relaxed | **Acquire** | ❌ Need sync |
| `ResourceGovernor::record_kill()` CAS | Release/Relaxed | Release/**Acquire** | ⚠️ Weak |
| `ResourceGovernor::trip_circuit_breaker()` CAS | Release/Relaxed | Release/**Acquire** | ⚠️ Weak |

**Fix Pattern**: Change ALL Relaxed loads to Acquire when reading state written with Release

---

## Deadlock Analysis

### Potential Deadlocks

**None Found**: ✅ 100% lockfree (no mutex/RwLock)

**Async Deadlocks**: None (tokio::select! doesn't deadlock)

**Livelock Potential**:
- CAS loops could livelock under extreme contention
- Mitigation: Add max retry count or exponential backoff

---

## Data Race Catalog

### Race 1: Generation Counter (CRITICAL)
**Type**: Lost update (read-modify-write race)
**Severity**: CRITICAL
**Fix**: CAS loop

### Race 2: State Visibility (HIGH)
**Type**: Stale read (insufficient ordering)
**Severity**: HIGH
**Fix**: Acquire loads

### Race 3: Circuit Breaker Sync (HIGH)
**Type**: Missed synchronization
**Severity**: HIGH
**Fix**: Acquire loads

### Race 4: Nested CAS Gap (MEDIUM)
**Type**: Ordering gap between CAS loops
**Severity**: MEDIUM
**Fix**: Memory fence or SeqCst

---

## ThreadSanitizer Expected Failures

When running with `-Z sanitizer=thread`:

**Expected Detections**:
1. ❌ Data race in `generation()` (concurrent reads + non-atomic increment)
2. ❌ Data race in `is_hung()` (Relaxed load of Release-written state)
3. ❌ Data race in `can_kill()` (Relaxed load of Release-written circuit)

**False Positives (Safe)**:
- Arc refcount operations (TSan doesn't understand Arc)
- Tokio internal races (TSan doesn't understand async)

---

## Loom Model Checking Plan

### Test Scenarios

**Scenario 1: Concurrent Generation Increment**
```rust
loom::model(|| {
    let capsule = Arc::new(ProcessStateCapsule::new(1234));
    let threads: Vec<_> = (0..2).map(|_| {
        let c = Arc::clone(&capsule);
        loom::thread::spawn(move || {
            c.update(1234, 100.0, 200, false, false, false);
        })
    }).collect();
    // Verify generation increments by 2 (not lost)
});
```

**Scenario 2: Circuit Breaker Trip Race**
```rust
loom::model(|| {
    let governor = Arc::new(ResourceGovernorCapsule::new(...));
    // Concurrent kills racing to trip circuit
    // Verify only one trip, or all trips consistent
});
```

---

## Recommendations

### Immediate (P0)
1. Change ALL Relaxed loads to Acquire (when reading Release-written state)
2. Use CAS loop for generation increment
3. Add memory fence between nested CAS loops

### High Priority (P1)
4. Run ThreadSanitizer to confirm races
5. Add Loom model checking for state machine
6. Add exponential backoff to CAS loops

### Medium Priority (P2)
7. Add SeqCst for complex invariants
8. Document memory ordering rationale (inline comments)
9. Add compile-time ordering verification (type system)

---

## Conclusion

**Concurrency Safety**: ❌ **NOT SAFE**

**Root Cause**: Insufficient memory ordering (Release → Relaxed)

**Impact**: Data races, stale reads, lost updates, TOCTOU failures

**Fix Complexity**: LOW (mostly changing Relaxed → Acquire)

**Fix Time**: 1-2 hours

**Verification**: ThreadSanitizer + Loom + stress tests

**End of Concurrency Analysis**
