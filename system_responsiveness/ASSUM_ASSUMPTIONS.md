# ASSUM Safety Assumptions Catalog
## System Responsiveness Daemon (sysrespond) v0.1.0

**Audit Date**: 2025-10-20
**Auditor**: ASSUM Safety Framework Specialist
**Framework**: ASSUM_SAFETY.md (10 categories)

---

## Executive Summary

**Total Assumptions Identified**: 58
- **Category 1 (PANIC_SAFETY)**: 4 assumptions
- **Category 2 (TYPE_SAFETY)**: 0 assumptions (zero unsafe blocks)
- **Category 3 (TOCTOU_PREVENTION)**: 12 assumptions
- **Category 4 (MEMORY_ORDERING)**: 18 assumptions
- **Category 5 (SEND_SYNC_TRAITS)**: 2 assumptions
- **Category 6 (STATE_TRANSITIONS)**: 3 assumptions
- **Category 7 (METRIC_ATOMICITY)**: 6 assumptions
- **Category 8 (LIFETIME_SAFETY)**: 2 assumptions
- **Category 9 (INVARIANT_MAINTENANCE)**: 7 assumptions
- **Category 10 (RESOURCE_CLEANUP)**: 4 assumptions

---

## Category 1: PANIC_SAFETY (4 assumptions)

### ASSUM-PANIC-001: SystemTime::now().unwrap()
**Location**: `process_state.rs:98-101`, `resource_governor.rs:95-98`, `resource_governor.rs:154-157`
**Severity**: WARNING
**Pattern**: `.unwrap()` on `SystemTime::now().duration_since(UNIX_EPOCH)`

**Assumption**:
- SystemTime::now() always returns a time after UNIX_EPOCH (1970-01-01)
- System clock is never set to a time before 1970

**Risk**:
- If system clock is set to pre-1970 date, this will panic
- Could happen on embedded systems, time travel bugs, or malicious clock manipulation

**Mitigation Required**: YES
- Replace with `.unwrap_or(0)` or `.ok().unwrap_or(0)` to handle clock errors gracefully

### ASSUM-PANIC-002: HashMap entry().or_insert_with()
**Location**: `streaming_monitor.rs:147-148`
**Severity**: LOW
**Pattern**: HashMap entry API (cannot panic under normal conditions)

**Assumption**:
- HashMap memory allocation succeeds
- No OOM during entry insertion

**Risk**:
- OOM could trigger allocator abort (not unwrap panic)
- Considered acceptable for daemon operations

### ASSUM-PANIC-003: Duration::from_secs() arithmetic
**Location**: `streaming_monitor.rs:100`, `resource_governor.rs:100`
**Severity**: LOW
**Pattern**: Duration arithmetic (u32 timestamp subtraction)

**Assumption**:
- Unix timestamp fits in u32 (valid until year 2106)
- Timestamp arithmetic doesn't overflow

**Risk**:
- Year 2038 problem (u32 timestamp overflow)
- Timestamp wrapping could cause incorrect comparisons

**Mitigation Required**: YES
- Use u64 for timestamps instead of u32
- Add overflow checks for timestamp arithmetic

### ASSUM-PANIC-004: Process name to_string_lossy()
**Location**: `streaming_monitor.rs:139`
**Severity**: LOW
**Pattern**: OsStr to Cow<str> conversion (cannot panic)

**Assumption**:
- Process name conversion is infallible (returns lossy UTF-8)

**Risk**: None (method is explicitly infallible)

---

## Category 2: TYPE_SAFETY (0 assumptions)

### STATUS: ✅ EXCELLENT

**Zero unsafe blocks in application code**:
- No raw pointer dereferences
- No transmutes
- No union access
- No FFI in our code (only dependencies)
- No manual Send/Sync implementations

**Dependencies with unsafe**:
- `nix` crate (signal handling syscalls)
- `sysinfo` crate (process info syscalls)
- Standard library atomics (but verified safe)

See ASSUM_UNSAFE_AUDIT.md for dependency analysis.

---

## Category 3: TOCTOU_PREVENTION (12 assumptions)

### ASSUM-TOCTOU-001: Generation Counter Prevents PID Reuse
**Location**: `process_state.rs:18,52-59,78-82,135-138`
**Severity**: CRITICAL
**Pattern**: 8-bit generation counter for TOCTOU prevention

**Assumption**:
- Generation counter increments on every update
- PID + generation uniquely identifies a process instance
- PID doesn't get reused within same generation cycle

**Risk**:
- 8-bit counter wraps at 256 updates
- If process updates >256 times before PID reuse, TOCTOU possible
- At 10s scan interval: 256 updates = 42.6 minutes
- If PID reused within 42 minutes, could kill wrong process

**Verification Required**: Stress test PID reuse within generation wrap window

### ASSUM-TOCTOU-002: ProcessStateCapsule::set_whitelisted() CAS Loop
**Location**: `process_state.rs:148-165`
**Severity**: CRITICAL
**Pattern**: Compare-exchange loop for flag modification

**Assumption**:
- CAS loop eventually succeeds (no livelock)
- Other threads don't starve this operation
- Weak CAS is sufficient (spurious failures acceptable)

**Risk**:
- Under extreme contention, could livelock
- No backoff strategy (tight spin loop)

**Mitigation**: Add exponential backoff after N failed attempts

### ASSUM-TOCTOU-003: ResourceGovernor record_kill() CAS Loop
**Location**: `resource_governor.rs:115-148`
**Severity**: CRITICAL
**Pattern**: CAS loop for atomic kill counter increment

**Assumption**:
- CAS loop completes before circuit breaker trips
- No race between kill increment and circuit breaker check
- Counter increment is atomic with respect to circuit state

**Risk**:
- Race: increment succeeds but circuit trips before check
- Could allow N+1 kill when threshold is N

**Current Behavior**: Acceptable (one extra kill is tolerable)

### ASSUM-TOCTOU-004: Circuit Breaker Trip State Transition
**Location**: `resource_governor.rs:151-176`
**Severity**: HIGH
**Pattern**: CAS loop to transition circuit state to Open

**Assumption**:
- Only one thread successfully trips the circuit
- Timestamp update is atomic with state change
- CAS prevents duplicate trip actions

**Risk**:
- Multiple threads could all attempt to trip simultaneously
- Last writer wins on timestamp (acceptable)

**Verification**: Concurrent stress test with simultaneous kills

### ASSUM-TOCTOU-005: Circuit Breaker Reset State Transition
**Location**: `resource_governor.rs:179-217`
**Severity**: HIGH
**Pattern**: Nested CAS loops (limits reset + circuit state change)

**Assumption**:
- Outer CAS (limits) completes before inner CAS (circuit)
- No race between the two state updates
- Inner loop can be skipped if circuit already Closed

**Risk**:
- If outer CAS succeeds but inner CAS never succeeds, circuit stuck Open
- Nested loops could livelock under contention

**Mitigation**: Add livelock detection (max retry count)

### ASSUM-TOCTOU-006: Process HashMap Cleanup TOCTOU
**Location**: `streaming_monitor.rs:177-179`
**Severity**: MEDIUM
**Pattern**: Check-then-retain pattern on HashMap

**Assumption**:
- Process existence check is stable during retain
- sysinfo.process(pid) doesn't have TOCTOU with HashMap modification
- No race between existence check and capsule access

**Risk**:
- Process could exit between check and next scan
- HashMap retains stale capsule for one scan cycle (acceptable)

**Current Behavior**: Acceptable (cleaned up next cycle)

### ASSUM-TOCTOU-007: Kill Process PID Validity
**Location**: `streaming_monitor.rs:191-226`
**Severity**: CRITICAL
**Pattern**: PID validity between hung detection and kill signal

**Assumption**:
- PID still valid when kill() is called (after hung detection)
- Process hasn't exited and PID reused between detection and kill
- Generation counter prevents killing wrong process

**Risk**:
- High-churn PID reuse could kill wrong process
- Time window: scan → circuit breaker → SIGTERM → SIGKILL (30+ seconds)
- Generation counter should catch this, but 8-bit wrapping is concerning

**Mitigation**: Check generation counter BEFORE kill, not just during scan

### ASSUM-TOCTOU-008: SIGTERM → SIGKILL Grace Period
**Location**: `streaming_monitor.rs:211-220`
**Severity**: HIGH
**Pattern**: Async sleep between SIGTERM and SIGKILL

**Assumption**:
- Process still exists during grace period
- PID hasn't been reused during sleep
- kill(pid, None) check is atomic with respect to signal delivery

**Risk**:
- PID reused during 30s grace period
- Could send SIGKILL to innocent process
- No generation counter check during kill

**Mitigation Required**: CRITICAL - Add generation counter validation before SIGKILL

### ASSUM-TOCTOU-009: Circuit Breaker can_kill() Check
**Location**: `resource_governor.rs:85-104`, `streaming_monitor.rs:168-172`
**Severity**: HIGH
**Pattern**: Load circuit state, then use result

**Assumption**:
- Circuit state doesn't change between can_kill() and record_kill()
- Relaxed ordering is sufficient for circuit state reads
- Race between can_kill() and trip_circuit_breaker() is acceptable

**Risk**:
- Circuit could trip between can_kill() check and actual kill
- Already recorded kill could be rejected (acceptable loss)

**Current Behavior**: Acceptable (conservative: reject on trip)

### ASSUM-TOCTOU-010: Timestamp Comparison for Cooldown
**Location**: `resource_governor.rs:93-100`
**Severity**: MEDIUM
**Pattern**: SystemTime::now() used for cooldown calculation

**Assumption**:
- Timestamp arithmetic doesn't overflow (u32 wraps in 2106)
- Clock monotonicity (system clock doesn't go backward)
- Cooldown calculation is correct across timestamp wrap

**Risk**:
- System clock change could break cooldown logic
- Backward clock jump → cooldown never expires
- Forward clock jump → cooldown expires immediately

**Mitigation**: Use monotonic clock (std::time::Instant) instead of wall clock

### ASSUM-TOCTOU-011: Total Kills Counter Wrapping
**Location**: `resource_governor.rs:121`
**Severity**: LOW
**Pattern**: wrapping_add() on u16 counter

**Assumption**:
- Counter wrap at 65535 is acceptable
- Monitoring tools handle wrap correctly
- No logic depends on absolute kill count

**Risk**: Minimal (monitoring-only counter)

### ASSUM-TOCTOU-012: Active Kills vs Circuit Threshold Race
**Location**: `resource_governor.rs:138-143`
**Severity**: MEDIUM
**Pattern**: Load threshold, compare, then trip circuit

**Assumption**:
- Threshold doesn't change during comparison
- new_active is correct value at time of check
- No race between increment completion and threshold check

**Risk**:
- If threshold changes mid-check, could trip incorrectly
- Current code: threshold is constant after construction (safe)

---

## Category 4: MEMORY_ORDERING (18 assumptions)

### ASSUM-ORDER-001: ProcessState Relaxed Load in is_hung()
**Location**: `process_state.rs:110`
**Severity**: WARNING
**Pattern**: `Ordering::Relaxed` for state read

**Assumption**:
- Stale state is acceptable for hung detection
- No synchronization needed with state updates
- Approximate values (CPU%, runtime) are sufficient

**Justification**:
- False negative: miss one hung detection cycle (acceptable)
- False positive: impossible (conservative thresholds)
- Next scan will see updated state

**Verification**: Acceptable for monitoring use case

### ASSUM-ORDER-002: ProcessState Release Store in update()
**Location**: `process_state.rs:96`
**Severity**: CRITICAL
**Pattern**: `Ordering::Release` for state publish

**Assumption**:
- Release ordering publishes all state fields to readers
- Acquire fence in readers synchronizes with this Release
- Prevents reordering of packed state construction

**Verification**: **MISSING ACQUIRE FENCE IN READERS**
- is_hung() uses Relaxed (no synchronization)
- pid() uses Relaxed (no synchronization)
- generation() uses Relaxed (no synchronization)

**Risk**: HIGH - Readers might see torn/inconsistent state
**Mitigation Required**: Change readers to Acquire or add fence

### ASSUM-ORDER-003: ProcessState Relaxed Load in pid()
**Location**: `process_state.rs:129`
**Severity**: MEDIUM
**Pattern**: `Ordering::Relaxed` for PID extraction

**Assumption**:
- PID value is stable (doesn't change after init)
- Relaxed sufficient for immutable field
- No torn reads on 64-bit atomic

**Risk**: PID DOES change (update() modifies entire state)
- Could read stale PID
- Generation counter doesn't help if PID is stale

**Mitigation Required**: Use Acquire ordering

### ASSUM-ORDER-004: ProcessState Relaxed Load in generation()
**Location**: `process_state.rs:136`
**Severity**: CRITICAL
**Pattern**: `Ordering::Relaxed` for generation counter read

**Assumption**:
- Relaxed sufficient for TOCTOU prevention
- Generation counter visible immediately after update
- No reordering of generation read relative to PID read

**Risk**: CRITICAL - Generation counter could be stale
- If generation is stale, TOCTOU protection fails
- Could kill process with old generation + new PID (wrong process)

**Mitigation Required**: MUST use Acquire ordering for TOCTOU safety

### ASSUM-ORDER-005: ProcessState Relaxed Store for last_updated
**Location**: `process_state.rs:102`
**Severity**: LOW
**Pattern**: `Ordering::Relaxed` for timestamp

**Assumption**:
- Timestamp is monitoring-only (no correctness dependency)
- Stale timestamp is acceptable
- No synchronization needed

**Verification**: Acceptable (timestamp not used for logic)

### ASSUM-ORDER-006: ProcessState Relaxed in set_whitelisted() CAS
**Location**: `process_state.rs:150,159`
**Severity**: HIGH
**Pattern**: Relaxed load + Release/Relaxed CAS

**Assumption**:
- Relaxed load sufficient for CAS loop source value
- Release on success publishes whitelist flag
- Relaxed on failure avoids unnecessary synchronization

**Risk**: Relaxed load could miss concurrent updates
- CAS loop will retry, but could spin unnecessarily

**Verification**: Acceptable (CAS loop handles races)

### ASSUM-ORDER-007: ResourceGovernor Relaxed in can_kill()
**Location**: `resource_governor.rs:86`
**Severity**: MEDIUM
**Pattern**: `Ordering::Relaxed` for circuit state read

**Assumption**:
- Stale circuit state is acceptable
- Conservative bias (may reject valid kill)
- No synchronization needed with trip/reset

**Risk**: Circuit state could be stale
- Could allow kill when circuit just tripped (race window)
- Could reject kill when circuit just reset (conservative)

**Current Behavior**: Acceptable (conservative kills preferred)

### ASSUM-ORDER-008: ResourceGovernor Release in record_kill()
**Location**: `resource_governor.rs:132`
**Severity**: CRITICAL
**Pattern**: `Ordering::Release` for kill counter CAS

**Assumption**:
- Release publishes counter update to circuit breaker checker
- Happens-before relationship with can_kill() reads
- Prevents reordering of counter arithmetic

**Verification**: **MISSING ACQUIRE IN can_kill()**
- can_kill() uses Relaxed (no synchronization)
- Could see stale counter values

**Risk**: MEDIUM - Circuit breaker could miss threshold trip
**Mitigation**: Add Acquire fence in can_kill() or use SeqCst

### ASSUM-ORDER-009: ResourceGovernor Release in trip_circuit_breaker()
**Location**: `resource_governor.rs:168`
**Severity**: CRITICAL
**Pattern**: `Ordering::Release` for circuit state CAS

**Assumption**:
- Release publishes Open state to can_kill() readers
- Happens-before relationship established
- Timestamp update visible with state change

**Verification**: **MISSING ACQUIRE IN can_kill()**
- can_kill() uses Relaxed (no synchronization)
- Could see Open state but stale timestamp

**Risk**: HIGH - Cooldown logic could use wrong timestamp
**Mitigation Required**: can_kill() must use Acquire

### ASSUM-ORDER-010: ResourceGovernor Nested CAS Ordering
**Location**: `resource_governor.rs:186,204`
**Severity**: HIGH
**Pattern**: Release/Relaxed CAS in nested loops

**Assumption**:
- First CAS (limits) Release visible before second CAS (circuit)
- No memory barrier needed between loops
- Both CASs independently synchronized

**Risk**: Second CAS might not see first CAS effects
- Could reset limits but fail to reset circuit
- Inconsistent state (limits=0, circuit=Open)

**Mitigation**: Add fence between CASs or use SeqCst

### ASSUM-ORDER-011: Circuit Timestamp Overflow Arithmetic
**Location**: `resource_governor.rs:100`
**Severity**: MEDIUM
**Pattern**: u32 timestamp arithmetic (wrapping subtraction)

**Assumption**:
- Wrapping subtraction handles timestamp wrap correctly
- Cooldown comparison works across u32 boundary
- No integer overflow in comparison

**Risk**: Timestamp wrap at 2106
- Subtraction: `(2^32-1) - 0 = 2^32-1` (correct)
- Subtraction across wrap: `5 - (2^32-2) = 7` (wraps correctly)

**Verification**: Wrapping subtraction is correct for circular time

### ASSUM-ORDER-012: Total Kills Read Ordering
**Location**: `resource_governor.rs:227`
**Severity**: LOW
**Pattern**: `Ordering::Relaxed` for monitoring counter

**Assumption**:
- Stale counter acceptable for logging/monitoring
- No correctness dependency on exact value
- Eventual visibility sufficient

**Verification**: Acceptable (monitoring-only)

### ASSUM-ORDER-013: Active Kills Read Ordering
**Location**: `resource_governor.rs:233`
**Severity**: LOW
**Pattern**: `Ordering::Relaxed` for monitoring counter

**Assumption**: Same as ASSUM-ORDER-012
**Verification**: Acceptable (monitoring-only)

### ASSUM-ORDER-014: Circuit State Read Ordering
**Location**: `resource_governor.rs:239`
**Severity**: LOW
**Pattern**: `Ordering::Relaxed` for monitoring state

**Assumption**: Same as ASSUM-ORDER-012
**Verification**: Acceptable (monitoring-only)

### ASSUM-ORDER-015: CPU Limit Read Ordering
**Location**: `resource_governor.rs:221`
**Severity**: LOW
**Pattern**: `Ordering::Relaxed` for constant value

**Assumption**:
- CPU limit never changes after construction
- Relaxed sufficient for immutable configuration
- No synchronization needed

**Verification**: Acceptable (immutable field)

### ASSUM-ORDER-016: AtomicU64 Single-Read Guarantee
**Location**: All 64-bit packed state loads
**Severity**: CRITICAL
**Pattern**: Assumption that AtomicU64::load() is single instruction

**Assumption**:
- 64-bit load is atomic on x86_64 (true)
- No torn reads (partial old + partial new state)
- All platforms support 8-byte atomic loads

**Risk**: 32-bit platforms might not have native 64-bit atomics
- Could use locks underneath (slower, but still safe)
- ARM32 might tear 64-bit loads without alignment

**Verification**: Rust guarantees AtomicU64 is always atomic (or compile error)

### ASSUM-ORDER-017: Happens-Before Chain: update() → is_hung()
**Location**: `process_state.rs:96,110`
**Severity**: CRITICAL
**Pattern**: Release store → Relaxed load

**Assumption**:
- No happens-before relationship established
- Readers may see stale state (acceptable)
- Approximate values sufficient for hung detection

**Verification**: **BROKEN** - No synchronization
- Should be Release → Acquire for visibility guarantee
- Current: Release → Relaxed = no ordering guarantee

**Mitigation Required**: Change is_hung() to Acquire load

### ASSUM-ORDER-018: Happens-Before Chain: record_kill() → can_kill()
**Location**: `resource_governor.rs:132,86`
**Severity**: CRITICAL
**Pattern**: Release CAS → Relaxed load

**Assumption**:
- Circuit breaker state propagates to can_kill() readers
- Kill counter increments are visible
- Relaxed reads are "eventually consistent"

**Verification**: **BROKEN** - No happens-before relationship
- Release → Relaxed does NOT establish synchronization
- can_kill() could see stale circuit state indefinitely

**Mitigation Required**: CRITICAL - Use Acquire in can_kill()

---

## Category 5: SEND_SYNC_TRAITS (2 assumptions)

### ASSUM-SEND-001: ProcessStateCapsule is Send + Sync
**Location**: `process_state.rs:13` (derived automatically)
**Severity**: LOW
**Pattern**: Derive ComputationalCapsule implies Send + Sync

**Assumption**:
- AtomicU64 is Sync (true - standard library)
- [u8; N] padding is Send + Sync (true - primitive)
- No non-Send/Sync fields

**Verification**: ✅ Compiler enforces this automatically

### ASSUM-SEND-002: ResourceGovernorCapsule is Send + Sync
**Location**: `resource_governor.rs:13` (derived automatically)
**Severity**: LOW
**Pattern**: Same as ASSUM-SEND-001

**Verification**: ✅ Compiler enforces this automatically

---

## Category 6: STATE_TRANSITIONS (3 assumptions)

### ASSUM-STATE-001: Circuit Breaker State Machine
**Location**: `resource_governor.rs:50-54`
**Severity**: CRITICAL
**Pattern**: FSM with 3 states (Closed → Open → HalfOpen → Closed)

**Assumption**:
- Only valid transitions allowed
- State transitions are atomic
- No invalid states reachable

**Valid Transitions**:
- Closed → Open (trip_circuit_breaker, kill threshold exceeded)
- Open → HalfOpen (reset_active_kills, periodic reset)
- HalfOpen → Closed (can_kill + cooldown expired)
- HalfOpen → Open (kill during HalfOpen → threshold → trip again)

**Risk**: Race conditions could create invalid state
- Multiple threads tripping simultaneously
- Reset racing with trip

**Verification Required**: Property test with concurrent operations

### ASSUM-STATE-002: Circuit Breaker State Encoding
**Location**: `resource_governor.rs:50-54`
**Severity**: MEDIUM
**Pattern**: u8 state encoding (0=Closed, 1=HalfOpen, 2=Open)

**Assumption**:
- Only values 0, 1, 2 are written
- No bit corruption (AtomicU64 prevents this)
- match statement exhaustiveness prevents invalid states

**Risk**: Invalid state value (3-255) could bypass safety
- match default case treats as Closed

**Verification**: Acceptable (conservative fallback to Closed)

### ASSUM-STATE-003: Process Lifecycle States
**Location**: Implicit in streaming_monitor.rs
**Severity**: MEDIUM
**Pattern**: Process states: NotScanned → Scanned → Hung → Killed

**Assumption**:
- Process can only transition forward in lifecycle
- Dead processes are removed from HashMap
- No resurrection of killed processes

**Risk**: PID reuse could resurrect "killed" process
- HashMap entry persists for reused PID
- Generation counter should catch this

**Verification**: Stress test with rapid PID reuse

---

## Category 7: METRIC_ATOMICITY (6 assumptions)

### ASSUM-METRIC-001: Total Kills Counter Atomicity
**Location**: `resource_governor.rs:121`
**Severity**: HIGH
**Pattern**: fetch_add equivalent via CAS loop

**Assumption**:
- CAS loop provides atomic increment
- No lost updates under contention
- Counter is monotonically increasing

**Verification**: ✅ CAS loop guarantees atomicity

### ASSUM-METRIC-002: Active Kills Counter Atomicity
**Location**: `resource_governor.rs:120,123`
**Severity**: HIGH
**Pattern**: Read-modify-write via CAS loop

**Assumption**: Same as ASSUM-METRIC-001
**Verification**: ✅ CAS loop guarantees atomicity

### ASSUM-METRIC-003: Generation Counter Atomicity
**Location**: `process_state.rs:79-82`
**Severity**: CRITICAL
**Pattern**: Load-increment-pack-store (NOT atomic across)

**Assumption**:
- Relaxed load + non-atomic increment + Release store
- NO CAS loop (vulnerable to races)
- Assumes only one thread updates a given capsule

**Risk**: CRITICAL - Multiple threads could increment simultaneously
- Lost generation increments
- Same generation number reused
- TOCTOU protection breaks down

**Verification**: **BROKEN** - Race condition in generation increment
**Mitigation Required**: CRITICAL - Use CAS loop or fetch_add

### ASSUM-METRIC-004: Last Updated Timestamp Atomicity
**Location**: `process_state.rs:97-103`
**Severity**: LOW
**Pattern**: Atomic store (independent of state store)

**Assumption**:
- Timestamp update is separate from state update
- No atomicity requirement between them
- Timestamp can be newer or older than state

**Risk**: Timestamp might not match state
- State updated → thread preempted → timestamp stale
- Acceptable for monitoring

**Verification**: Acceptable (monitoring-only)

### ASSUM-METRIC-005: Scanned/Hung Counters in scan_and_evaluate()
**Location**: `streaming_monitor.rs:128-129`
**Severity**: LOW
**Pattern**: Local variables (no atomicity needed)

**Assumption**:
- Single-threaded scan (no concurrent scans)
- Counters local to scan invocation
- No sharing required

**Verification**: ✅ Tokio select ensures serial scans

### ASSUM-METRIC-006: Circuit Breaker Metrics Consistency
**Location**: `resource_governor.rs` (limits + circuit_breaker atomics)
**Severity**: MEDIUM
**Pattern**: Two separate AtomicU64 for related state

**Assumption**:
- limits and circuit_breaker can be inconsistent temporarily
- No cross-atomic invariants required
- Each atomic is independently consistent

**Risk**: Observer could see limits=0 but circuit=Open
- Acceptable (next load will see consistent state)

**Verification**: Acceptable (eventual consistency)

---

## Category 8: LIFETIME_SAFETY (2 assumptions)

### ASSUM-LIFE-001: Arc<ProcessStateCapsule> Lifetime
**Location**: `streaming_monitor.rs:14,147`
**Severity**: LOW
**Pattern**: Arc shared ownership in HashMap

**Assumption**:
- Capsule lives as long as HashMap entry
- No dangling references when entry removed
- Arc refcount prevents use-after-free

**Verification**: ✅ Rust ownership guarantees this

### ASSUM-LIFE-002: Arc<ResourceGovernorCapsule> Lifetime
**Location**: `streaming_monitor.rs:18,80`, `main.rs:42-55`
**Severity**: LOW
**Pattern**: Arc shared between main and monitor

**Assumption**:
- Governor outlives all references
- Arc keeps governor alive until monitor exits
- No premature drop

**Verification**: ✅ Rust ownership guarantees this

---

## Category 9: INVARIANT_MAINTENANCE (7 assumptions)

### ASSUM-INV-001: Bit Packing Invariants (ProcessState)
**Location**: `process_state.rs:8-19,68-93`
**Severity**: CRITICAL
**Pattern**: Manual bit packing with masks

**Assumption**:
- PID fits in 20 bits (max 1,048,576)
- CPU % × 10 fits in 12 bits (max 4095 → 409.5%)
- Runtime fits in 20 bits (max 1,048,576s = 12 days)
- Generation fits in 8 bits (wraps at 256)
- Flags fit in 4 bits

**Risk**: Overflow in any field corrupts packed state
- PID > 1M: truncated to lower 20 bits
- CPU > 409%: clamped to 4095 (handled via .min())
- Runtime > 12 days: clamped to 0xFFFFF (handled via .min())

**Verification**: ✅ Clamping prevents overflow (except PID)
**Mitigation**: Add PID overflow check

### ASSUM-INV-002: Bit Packing Invariants (ResourceGovernor)
**Location**: `resource_governor.rs:8-19,68-73`
**Severity**: CRITICAL
**Pattern**: Similar to ASSUM-INV-001

**Assumption**:
- CPU limit × 10 fits in 16 bits (max 6553 → 655.3%)
- Memory limit fits in 24 bits (max 16TB)
- Active kills fits in 8 bits (max 255)
- Total kills fits in 16 bits (wraps at 65535)

**Risk**: Same overflow risks as ProcessState
**Verification**: Wrapping is intentional for counters (acceptable)

### ASSUM-INV-003: Bit Packing Invariants (Circuit Breaker)
**Location**: `resource_governor.rs:22-28,72-73`
**Severity**: CRITICAL
**Pattern**: Packed circuit breaker state

**Assumption**:
- State fits in 8 bits (values 0-2, rest reserved)
- Timestamp fits in 32 bits (wraps in 2106)
- Threshold fits in 8 bits (max 255 kills/min)
- Cooldown fits in 16 bits (max 65535s = 18 hours)

**Risk**: Timestamp overflow in 2106 (Y2106 problem)
**Verification**: Acceptable (system unlikely to run until 2106)

### ASSUM-INV-004: Cache Alignment Invariants
**Location**: `process_state.rs:13` (128B), `resource_governor.rs:13` (64B)
**Severity**: HIGH
**Pattern**: Manual alignment via #[repr(C, align(N))]

**Assumption**:
- Compiler respects alignment attribute
- Padding is correctly sized
- Capsules don't share cache lines (false sharing prevention)

**Verification**: ✅ Derive macro enforces this at compile time
**Tests**: `tests::test_capsule_alignment()` validates

### ASSUM-INV-005: HashMap Consistency Invariant
**Location**: `streaming_monitor.rs:14,177-179`
**Severity**: MEDIUM
**Pattern**: HashMap cleanup removes dead PIDs

**Assumption**:
- HashMap always contains only live processes (eventually)
- Dead PIDs removed within one scan cycle
- No memory leak from unbounded growth

**Verification**: ✅ Explicit cleanup in retain()

### ASSUM-INV-006: Kill Counter vs Circuit State Invariant
**Location**: `resource_governor.rs` (entire module)
**Severity**: HIGH
**Pattern**: active_kills ≤ threshold → Closed, >threshold → Open

**Assumption**:
- Invariant: circuit=Closed ⟺ active_kills ≤ threshold
- Invariant: circuit=Open ⟺ active_kills > threshold (just tripped)
- Reset moves circuit HalfOpen and clears active_kills

**Risk**: Race conditions could violate invariant
- active_kills increments after circuit check
- Multiple threads racing on threshold boundary

**Verification Required**: Property test for invariant

### ASSUM-INV-007: Process Name Pattern Matching Invariant
**Location**: `streaming_monitor.rs:229-236`
**Severity**: LOW
**Pattern**: Substring matching for process classification

**Assumption**:
- Patterns are substrings (not regex)
- Case-sensitive matching
- Any match → positive classification

**Risk**: False positives (e.g., "test" matches "contest")
**Verification**: Acceptable (conservative classification)

---

## Category 10: RESOURCE_CLEANUP (4 assumptions)

### ASSUM-CLEAN-001: ProcessStateCapsule Drop
**Location**: Implicit (no custom Drop impl)
**Severity**: LOW
**Pattern**: Compiler-generated Drop

**Assumption**:
- AtomicU64 has no cleanup needed
- Padding has no cleanup needed
- No resources to release

**Verification**: ✅ No cleanup required (trivial Drop)

### ASSUM-CLEAN-002: ResourceGovernorCapsule Drop
**Location**: Implicit (no custom Drop impl)
**Severity**: LOW
**Pattern**: Same as ASSUM-CLEAN-001

**Verification**: ✅ No cleanup required (trivial Drop)

### ASSUM-CLEAN-003: HashMap Entry Removal
**Location**: `streaming_monitor.rs:177-179`
**Severity**: LOW
**Pattern**: HashMap.retain() drops removed Arcs

**Assumption**:
- Arc refcount drops to zero → capsule dropped
- No memory leak from cyclic references
- Drop order doesn't matter

**Verification**: ✅ Arc guarantees cleanup

### ASSUM-CLEAN-004: Tokio Task Cleanup
**Location**: `main.rs:70`, `streaming_monitor.rs:92-116`
**Severity**: MEDIUM
**Pattern**: Infinite loop in async task (never returns)

**Assumption**:
- Task runs forever (daemon process)
- No cleanup on exit (process termination)
- OS reclaims resources on process exit

**Risk**: No graceful shutdown
- Signal handler could allow clean exit
- Current: SIGTERM kills process abruptly

**Verification**: Acceptable for daemon (OS cleanup)

---

## Summary Statistics

| Category | Count | Critical | High | Medium | Low |
|----------|-------|----------|------|--------|-----|
| PANIC_SAFETY | 4 | 0 | 0 | 0 | 4 |
| TYPE_SAFETY | 0 | 0 | 0 | 0 | 0 |
| TOCTOU_PREVENTION | 12 | 4 | 4 | 3 | 1 |
| MEMORY_ORDERING | 18 | 7 | 3 | 4 | 4 |
| SEND_SYNC_TRAITS | 2 | 0 | 0 | 0 | 2 |
| STATE_TRANSITIONS | 3 | 1 | 0 | 2 | 0 |
| METRIC_ATOMICITY | 6 | 1 | 2 | 1 | 2 |
| LIFETIME_SAFETY | 2 | 0 | 0 | 0 | 2 |
| INVARIANT_MAINTENANCE | 7 | 3 | 1 | 1 | 2 |
| RESOURCE_CLEANUP | 4 | 0 | 0 | 1 | 3 |
| **TOTAL** | **58** | **16** | **10** | **12** | **20** |

**Critical Issues Requiring Immediate Fix**: 16
**High Priority Issues**: 10
**Medium Priority Issues**: 12
**Low Priority Issues**: 20

---

## Next Steps

1. **ASSUM_VERIFICATION.md**: Verify each assumption with evidence
2. **ASSUM_UNSAFE_AUDIT.md**: Audit dependencies for unsafe code
3. **MIRI Validation**: Run undefined behavior detection
4. **ASSUM_CONCURRENCY.md**: Analyze happens-before relationships
5. **Safety Tests**: Error injection and stress testing
6. **ASSUM_SAFETY_REPORT.md**: Final safety rating

**End of Assumptions Catalog**
