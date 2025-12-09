# P2 Scaling Architecture: 10K+ Tenant Composition Patterns

**Date**: 2025-10-22
**Framework**: UCE34 Q10.5 (Composition Terminology)
**Status**: Architecture Design (Implementation Pending)

---

## Executive Summary

This document presents the architecture for scaling clapi_core timeline aggregation from **1,000 tenants** (P1 baseline) to **10K+ tenants** (P2 enhancement) using hierarchical composition patterns from the UCE34 framework.

**Key Decision**: **Sharded Container Capsule** (Pattern 4 - 16-shard distribution)

### UCE34 Q1-Q34 Internal Answers

#### Phase 0: Meta-Cognitive Analysis (Q1-Q9)

**Q1: Scope** - Scale timeline aggregation to 10K+ tenants with <1µs P99 lookup
**Q2: Assumptions** - Current DashMap supports 1K tenants @ <500ns, need 10× tenant scale
**Q3: Constraints** - <64GB memory @ 10K tenants (6.4MB per tenant), <1µs lookup budget
**Q4: Context** - SaaS multi-tenant metrics platform, strict per-tenant isolation (compliance)
**Q5: Success** - Linear scalability to 10K tenants, sublinear degradation to 100K
**Q6: Failure** - Memory exhaustion (>64GB), lookup latency >1µs, shard contention
**Q7: Patterns** - UCE34 Q10.5 Container Capsule (management structure for ≥100K capsules)
**Q8: Alternatives** - (1) Flat DashMap (P1 baseline, not scalable), (2) Hierarchical sharding (P2), (3) External DB (rejected: violates lockfree mandate)
**Q9: Trade-offs** - Optimizing for: Scalability (10K→100K tenants) + Memory efficiency (<64GB) + Lookup latency (<1µs)

#### Phase 1: Foundation (Q10-Q12)

**Q10: Computational Capsule** - **T4 (Container Capsule)** for ≥1000 tenants
- **Tier Justification**: Container capsule manages 10K+ timelines with sharded coordination
- **Composition**: Sharded container (16 shards × ConcurrentMapCapsule<u64, Arc<Timeline>>)
- **Pattern**: UCE34 Q10.5 Pattern 4 (Management Structure)

**Q11: Rust Transform** - Leverage atomic_capsule ConcurrentMapCapsule (lockfree, proven in Phase 5.3)
- Use Arc<ConcurrentMapCapsule> for zero-copy shard references
- Const generics for shard count (16/32/64)
- Hash-based sharding: `(tenant_id >> 60) & 0xF` (upper bits for better distribution)

**Q12: Nightly Enhancement** - Optional const_generic shard counts
```rust
#[cfg(feature = "nightly-generic")]
pub struct ShardedMultiTenantCapsule<const SHARDS: usize = 16> { ... }
```

#### Phase 2: Domain Analysis (Q13-Q21)

**Q13: Resources** - Memory: 64GB target @ 10K tenants (6.4MB per tenant), CPU: 16-core AMD/Intel
**Q14: Dependencies** - atomic_capsule (ConcurrentMapCapsule), dashmap (Phase 5.3 replacement pending)
**Q15: Scale** - Linear to 10K tenants, sublinear to 100K (16-shard distribution)
**Q16: Security** - Per-tenant isolation (compliance: SOX, SOC2, GDPR)
**Q17: Interfaces** - `get_or_create(tenant_id) -> Arc<Timeline>`, `query(tenant_id, ts) -> BucketSnapshot`
**Q18: Testing** - T28 framework (unit/property/integration/production)
**Q19: Monitoring** - Shard distribution balance, per-shard contention, memory footprint
**Q20: Error Handling** - Tenant not found, capacity exceeded, shard contention timeout
**Q21: Lifecycle** - Lazy allocation (per tenant), no eviction (P2 scope)

#### Phase 3: Implementation (Q22-Q30)

**Q22: State Management** - 16-shard array, per-shard ConcurrentMapCapsule, atomic shard counters
**Q23: Concurrency** - Lockfree ConcurrentMapCapsule per shard, no cross-shard locks
**Q24: Memory Layout** - 256B aligned container header + 16 Arc<ConcurrentMapCapsule> shards
**Q25: Verification** - `#[derive(ComputationalCapsule)]` for container header
**Q26: Optimization** - Hash-based sharding (upper bits), prefetch shard on lookup
**Q27: Composition** - Container capsule (T4) managing 10K× Timeline capsules (T4 nested)
**Q28: Migration** - P1 DashMap → P2 ShardedMultiTenant (wrapper compatibility layer)
**Q29: Documentation** - Architecture doc (this), migration guide, composition patterns
**Q30: Production** - Comprehensive T28 tests, B32 benchmarks (1K/10K/100K tenants)

#### Phase 4: Refinement (Q31-Q34)

**Q31: Simplicity** - Hide sharding behind `get_or_create(tenant_id)` API (users don't see shards)
**Q32: Practical Constraints** - 64GB RAM limit, 16-core CPU, <1µs latency budget
**Q33: Empirical Validation** - B32 benchmarks validate <1µs P99 @ 10K tenants
**Q34: Auditability** - Per-tenant timeline hash chains (inherited from TimelineAggregationCapsule)

---

## Chosen Composition Pattern: Sharded Container Capsule (Pattern 4)

### Pattern Classification (UCE34 Q10.5)

**Pattern**: **Container Capsule** (Management Structure)
**Tier**: T4 (Batch/Container)
**Capacity**: 10K-100K tenants
**Alignment**: 256B (container header)

### Architecture Overview

```
ShardedMultiTenantCapsule (Container, 256B aligned)
│
├─ Header (256B)
│  ├─ total_tenants: AtomicU64
│  ├─ shard_count: AtomicU64 (16/32/64)
│  ├─ generation: AtomicU64 (TOCTOU prevention)
│  └─ _padding: [u8; ...]
│
└─ Shards: [Arc<ConcurrentMapCapsule<u64, Arc<Timeline>>>; 16]
   ├─ Shard 0: tenant_id % 16 == 0 (~625 tenants @ 10K)
   ├─ Shard 1: tenant_id % 16 == 1 (~625 tenants)
   ├─ ...
   └─ Shard 15: tenant_id % 16 == 15 (~625 tenants)
```

### Sharding Strategy

**Hash Function**: `shard_index = (tenant_id >> 60) & 0xF`
**Rationale**: Upper bits have better distribution than lower bits (avoids sequential tenant_id clustering)

**Alternative**: `shard_index = tenant_id % 16` (simpler, but worse distribution)

**Shard Count Options**:
- **16 shards**: Baseline (625 tenants/shard @ 10K)
- **32 shards**: Low contention (312 tenants/shard @ 10K)
- **64 shards**: Ultra-low contention (156 tenants/shard @ 10K)

**Decision**: Start with 16 shards, scale to 32/64 if >10K tenants

---

## Memory Analysis

### Per-Tenant Memory Footprint

```
Timeline (6.4MB each):
  └─ 100K buckets × 64B = 6.4MB
```

### Total Memory @ 10K Tenants

```
Container Header: 256B
Shards (16): 16 × Arc<ConcurrentMapCapsule> = ~16 × 128B = 2KB
Timelines: 10K × 6.4MB = 64GB

Total: 64GB + 2KB ≈ 64GB
```

### Scalability

| Tenants | Memory | Shards | Tenants/Shard | Verdict |
|---------|--------|--------|---------------|---------|
| 1K | 6.4GB | 16 | 62 | ✅ Baseline |
| 10K | 64GB | 16 | 625 | ✅ Target |
| 100K | 640GB | 64 | 1562 | ⚠️ Requires 64 shards + eviction |

**64GB Limit Analysis**:
- P1 baseline: 1K tenants = 6.4GB (10% utilization)
- P2 target: 10K tenants = 64GB (100% utilization)
- P3 stretch: 100K tenants = 640GB (requires compression or eviction)

---

## Performance Analysis

### Lookup Latency Breakdown

**P1 Baseline** (DashMap, 1K tenants):
```
DashMap lookup: 500ns (16 shards built-in)
Timeline access: 50ns (Arc clone)
Total: 550ns P99
```

**P2 Sharded** (16 shards, 10K tenants):
```
Shard selection: 10ns (bit shift)
ConcurrentMapCapsule lookup: 100ns (lockfree read)
Timeline access: 50ns (Arc clone)
Total: 160ns P50, 500ns P99 (estimated)
```

**P2 Sharded** (32 shards, 10K tenants):
```
Shard selection: 10ns
ConcurrentMapCapsule lookup: 80ns (less contention)
Timeline access: 50ns
Total: 140ns P50, 400ns P99 (estimated)
```

### Contention Analysis

**Shard Load Distribution** (assuming uniform tenant_id distribution):

| Shards | Tenants/Shard @ 10K | Expected Contention | P99 Latency |
|--------|---------------------|---------------------|-------------|
| 16 | 625 | Moderate | <500ns |
| 32 | 312 | Low | <400ns |
| 64 | 156 | Very Low | <300ns |

**Concurrent Threads**:

| Threads | Shards | Contention Risk | Mitigation |
|---------|--------|-----------------|------------|
| 1-16 | 16 | Low | Each thread likely hits different shard |
| 17-64 | 16 | Moderate | 1-4 threads per shard |
| 65-128 | 16 | High | 4-8 threads per shard |
| 17-64 | 32 | Low | 1-2 threads per shard |

**Decision**: 16 shards sufficient for <64 concurrent threads, scale to 32 for >64 threads

---

## Implementation Design

### Core Structure

```rust
use atomic_capsule_derive::ComputationalCapsule;
use atomic_capsule::collections::ConcurrentMapCapsule;
use std::sync::Arc;

/// Sharded multi-tenant timeline container (T4 Container Capsule)
///
/// Manages 10K+ tenants with lockfree sharded access.
///
/// # UCE34 Classification
/// - **Tier**: T4 (Container Capsule)
/// - **Pattern**: Q10.5 Container (Management Structure)
/// - **Capacity**: 10K-100K tenants
/// - **Alignment**: 256B (container header)
///
/// # Memory Layout
/// ```text
/// [0-7]     total_tenants: AtomicU64
/// [8-15]    shard_count: AtomicU64
/// [16-23]   generation: AtomicU64 (TOCTOU prevention)
/// [24-31]   created_at: AtomicU64 (epoch timestamp)
/// [32-255]  _padding: [u8; 224]
/// ```
///
/// # Performance Targets
/// - Lookup: <500ns P99 @ 10K tenants (16 shards)
/// - Lookup: <1µs P99 @ 100K tenants (64 shards)
/// - Memory: <64GB @ 10K tenants (6.4MB per tenant)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
pub struct ShardedMultiTenantCapsuleHeader {
    /// Total tenant count across all shards
    total_tenants: AtomicU64,

    /// Number of shards (16/32/64)
    shard_count: AtomicU64,

    /// Generation counter (TOCTOU prevention)
    generation: AtomicU64,

    /// Creation timestamp (epoch seconds)
    created_at: AtomicU64,

    /// Padding to 256B
    _padding: [u8; 224],
}

/// Sharded multi-tenant container (full structure)
pub struct ShardedMultiTenantCapsule {
    /// Container header (256B aligned)
    header: Arc<ShardedMultiTenantCapsuleHeader>,

    /// Shards (16/32/64 lockfree maps)
    shards: Vec<Arc<ConcurrentMapCapsule<u64, Arc<TimelineAggregationCapsuleCore>>>>,
}

impl ShardedMultiTenantCapsule {
    /// Create new sharded container
    ///
    /// # Arguments
    /// - `shard_count`: Number of shards (16/32/64)
    /// - `granularity`: Bucket granularity for all timelines
    ///
    /// # Performance
    /// - Initialization: <100ms for 16 shards
    /// - Memory: ~2KB (header + shard references)
    pub fn new(shard_count: usize, granularity: BucketGranularity) -> Self {
        let header = Arc::new(ShardedMultiTenantCapsuleHeader {
            total_tenants: AtomicU64::new(0),
            shard_count: AtomicU64::new(shard_count as u64),
            generation: AtomicU64::new(0),
            created_at: AtomicU64::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ),
            _padding: [0u8; 224],
        });

        let shards = (0..shard_count)
            .map(|_| Arc::new(ConcurrentMapCapsule::new()))
            .collect();

        Self { header, shards }
    }

    /// Get shard index for tenant
    ///
    /// Uses upper bits of tenant_id for better distribution.
    ///
    /// # Performance
    /// - Target: <10ns (bit shift + mask)
    #[inline(always)]
    fn shard_index(&self, tenant_id: u64) -> usize {
        let shard_count = self.header.shard_count.load(Ordering::Relaxed) as usize;

        // Use upper bits for better distribution
        // For 16 shards: extract 4 bits from position 60-63
        let shift = 64 - (shard_count.trailing_zeros() + shard_count.count_ones() - 1);
        let mask = shard_count - 1;

        ((tenant_id >> shift) as usize) & mask
    }

    /// Get or create timeline for tenant (lockfree)
    ///
    /// # Arguments
    /// - `tenant_id`: Unique tenant identifier
    ///
    /// # Returns
    /// - Arc<TimelineAggregationCapsuleCore> for this tenant
    ///
    /// # Performance
    /// - Fast path (existing): <160ns P50, <500ns P99
    /// - Slow path (new tenant): <1.5ms (allocation)
    ///
    /// # Scalability
    /// - 10K tenants: <500ns P99 (16 shards)
    /// - 100K tenants: <1µs P99 (64 shards)
    pub fn get_or_create(&self, tenant_id: u64) -> Arc<TimelineAggregationCapsuleCore> {
        let shard_idx = self.shard_index(tenant_id);
        let shard = &self.shards[shard_idx];

        // Lockfree get_or_insert
        shard.get_or_insert(tenant_id, || {
            // Increment total tenants counter
            self.header.total_tenants.fetch_add(1, Ordering::Relaxed);

            // Create new timeline (6.4MB allocation)
            TimelineAggregationCapsuleCore::new(
                0, // Start at epoch 0
                BucketGranularity::Minute,
                100_000, // 100K buckets
            )
        })
    }

    /// Append event to tenant timeline
    ///
    /// # Performance
    /// - Lookup: <500ns P99
    /// - Append: <100ns (lockfree)
    /// - Total: <600ns P99
    pub fn append(&self, tenant_id: u64, event_ts: u64) -> ClapiResult<()> {
        let timeline = self.get_or_create(tenant_id);
        timeline.append(event_ts)
    }

    /// Query tenant timeline
    pub fn query(&self, tenant_id: u64, ts: u64) -> ClapiResult<BucketSnapshot> {
        let timeline = self.get_or_create(tenant_id);
        timeline.query_by_timestamp(ts)
    }

    /// Get total tenant count (lockfree read)
    #[inline(always)]
    pub fn total_tenants(&self) -> u64 {
        self.header.total_tenants.load(Ordering::Relaxed)
    }

    /// Get shard distribution statistics
    pub fn shard_stats(&self) -> Vec<ShardStats> {
        self.shards
            .iter()
            .enumerate()
            .map(|(idx, shard)| {
                ShardStats {
                    shard_id: idx,
                    tenant_count: shard.len(),
                    // Additional metrics from ConcurrentMapCapsule
                }
            })
            .collect()
    }
}

/// Shard statistics for monitoring
#[derive(Debug, Clone)]
pub struct ShardStats {
    pub shard_id: usize,
    pub tenant_count: usize,
}
```

---

## Migration Guide: P1 → P2

### Compatibility Layer

```rust
/// Compatibility wrapper for P1 → P2 migration
pub struct MultiTenantTimelineCapsule {
    inner: ShardedMultiTenantCapsule,
}

impl MultiTenantTimelineCapsule {
    /// Create new multi-tenant timeline (P1 API)
    pub fn new(granularity: BucketGranularity) -> Self {
        // Use P2 implementation with 16 shards
        Self {
            inner: ShardedMultiTenantCapsule::new(16, granularity),
        }
    }

    /// Append event (P1 API unchanged)
    pub fn append(&self, tenant_id: u64, event_ts: u64) -> ClapiResult<()> {
        self.inner.append(tenant_id, event_ts)
    }

    /// Query timeline (P1 API unchanged)
    pub fn query(&self, tenant_id: u64, ts: u64) -> ClapiResult<BucketSnapshot> {
        self.inner.query(tenant_id, ts)
    }
}
```

**Migration Steps**:
1. Replace `MultiTenantTimelineCapsule` implementation with sharded version
2. All P1 APIs remain unchanged (drop-in replacement)
3. No code changes required in consumers
4. Monitor shard distribution via `shard_stats()`

---

## Benchmarking Strategy (B32 Framework)

### Benchmark Suite 1: Tenant Lookup Scalability

**Goal**: Validate <1µs P99 @ 10K tenants

```rust
fn bench_p2_tenant_lookup_scalability(c: &mut Criterion) {
    for num_tenants in [1_000, 10_000, 100_000] {
        for num_shards in [16, 32, 64] {
            let mt = ShardedMultiTenantCapsule::new(num_shards, BucketGranularity::Minute);

            // Pre-populate tenants
            for i in 0..num_tenants {
                mt.get_or_create(i);
            }

            // Measure lookup latency
            c.bench_function(&format!("lookup_{}tenants_{}shards", num_tenants, num_shards), |b| {
                b.iter(|| {
                    let tenant_id = rand::random::<u64>() % num_tenants;
                    black_box(mt.get_or_create(tenant_id))
                })
            });
        }
    }
}
```

**Expected Results**:

| Tenants | Shards | P50 | P99 | Budget | Verdict |
|---------|--------|-----|-----|--------|---------|
| 1K | 16 | 100ns | 300ns | <1µs | ✅ PASS |
| 10K | 16 | 150ns | 500ns | <1µs | ✅ PASS |
| 10K | 32 | 120ns | 400ns | <1µs | ✅ PASS |
| 100K | 64 | 200ns | 800ns | <1µs | ✅ PASS |

### Benchmark Suite 2: Shard Distribution Balance

**Goal**: Validate uniform distribution across shards

```rust
fn bench_p2_shard_distribution(c: &mut Criterion) {
    let mt = ShardedMultiTenantCapsule::new(16, BucketGranularity::Minute);

    // Insert 10K tenants
    for i in 0..10_000 {
        mt.get_or_create(i);
    }

    // Measure distribution
    let stats = mt.shard_stats();
    for (idx, stat) in stats.iter().enumerate() {
        println!("Shard {}: {} tenants", idx, stat.tenant_count);
    }

    // Assert balanced distribution (each shard should have ~625 ± 10%)
    let expected = 10_000 / 16;
    let tolerance = expected / 10;
    for stat in &stats {
        assert!(
            stat.tenant_count >= expected - tolerance &&
            stat.tenant_count <= expected + tolerance,
            "Shard {} has {} tenants (expected {}±{})",
            stat.shard_id, stat.tenant_count, expected, tolerance
        );
    }
}
```

### Benchmark Suite 3: Concurrent Access (Contention)

**Goal**: Validate scalability under concurrent load

```rust
fn bench_p2_concurrent_access(c: &mut Criterion) {
    for num_threads in [1, 2, 4, 8, 16, 32, 64] {
        let mt = Arc::new(ShardedMultiTenantCapsule::new(16, BucketGranularity::Minute));

        // Pre-populate 10K tenants
        for i in 0..10_000 {
            mt.get_or_create(i);
        }

        c.bench_function(&format!("concurrent_{}threads_10k_tenants", num_threads), |b| {
            b.iter(|| {
                let handles: Vec<_> = (0..num_threads)
                    .map(|_| {
                        let mt_clone = Arc::clone(&mt);
                        std::thread::spawn(move || {
                            for _ in 0..1000 {
                                let tenant_id = rand::random::<u64>() % 10_000;
                                black_box(mt_clone.get_or_create(tenant_id));
                            }
                        })
                    })
                    .collect();

                for handle in handles {
                    handle.join().unwrap();
                }
            })
        });
    }
}
```

**Expected Results**:

| Threads | Throughput | Latency P99 | Scalability | Verdict |
|---------|------------|-------------|-------------|---------|
| 1 | 2M ops/s | 500ns | 1× | ✅ Baseline |
| 16 | 25M ops/s | 800ns | 12.5× | ✅ Linear |
| 64 | 80M ops/s | 1.2µs | 40× | ✅ Sublinear (contention) |

---

## Testing Strategy (T28 Framework)

### Unit Tests (Q1-Q7)

```rust
#[test]
fn test_shard_index_distribution() {
    let mt = ShardedMultiTenantCapsule::new(16, BucketGranularity::Minute);

    // Test 1000 random tenant IDs
    let mut shard_counts = vec![0; 16];
    for _ in 0..1000 {
        let tenant_id = rand::random::<u64>();
        let shard_idx = mt.shard_index(tenant_id);
        shard_counts[shard_idx] += 1;
    }

    // Assert roughly uniform distribution (±20%)
    let expected = 1000 / 16; // 62.5
    for count in shard_counts {
        assert!(count >= expected * 8 / 10 && count <= expected * 12 / 10);
    }
}

#[test]
fn test_get_or_create_idempotent() {
    let mt = ShardedMultiTenantCapsule::new(16, BucketGranularity::Minute);

    let timeline1 = mt.get_or_create(123);
    let timeline2 = mt.get_or_create(123);

    // Should return same Arc (pointer equality)
    assert!(Arc::ptr_eq(&timeline1, &timeline2));
}

#[test]
fn test_total_tenants_counter() {
    let mt = ShardedMultiTenantCapsule::new(16, BucketGranularity::Minute);

    for i in 0..100 {
        mt.get_or_create(i);
    }

    assert_eq!(mt.total_tenants(), 100);
}
```

### Property Tests (Q8-Q14)

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_shard_deterministic(tenant_id in 0u64..1_000_000) {
        let mt = ShardedMultiTenantCapsule::new(16, BucketGranularity::Minute);

        // Same tenant_id always maps to same shard
        let shard1 = mt.shard_index(tenant_id);
        let shard2 = mt.shard_index(tenant_id);
        prop_assert_eq!(shard1, shard2);
    }

    #[test]
    fn prop_concurrent_get_or_create_safe(
        tenant_ids in prop::collection::vec(0u64..10_000, 1000)
    ) {
        let mt = Arc::new(ShardedMultiTenantCapsule::new(16, BucketGranularity::Minute));

        // Spawn 10 threads, each getting/creating random tenants
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let mt_clone = Arc::clone(&mt);
                let ids = tenant_ids.clone();
                std::thread::spawn(move || {
                    for tenant_id in ids {
                        mt_clone.get_or_create(tenant_id);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify total_tenants counter is correct
        let unique_tenants: std::collections::HashSet<_> = tenant_ids.into_iter().collect();
        prop_assert_eq!(mt.total_tenants(), unique_tenants.len() as u64);
    }
}
```

### Integration Tests (Q15-Q21)

```rust
#[test]
fn test_p1_to_p2_compatibility() {
    // P1 API
    let p1 = MultiTenantTimelineCapsule::new(BucketGranularity::Minute);
    p1.append(1, 1000).unwrap();
    p1.append(1, 1060).unwrap();

    let snapshot = p1.query(1, 1000).unwrap();
    assert_eq!(snapshot.event_count, 2);

    // P2 API (same behavior)
    let p2 = ShardedMultiTenantCapsule::new(16, BucketGranularity::Minute);
    p2.append(1, 1000).unwrap();
    p2.append(1, 1060).unwrap();

    let snapshot = p2.query(1, 1000).unwrap();
    assert_eq!(snapshot.event_count, 2);
}

#[test]
fn test_10k_tenant_stress() {
    let mt = Arc::new(ShardedMultiTenantCapsule::new(16, BucketGranularity::Minute));

    // Create 10K tenants concurrently
    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let mt_clone = Arc::clone(&mt);
            std::thread::spawn(move || {
                for i in 0..1000 {
                    let tenant_id = thread_id * 1000 + i;
                    let timeline = mt_clone.get_or_create(tenant_id);
                    timeline.append(1000 + i).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all 10K tenants exist
    assert_eq!(mt.total_tenants(), 10_000);

    // Verify shard distribution is balanced
    let stats = mt.shard_stats();
    let avg = 10_000 / 16; // 625
    for stat in stats {
        assert!(stat.tenant_count >= avg * 8 / 10); // Within 20%
    }
}
```

### Production Tests (Q22-Q28)

```rust
#[test]
fn test_memory_footprint_10k_tenants() {
    let mt = Arc::new(ShardedMultiTenantCapsule::new(16, BucketGranularity::Minute));

    // Measure baseline memory
    let baseline_rss = get_process_rss();

    // Create 10K tenants
    for i in 0..10_000 {
        mt.get_or_create(i);
    }

    // Measure final memory
    let final_rss = get_process_rss();
    let memory_used = final_rss - baseline_rss;

    // Assert memory is within budget (64GB + 10% overhead)
    let expected = 64 * 1024 * 1024 * 1024; // 64GB
    let tolerance = expected / 10; // 10%
    assert!(
        memory_used <= expected + tolerance,
        "Memory used: {} bytes (expected: {} ± {})",
        memory_used, expected, tolerance
    );
}

fn get_process_rss() -> usize {
    // Implementation: Read /proc/self/status on Linux
    // Or use platform-specific APIs
    unimplemented!("Platform-specific implementation")
}
```

---

## Future Optimization Opportunities (P3: 100K+ Tenants)

### 1. Eviction Policy (LRU)

**Problem**: 100K tenants = 640GB (exceeds 64GB budget)
**Solution**: LRU eviction of inactive tenants

```rust
struct EvictableTimeline {
    timeline: Arc<TimelineAggregationCapsuleCore>,
    last_access: AtomicU64, // Timestamp
}

impl ShardedMultiTenantCapsule {
    fn evict_lru(&self, target_count: usize) {
        // Scan all shards, find oldest timelines
        // Evict until target_count reached
    }
}
```

### 2. Compression (Downsampling)

**Problem**: Old data at full resolution wastes memory
**Solution**: Downsample old buckets (minute → hour → day)

```rust
impl TimelineAggregationCapsuleCore {
    fn downsample_old_buckets(&self, cutoff_age: Duration) {
        // Aggregate old minute buckets into hour buckets
        // Free minute buckets
    }
}
```

### 3. Tiered Storage (Hot/Warm/Cold)

**Problem**: Active tenants need fast access, inactive can be slower
**Solution**: Tier by access frequency

```rust
enum TenantTier {
    Hot,   // In-memory, <500ns
    Warm,  // Compressed, <10µs
    Cold,  // External DB, <1ms
}
```

### 4. Dynamic Shard Count

**Problem**: 16 shards optimal for 10K, but not 100K
**Solution**: Increase shard count dynamically

```rust
impl ShardedMultiTenantCapsule {
    fn increase_shard_count(&mut self, new_count: usize) {
        // Redistribute tenants across new shards
        // Requires brief write lock (rare operation)
    }
}
```

---

## Deployment Checklist

### Pre-Deployment Validation

- [ ] All T28 tests pass (unit/property/integration/production)
- [ ] B32 benchmarks validate <1µs P99 @ 10K tenants
- [ ] Memory footprint <64GB @ 10K tenants
- [ ] Shard distribution balanced (±10%)
- [ ] P1 API compatibility verified

### Rollout Strategy (I20 Framework)

**Strategy**: Big-Bang Deployment (100% immediately)
**Rationale**: Deterministic capsule code (Q19 from I20 Framework)

**Rollback Plan**: Git revert + redeploy (<5 minutes)

### Monitoring Metrics

- **Tenant count**: `total_tenants()` (gauge)
- **Shard distribution**: `shard_stats()` (histogram)
- **Lookup latency**: P50/P99/P999 (histogram)
- **Memory RSS**: Process memory (gauge)
- **Errors**: Capacity exceeded, lookup failures (counter)

---

## Conclusion

### Key Decisions

1. **Pattern**: Sharded Container Capsule (UCE34 Q10.5 Pattern 4)
2. **Shards**: 16 baseline, scale to 32/64 for >10K tenants
3. **Hash**: Upper bits (better distribution than modulo)
4. **Compatibility**: Drop-in replacement for P1 API
5. **Scalability**: Linear to 10K, sublinear to 100K

### Performance Summary

| Metric | P1 Baseline | P2 Target | P2 Achieved (Estimated) | Verdict |
|--------|-------------|-----------|-------------------------|---------|
| Tenants | 1K | 10K | 10K | ✅ |
| Memory | 6.4GB | <64GB | 64GB | ✅ |
| Lookup P99 | 550ns | <1µs | 500ns | ✅ |
| Scalability | Linear | Linear | Linear (16 shards) | ✅ |

### Next Steps

1. Implement `ShardedMultiTenantCapsule` (see code above)
2. Write comprehensive T28 tests (unit/property/integration/production)
3. Create B32 benchmarks (1K/10K/100K tenants)
4. Validate memory footprint @ 10K tenants
5. Deploy to staging (I20 big-bang strategy)
6. Monitor shard distribution and latency
7. Optimize shard count if needed (32/64 shards)

---

**Status**: Architecture Design Complete
**Implementation**: Pending (estimate 2-3 days)
**Framework Compliance**: UCE34 Q1-Q34, I20 Q1-Q20, B32, T28, ASSUM
**Date**: 2025-10-22
