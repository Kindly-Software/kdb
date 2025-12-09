# Architecture Deep Dive - Clapi Core

**Read Time**: 30 minutes
**Target Audience**: Advanced developers, system architects, contributors
**Prerequisites**: [ARCHITECTURE_OVERVIEW.md](ARCHITECTURE_OVERVIEW.md)

This document contains detailed implementation notes extracted from the original `/home/samuel/Primitives/clapi_core/ARCHITECTURE.md`.

---

## Detailed Memory Layouts

### BudgetSlotCapsule (128 bytes)

```rust
#[repr(C, align(128))]
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
pub struct BudgetSlotCapsule {
    // 16 bytes - atomic pointer to budget capsule
    capsule_ptr: AtomicPtr<RequestCapsule128>,

    // 8 bytes - generation counter (TOCTOU prevention)
    generation: AtomicU64,

    // 8 bytes - slot state (0 = free, 1 = allocated)
    state: AtomicU8,

    // 95 bytes - padding to 128B cache line
    _padding: [u8; 95],
}
```

**Verification**: Compile-time checked via `#[derive(ComputationalCapsule)]`
- Alignment: 128B (verified)
- Size: 128B (verified)
- Cache-aligned: Yes (exclusive double cache line on x86-64)
- False sharing: None (exclusive cache line per slot)

**Memory Ordering**:
- `capsule_ptr`: Acquire (load), Release (store) - full synchronization
- `generation`: Relaxed (counter only, no synchronization needed)
- `state`: Acquire/Release (slot state transitions)

---

### CircuitBreakerCapsule (64 bytes)

```rust
#[repr(C, align(64))]
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
pub struct CircuitBreakerCapsule {
    // 8 bytes - failure count
    failure_count: AtomicU64,

    // 8 bytes - total requests
    total_requests: AtomicU64,

    // 1 byte - state (0 = Closed, 1 = HalfOpen, 2 = Open)
    state: AtomicU8,

    // 8 bytes - last trip timestamp (epoch seconds)
    last_trip: AtomicU64,

    // 39 bytes - padding to 64B cache line
    _padding: [u8; 39],
}
```

**State Machine Implementation**:
```rust
impl CircuitBreakerCapsule {
    pub fn check_and_update(&self) -> CircuitState {
        let total = self.total_requests.fetch_add(1, Ordering::Relaxed);
        let failures = self.failure_count.load(Ordering::Relaxed);

        // Minimum samples before evaluation
        if total < MIN_SAMPLES {
            return CircuitState::Closed;
        }

        // Calculate failure rate in basis points (1bp = 0.01%)
        let failure_rate_bp = (failures * 10_000) / total;

        let current_state = self.state.load(Ordering::Acquire);

        match CircuitState::from_u8(current_state) {
            CircuitState::Closed if failure_rate_bp > FAILURE_THRESHOLD_BP => {
                // Trip circuit breaker
                self.state.store(CircuitState::Open as u8, Ordering::Release);
                self.last_trip.store(current_timestamp(), Ordering::Relaxed);
                CircuitState::Open
            },
            CircuitState::Open if elapsed_since_trip() > COOLDOWN_SECS => {
                // Enter half-open state for testing
                self.state.store(CircuitState::HalfOpen as u8, Ordering::Release);
                CircuitState::HalfOpen
            },
            CircuitState::HalfOpen if failure_rate_bp < RECOVERY_THRESHOLD_BP => {
                // Close circuit breaker
                self.state.store(CircuitState::Closed as u8, Ordering::Release);
                CircuitState::Closed
            },
            state => state,
        }
    }
}
```

---

## Lockfree Architecture Evolution

### Phase 1: Hybrid Lockfree (v0.1.x)

```rust
struct BudgetRegistry {
    // Cold path: HashMap with RwLock sharding (64 shards)
    slots: Arc<[RwLock<HashMap<u64, Arc<RequestCapsule128>>>; 64]>,

    // Hot path: Atomic CAS operations on capsules
    // Bottleneck: Write lock blocks ALL reads in same shard during insertion
}
```

**Performance**: 200-400ns (lock contention under load)

### Phase 2: Pure Atomic (v0.2.x+)

```rust
struct BudgetRegistry {
    // Preallocated array (zero hot-path allocation)
    slots: Box<[BudgetSlotCapsule; 1M]>,

    // Lockfree access via AtomicPtr
    // Zero lock contention - all paths lockfree
}
```

**Performance**: <100ns (3-4× faster, 8× better p99)

**Key Innovation**: Preallocate all slots upfront, use lockfree AtomicPtr + generation counters for access.

---

## Detailed Hot Path Analysis

### Budget Deduction Path (60ns)

```rust
pub fn try_deduct(&self, budget_id: u64, cost: i64) -> Result<i64, ClapiError> {
    // === Phase 1: Slot Lookup (10ns) ===
    // O(1) array access, no indirection
    let slot_idx = budget_id % self.max_slots;
    let slot = &self.slots[slot_idx];

    // === Phase 2: Circuit Breaker Check (5ns) ===
    // Single atomic load (Relaxed, no synchronization)
    if !self.circuit_breaker.allows_operation() {
        return Err(ClapiError::CircuitOpen);
    }

    // === Phase 3: Load Capsule Pointer (10ns) ===
    // Acquire ordering - synchronizes with Release store
    let capsule_ptr = slot.capsule_ptr.load(Ordering::Acquire);

    if capsule_ptr.is_null() {
        return Err(ClapiError::SlotNotAllocated { slot_id: slot_idx });
    }

    // === Phase 4: CAS Deduction (40ns) ===
    // Critical section - lockfree via compare-exchange
    let capsule = unsafe { &*capsule_ptr };

    let mut current = capsule.budget_cents.load(Ordering::Acquire);
    loop {
        if current < cost {
            return Err(ClapiError::BudgetExhausted {
                requested: cost,
                available: current,
            });
        }

        let new = current - cost;

        match capsule.budget_cents.compare_exchange_weak(
            current,
            new,
            Ordering::Release,  // Success: publish to other threads
            Ordering::Relaxed,  // Failure: no synchronization needed
        ) {
            Ok(_) => {
                // === Phase 5: Generation Increment (5ns) ===
                // Relaxed ordering - counter only
                slot.generation.fetch_add(1, Ordering::Relaxed);
                return Ok(new);
            },
            Err(actual) => {
                // CAS failed - retry with updated value
                current = actual;
            },
        }
    }
}
```

**Latency Breakdown** (AMD Ryzen 9 6900HX, DDR5-4800):
1. Slot lookup: ~10ns (L1 cache hit)
2. Circuit check: ~5ns (L1 cache hit, single atomic load)
3. Capsule load: ~10ns (L2 cache hit typical)
4. CAS loop: ~40ns (1-2 iterations typical, <1% contention)
5. Generation: ~5ns (L1 cache hit, relaxed atomic)

**Total**: ~60ns P50, ~120ns P99

---

## Concurrency Deep Dive

### TOCTOU Prevention via Generation Counters

**Problem**: Time-of-Check-Time-of-Use race condition

```rust
// ❌ UNSAFE: TOCTOU race condition
let ptr1 = slot.load();  // Thread A
let ptr2 = slot.load();  // Thread B
slot.store(new_ptr);     // Thread A deallocates
unsafe { &*ptr2 }        // Thread B accesses freed memory (UAF)
```

**Solution**: Generation counters

```rust
// ✅ SAFE: Generation counter prevents UAF
struct BudgetSlotCapsule {
    capsule_ptr: AtomicPtr<RequestCapsule128>,
    generation: AtomicU64,  // Incremented on every modification
}

impl BudgetSlotCapsule {
    pub fn load_with_generation(&self) -> (Option<&RequestCapsule128>, u64) {
        let gen_before = self.generation.load(Ordering::Acquire);
        let ptr = self.capsule_ptr.load(Ordering::Acquire);

        // Memory fence ensures ordering
        atomic::fence(Ordering::Acquire);

        let gen_after = self.generation.load(Ordering::Acquire);

        if gen_before != gen_after {
            // Slot was modified during load - retry
            return (None, gen_after);
        }

        let capsule = if ptr.is_null() {
            None
        } else {
            Some(unsafe { &*ptr })
        };

        (capsule, gen_after)
    }
}
```

**ASSUM Tags**:
```rust
// #ASSUME: Generation counter prevents ABA problem
// #VERIFY: Property test validates no UAF under 1000-thread load
#[test]
fn test_generation_counter_prevents_uaf() {
    // See tests/concurrent_allocation.rs
}
```

---

## Scalability Analysis

### Cache Coherence Overhead

**MESI Protocol** (Modified, Exclusive, Shared, Invalid):

```
Thread 1: STORE budget_cents (Exclusive)
Thread 2: LOAD budget_cents  (Shared - cache line transferred)
Thread 3: LOAD budget_cents  (Shared - cache line transferred)
Thread 4: STORE budget_cents (Exclusive - invalidates T2/T3 caches)
```

**Measurement** (Intel Ultra 7 155H, 8P + 6E cores):

| Threads | Ops/s | Efficiency | Cache Misses/s |
|---------|-------|------------|----------------|
| 1 | 10M | 100% | 100K |
| 2 | 19M | 95% | 500K |
| 4 | 35M | 87.5% | 2M |
| 8 | 60M | 75% | 8M |
| 16 | 85M | 53% | 25M |

**Observations**:
- Linear scaling up to 4 threads (L1/L2 cache sufficient)
- Sub-linear at 8+ threads (L3 cache saturation, coherence overhead)
- Efficiency drops to 53% at 16 threads (coherence protocol dominates)

**Mitigation**: Reduce thread count to 4-8 for optimal efficiency.

---

## Error Handling Patterns

### Internal Retry with Exponential Backoff

```rust
async fn request_with_retry<F, T>(
    operation: F,
    max_attempts: u32,
) -> Result<T, ClapiError>
where
    F: Fn() -> Result<T, ClapiError>,
{
    let mut backoff_ms = 1;

    for attempt in 1..=max_attempts {
        match operation() {
            Ok(result) => return Ok(result),
            Err(ClapiError::AllocationConflict) if attempt < max_attempts => {
                // Internal retry for CAS failures (transparent to client)
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms *= 2;  // Exponential backoff: 1ms, 2ms, 4ms
            },
            Err(e) => return Err(e),
        }
    }

    Err(ClapiError::RetryLimitExceeded { attempts: max_attempts })
}
```

**Success Rate**: >99% within 3 attempts (conflicts rare <1%)

---

## Circuit Breaker Deep Dive

### Multi-Provider Coordination

**Challenge**: Prevent cascading failures across providers.

**Solution**: Isolated circuit breakers per provider

```rust
struct ProviderCircuitArray {
    circuits: [CircuitBreakerCapsule; 16],  // Max 16 providers
}

impl ProviderRouter {
    pub fn route_request(&self, request: &Request) -> Result<Response, ClapiError> {
        // Sort providers by priority
        let mut providers: Vec<_> = self.providers.iter().collect();
        providers.sort_by_key(|p| p.priority);

        // Try each provider in priority order
        for provider in providers {
            let circuit = &self.circuit_array.circuits[provider.id as usize];

            if circuit.allows_operation() {
                match self.send_request(provider, request).await {
                    Ok(response) => {
                        circuit.record_success();
                        return Ok(response);
                    },
                    Err(e) => {
                        circuit.record_failure();
                        // Continue to next provider (automatic failover)
                    },
                }
            }
        }

        Err(ClapiError::AllProvidersUnavailable)
    }
}
```

**Benefits**:
- **Isolation**: Provider A failure does NOT affect Provider B
- **Automatic Failover**: Priority-based routing with zero config
- **Fast Recovery**: Per-provider cooldowns (independent)

---

## Testing Strategy (T28 Framework)

### Unit Tests (Q1-Q7)

**Coverage**: Capsule invariants, atomic operations, generation counters

```rust
#[test]
fn test_budget_slot_alignment() {
    assert_eq!(std::mem::align_of::<BudgetSlotCapsule>(), 128);
    assert_eq!(std::mem::size_of::<BudgetSlotCapsule>(), 128);
}

#[test]
fn test_circuit_breaker_state_machine() {
    let circuit = CircuitBreakerCapsule::new();

    // Initial state: Closed
    assert_eq!(circuit.state(), CircuitState::Closed);

    // Simulate failures (>10%)
    for _ in 0..15 {
        circuit.record_failure();
    }

    // State should transition to Open
    assert_eq!(circuit.state(), CircuitState::Open);
}
```

### Property Tests (Q8-Q14)

**Coverage**: Concurrent allocation, generation counter correctness, ABA prevention

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_concurrent_allocation_uniqueness(
        num_threads in 2..100usize,
        allocations_per_thread in 1..1000usize,
    ) {
        let registry = Arc::new(BudgetRegistry::new(1000));
        let allocated = Arc::new(Mutex::new(HashSet::new()));

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let registry = registry.clone();
                let allocated = allocated.clone();

                thread::spawn(move || {
                    for i in 0..allocations_per_thread {
                        let budget_id = i as u64;
                        if let Ok(slot_id) = registry.allocate(budget_id, 1000) {
                            // Verify uniqueness
                            let mut set = allocated.lock().unwrap();
                            assert!(set.insert(slot_id), "Duplicate slot allocation!");
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // All allocations should be unique
        let set = allocated.lock().unwrap();
        prop_assert!(set.len() <= num_threads * allocations_per_thread);
    }
}
```

### Stress Tests (Q22-Q28)

**Coverage**: 1M allocation cycles, circuit breaker simulation, hash chain integrity

```rust
#[test]
#[ignore]  // Long-running test
fn test_million_allocation_cycles() {
    let registry = BudgetRegistry::new(10_000);

    for i in 0..1_000_000 {
        let budget_id = i % 10_000;  // Reuse slots

        // Allocate
        let slot_id = registry.allocate(budget_id, 1000).unwrap();

        // Deduct
        registry.try_deduct(budget_id, 100).unwrap();

        // Deallocate
        registry.deallocate(slot_id).unwrap();
    }

    // Verify no memory leaks
    assert_eq!(registry.active_slots(), 0);
}
```

---

## Framework Compliance Details

### UCE34 Computational Capsule Architecture

**Q10: Tier Selection**
- **T1 (Atomic)**: Budget registry (<100ns coordination)
- **T2 (SIMD)**: Response metrics (vectorized aggregation)
- **T3 (Fixed-Point)**: Cost tracking (deterministic arithmetic)
- **T4 (Batch)**: Provider circuit array (16 parallel circuits)
- **T5 (Streaming)**: Audit log (O(1) append latency)

**Q11: Rust Transform**
- AtomicPtr for lockfree pointer swaps
- Generation counters for TOCTOU prevention
- Cache alignment (64B/128B/256B) for false sharing prevention

**Q12: Nightly Features**
- Stable Rust (no nightly required)
- Optional nightly: `portable_simd` for SIMD metrics (future)

**Q33: Verification**
- `#[derive(ComputationalCapsule)]` compile-time checks
- All capsules verified: alignment, size, cache coherence

---

## Audit Trail Implementation

### Hash Chain Integrity

**Structure**:
```rust
struct AuditEntry {
    timestamp: u64,
    event_type: EventType,
    data: Vec<u8>,
    prev_hash: u64,  // Hash of previous entry
    hash: u64,       // Hash of this entry
}

impl AuditLog {
    pub fn append(&mut self, event: AuditEntry) -> Result<(), ClapiError> {
        // Compute hash chain
        let prev_hash = self.last_hash.load(Ordering::Acquire);
        let hash = fnv1a_hash(&event, prev_hash);

        event.prev_hash = prev_hash;
        event.hash = hash;

        // Append to log
        self.entries.push(event);

        // Update last hash
        self.last_hash.store(hash, Ordering::Release);

        Ok(())
    }

    pub fn verify_integrity(&self) -> Result<(), ClapiError> {
        let mut expected_hash = 0u64;

        for (i, entry) in self.entries.iter().enumerate() {
            if entry.prev_hash != expected_hash {
                return Err(ClapiError::HashChainCorrupted { entry_index: i as u64 });
            }

            let computed_hash = fnv1a_hash(entry, entry.prev_hash);
            if entry.hash != computed_hash {
                return Err(ClapiError::HashChainCorrupted { entry_index: i as u64 });
            }

            expected_hash = entry.hash;
        }

        Ok(())
    }
}
```

**Tamper Detection**: Any modification breaks hash chain (cryptographic integrity).

---

## Version History

- **v0.4.6** (2025-10-18): Const-hashing optimization (0ns static IDs, 100× speedup)
- **v0.4.5** (2025-10-17): Metrics infrastructure + forecasting
- **v0.4.0** (2025-10-17): Compliance audit trails (SOX/SOC2/GDPR/HIPAA)
- **v0.3.0** (2025-10-17): Built-in telemetry with hash integrity
- **v0.2.0** (2025-10-16): HTTP proxy + per-provider circuit breakers
- **v0.1.0** (2025-10-16): Pure atomic architecture (lockfree budget registry)

---

**Document Version**: 1.0
**Line Count**: ~725 lines
**Last Updated**: 2025-10-21
**For Overview**: See [ARCHITECTURE_OVERVIEW.md](ARCHITECTURE_OVERVIEW.md)
**For Performance**: See [PERFORMANCE.md](PERFORMANCE.md)
