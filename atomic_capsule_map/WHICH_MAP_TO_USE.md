# Which Concurrent Map Should I Use?

Decision matrix for choosing between DashMap and AtomicCapsuleMap based on workload characteristics.

## Quick Decision Tree

```
┌─────────────────────────────────────┐
│ Do you need a concurrent HashMap?  │
└─────────────────────────────────────┘
              │
              ├─ YES → Continue below
              └─ NO  → Use std::collections::HashMap

┌─────────────────────────────────────┐
│ Is your workload >95% reads?       │
└─────────────────────────────────────┘
              │
              ├─ YES → AtomicCapsuleMap ✅
              └─ NO  → Continue below

┌─────────────────────────────────────┐
│ Do writes happen frequently?        │
│ (>30% of operations)                │
└─────────────────────────────────────┘
              │
              ├─ YES → DashMap ✅
              └─ NO  → Continue below

┌─────────────────────────────────────┐
│ Is predictable latency critical?    │
│ (Real-time system)                  │
└─────────────────────────────────────┘
              │
              ├─ YES → AtomicCapsuleMap ✅
              └─ NO  → DashMap ✅ (default choice)
```

## Detailed Comparison Matrix

| Criterion | AtomicCapsuleMap | DashMap | Recommendation |
|-----------|------------------|---------|----------------|
| **Read Performance** | 7.6 ns (2.2× faster) | 17 ns | AtomicCapsuleMap ✅ |
| **Write Performance** | 362 ns (9.9× slower) | 36 ns | DashMap ✅ |
| **Mixed Workload** | 56 ns (3× slower) | 19 ns | DashMap ✅ |
| **Predictable Latency** | Yes (lockfree) | No (lock waiting) | AtomicCapsuleMap ✅ |
| **Memory Efficiency** | Higher (generation counters) | Lower (compact) | DashMap ✅ |
| **Production Stability** | New (v0.1.0) | Battle-tested (v6.1.0) | DashMap ✅ |
| **Contention Handling** | Moderate | Excellent | DashMap ✅ |
| **Ease of Use** | Same API | Same API | Tie ⚖️ |

## Workload Profiles

### Profile 1: Configuration Cache
- **Characteristics**: Read once at startup, never modified
- **Read/Write Ratio**: 99.99% / 0.01%
- **Recommendation**: **AtomicCapsuleMap** ✅
- **Why**: 2.2× faster reads, writes are rare

### Profile 2: Session Store
- **Characteristics**: Frequent updates, lookups, and expirations
- **Read/Write Ratio**: 60% / 40%
- **Recommendation**: **DashMap** ✅
- **Why**: 9.9× faster writes, 3× faster mixed workload

### Profile 3: Routing Table
- **Characteristics**: Lookup-heavy, occasional route updates
- **Read/Write Ratio**: 95% / 5%
- **Recommendation**: **AtomicCapsuleMap** ✅
- **Why**: Read-dominated, predictable latency for packet forwarding

### Profile 4: Metrics Aggregation
- **Characteristics**: Constant counter updates
- **Read/Write Ratio**: 20% / 80%
- **Recommendation**: **DashMap** ✅
- **Why**: Write-heavy workload, DashMap excels here

### Profile 5: Feature Flags
- **Characteristics**: Read on every request, updated via admin API
- **Read/Write Ratio**: 99.9% / 0.1%
- **Recommendation**: **AtomicCapsuleMap** ✅
- **Why**: Extremely read-heavy, predictable latency

### Profile 6: Cache with LRU Eviction
- **Characteristics**: Frequent reads and writes (cache misses)
- **Read/Write Ratio**: 70% / 30%
- **Recommendation**: **DashMap** ✅
- **Why**: Mixed workload with significant write component

### Profile 7: Real-Time Trading System
- **Characteristics**: Ultra-low latency requirement, read-heavy
- **Read/Write Ratio**: 98% / 2%
- **Recommendation**: **AtomicCapsuleMap** ✅
- **Why**: Predictable latency critical, no lock waiting

### Profile 8: Web Application State
- **Characteristics**: General-purpose state management
- **Read/Write Ratio**: 50% / 50%
- **Recommendation**: **DashMap** ✅
- **Why**: Balanced workload, production stability

## Benchmark-Backed Recommendations

### When AtomicCapsuleMap is Objectively Better:

1. **Read-Dominated Workloads (>95% reads)**
   - Benchmark: 2.2× faster at 7.6ns vs 17ns
   - Statistical significance: p < 0.001
   - Use case: Configuration, routing, feature flags

2. **Predictable Latency Requirements**
   - Benchmark: Lower std dev (±0.03ns vs ±0.59ns)
   - No lock waiting = no tail latency spikes
   - Use case: Real-time systems, hard deadlines

3. **Low Contention Scenarios (<4 threads)**
   - Benchmark: Similar or better performance
   - Use case: Embedded systems, single-server apps

### When DashMap is Objectively Better:

1. **Write-Heavy Workloads (>30% writes)**
   - Benchmark: 9.9× faster INSERT, 1.9× faster UPDATE
   - Statistical significance: p < 0.001
   - Use case: Caches with eviction, metrics, state management

2. **Mixed Workloads**
   - Benchmark: 3× faster overall (70/30 read/write)
   - Use case: Most real-world applications

3. **Production Stability**
   - Version: 6.1.0 (mature, battle-tested)
   - Use case: Any production system requiring reliability

4. **Memory Efficiency**
   - DashMap: Compact storage without generation counters
   - Use case: Large maps (millions of entries)

5. **High Contention (8+ threads)**
   - Benchmark: Better contention handling
   - Use case: Multi-threaded servers

## Migration Guide

### From std::HashMap to Concurrent Map

If you're currently using `std::HashMap` with `Arc<RwLock<HashMap>>`:

```rust
// Before: Manual locking
let map = Arc::new(RwLock::new(HashMap::new()));
let value = map.read().unwrap().get(&key).cloned();

// After: Choose based on workload
// If read-heavy (>95%):
let map = AtomicCapsuleMap::new();
let value = map.get(&key);

// If balanced or write-heavy:
let map = DashMap::new();
let value = map.get(&key).map(|v| v.clone());
```

### From DashMap to AtomicCapsuleMap

**Only migrate if**:
1. Profiling shows read latency is bottleneck
2. Workload is >95% reads
3. Predictable latency is critical

**Do NOT migrate if**:
1. Workload has >5% writes
2. Production stability is critical
3. Memory efficiency matters
4. You haven't profiled (premature optimization)

## Performance Expectations

### AtomicCapsuleMap

```
GET operations:     ~8 ns   (excellent)
INSERT operations:  ~360 ns (poor)
UPDATE operations:  ~32 ns  (moderate)
Mixed workload:     ~56 ns  (poor)
```

**Sweet spot**: Read-dominated workloads with <5% writes

### DashMap

```
GET operations:     ~17 ns  (good)
INSERT operations:  ~36 ns  (excellent)
UPDATE operations:  ~17 ns  (excellent)
Mixed workload:     ~19 ns  (excellent)
```

**Sweet spot**: General-purpose concurrent maps

## Real-World Examples

### Example 1: HTTP Router (AtomicCapsuleMap ✅)

```rust
// Routes are registered at startup, never modified
let router = AtomicCapsuleMap::new();
router.insert("/api/users", handler1);
router.insert("/api/posts", handler2);

// Hot path: route lookup on every request
fn handle_request(router: &AtomicCapsuleMap, path: &str) {
    if let Some(handler) = router.get(path) {
        handler(); // 7.6ns lookup vs 17ns with DashMap
    }
}
```

**Why AtomicCapsuleMap**: 2.2× faster lookups, routes never change

### Example 2: Session Store (DashMap ✅)

```rust
// Sessions are constantly created, updated, expired
let sessions = DashMap::new();

// Frequent writes
sessions.insert(session_id, session); // 36ns vs 360ns with AtomicCapsuleMap
sessions.remove(&expired_id);
sessions.alter(&id, |_, mut session| {
    session.last_accessed = now();
    session
});
```

**Why DashMap**: 9.9× faster writes, mixed workload 3× faster

### Example 3: Feature Flags (AtomicCapsuleMap ✅)

```rust
// Flags are updated via admin API (rare), checked on every request (frequent)
let flags = AtomicCapsuleMap::new();

// Hot path: flag check on every request
fn is_enabled(flags: &AtomicCapsuleMap, feature: &str) -> bool {
    flags.get(feature).map(|v| *v).unwrap_or(false) // 7.6ns
}

// Cold path: admin update (rare)
fn update_flag(flags: &AtomicCapsuleMap, feature: String, enabled: bool) {
    flags.insert(feature, enabled); // 360ns, but happens rarely
}
```

**Why AtomicCapsuleMap**: 99.9% reads, write overhead acceptable for rare updates

## FAQ

### Q: Should I always use DashMap?

**A**: Yes, **unless** you have:
1. Proven read-dominated workload (>95% reads)
2. Measured performance bottleneck in map reads
3. Acceptable write overhead (rare writes)

### Q: Is AtomicCapsuleMap production-ready?

**A**: Depends on your use case:
- ✅ Yes for read-heavy, low-write scenarios
- ❌ No for general-purpose use (DashMap is better)
- ⚠️ Carefully evaluate for production (new library, v0.1.0)

### Q: Can I mix both in the same application?

**A**: Yes! Use the right tool for each use case:
```rust
// Read-heavy: routing table
let routes = AtomicCapsuleMap::new();

// Write-heavy: session store
let sessions = DashMap::new();
```

### Q: What about std::sync::RwLock<HashMap>?

**A**: Both AtomicCapsuleMap and DashMap are better:
- DashMap: Sharded locking for better concurrency
- AtomicCapsuleMap: Lockfree for predictable latency
- RwLock<HashMap>: Single lock = contention bottleneck

### Q: Performance claims sound too good to be true?

**A**: Our claims are **statistically validated**:
- ✅ "2.2× faster reads" - TRUE (7.6ns vs 17ns, p < 0.001)
- ❌ "Faster overall" - FALSE (DashMap wins 5/7 benchmarks)
- ✅ "No lock waiting" - TRUE (architecture guarantee)

We only claim what benchmarks prove. See `DASHMAP_COMPARISON.md` for full data.

## Conclusion

**Default choice**: **DashMap** for general-purpose concurrent maps.

**Specialized choice**: **AtomicCapsuleMap** for read-dominated workloads with predictable latency requirements.

**Rule of thumb**: If unsure, use DashMap. Only switch to AtomicCapsuleMap if profiling proves read latency is a bottleneck and workload is >95% reads.

---

**Last Updated**: 2025-10-03
**Benchmark Data**: See `DASHMAP_COMPARISON.md`
**Framework**: B32 (Honest benchmarking, realistic expectations)
