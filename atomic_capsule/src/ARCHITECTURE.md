# Atomic Capsule Architecture

## Capsule Taxonomy (Updated Oct 2025)

This document provides a comprehensive taxonomy of computational capsules, including the new composite and container capsule categories introduced with UCE35 Q10.5.

### Tier-Based Classification

**Foundation Tiers** (Production-Ready):
- **T1 (Atomic)**: Lockfree coordination, <100ns operations, 64B alignment
- **T2 (SIMD)**: Vectorized computation, 2-19× speedup, 32-64B alignment
- **T3 (Fixed-Point)**: Deterministic arithmetic, 2-10× speedup, 16-64B alignment
- **T4 (Batch)**: High throughput, 10-100× speedup, preallocated arrays
- **T5 (Streaming)**: Continuous processing, O(1) latency, windowed computation
- **T6 (Mixed)**: Compound optimizations, 12-2000× speedup, multi-tier combinations

**Extended Tiers** (Frontier):
- **T7 (GPU)**: Massive parallelism, 100-1000× potential
- **T8 (Network)**: Zero-copy packet processing, 10-50× throughput
- **T9 (Persistent)**: Crash-safe storage, ACID guarantees
- **T10 (Probabilistic)**: Sketches/filters, 100-1000× memory reduction

### Composition-Based Classification (New - UCE35)

#### Composite Capsule (Flat Multi-Tier)

**Definition**: Single struct combining fields from multiple tiers in flat layout

**Characteristics**:
- **Structure**: All fields inline (no nested indirection)
- **Scale**: <10K objects
- **Tiers**: 2-3 tier combinations (T1+T2, T1+T3, T2+T3, T1+T2+T3)
- **Alignment**: Max of component tiers (128B for T1+T2)
- **Speedup**: 12-24× compound (3× × 4× × 2×)
- **Memory**: Increased due to alignment requirements
- **Complexity**: Moderate (managed through type system)

**Patterns**:
- **T1+T1**: DualAtomicU64 (128B, 2.1× vs unaligned, 67 production uses)
- **T1+T2**: AtomicSimdCapsule (128B, 12× compound speedup)
- **T1+T3**: AtomicFixedPointCapsule (64B, lockfree + deterministic, 83.4ns)
- **T2+T3**: SimdFixedPointCapsule (64B, 8× compound speedup)
- **T1+T2+T3**: AtomicSimdFixedPoint (128B, 24× compound, hypothetical)

**Examples**:
```rust
#[repr(C, align(128))]
pub struct AtomicSimdCapsule {
    // Tier 1: Atomic coordination (cache line 1)
    generation: AtomicU64,
    _padding1: [u8; 56],

    // Tier 2: SIMD data (cache line 2)
    data: [f32; 8],
    _padding2: [u8; 32],
}
```

**When to Use**:
- Need multiple optimizations (coordination + vectorization + precision)
- <10K objects (flat composition optimal at this scale)
- Compound speedup justifies memory overhead
- All fields fit in 2-3 cache lines

#### Container Capsule (Management Structure)

**Definition**: Management structure coordinating ≥100K capsules with infrastructure

**Characteristics**:
- **Structure**: Preallocated array + header + circuit breaker + counters
- **Scale**: ≥100K objects (management overhead amortized)
- **Infrastructure**: Circuit breaker, generation counters, hash chains, metrics
- **Overhead**: 50ms init + 15ns/op (amortized at scale)
- **ROI**: Breaks even at ~700K operations
- **Access**: O(1) lookup (hash map or array indexing)
- **Lifetime**: Long-lived (hours+)

**Components**:
- **Header**: Coordination metadata (128B aligned)
- **Slots**: Preallocated Box<[T; N]> array
- **Circuit Breaker**: Failure isolation
- **Generation Counters**: TOCTOU prevention
- **Metrics**: Atomic counters for monitoring

**Patterns**:
- **BudgetMetaCapsule**: 1M slots, 128MB, circuit breaker (clapi_core)
- **FullBrain**: 13 zones, 960K neurons, 100× speedup (kindly_hft)
- **ConcurrentMapCapsule**: 16K entries, 2MB, 3-59× speedup (Phase 5.3)

**Examples**:
```rust
pub struct BudgetMetaCapsule {
    // Header: Coordination metadata (128B)
    header: BudgetMetaCapsuleHeader,

    // Slots: Preallocated array (128MB for 1M × 128B)
    slots: Box<[BudgetSlotCapsule; 1_000_000]>,

    // Circuit Breaker: Isolation (64B)
    circuit_breaker: CircuitBreakerCapsule,
}
```

**When to Use**:
- Managing ≥100K capsules
- Need isolation (circuit breaker prevents cascading failures)
- Long-lived system (hours+, init cost amortized)
- O(1) access pattern required
- ROI positive (init cost < operational savings)

### Size-Based Classification

**Small Capsules** (64B):
- Single cache line
- Tier 1 (Atomic) or Tier 3 (Fixed-Point)
- Examples: CircuitBreakerCapsule (64B), PnlCapsule (64B)

**Medium Capsules** (128B):
- Dual cache line
- Tier 1+1 (DualAtomicU64) or Tier 1+2 (AtomicSimdCapsule)
- Examples: DualAtomicU64 (128B), ParticleCapsule (128B)

**Large Capsules** (256B+):
- Multiple cache lines
- Complex state or Tier 6 (Mixed)
- Examples: RequestCapsule256 (256B with hash chain)

**Container Capsules** (MB-GB):
- Management structures
- ≥100K slots or zones
- Examples: BudgetMetaCapsule (128MB), FullBrain (54GB)

### Alignment-Based Classification

**64B Aligned** (Single Cache Line):
- Tier 1 (Atomic), Tier 3 (Fixed-Point)
- Examples: CircuitBreakerCapsule, PnlCapsule

**128B Aligned** (Dual Cache Line):
- Tier 1+1, Tier 1+2, Tier 1+2+3
- Prevents false sharing between atomic and SIMD
- Examples: DualAtomicU64, AtomicSimdCapsule, BudgetSlotCapsule

**256B Aligned** (Quad Cache Line):
- Complex Tier 6 (Mixed) or large state
- Examples: RequestCapsule256 (with hash chain)

### Decision Framework

**Q10.5 Decision Tree** (UCE34_FRAMEWORK.md):

```
START: After tier selection (Q10)
│
├─ Single optimization? → Single tier capsule (T1 OR T2 OR T3)
├─ Two optimizations? → Composite capsule (flat T1+T2 or T1+T3 or T2+T3)
├─ Three optimizations? → Composite capsule (flat T1+T2+T3, rare)
└─ Managing ≥100K capsules? → Container capsule (management structure)
```

**Scale Thresholds**:
- <100 objects: Direct array (stack or small Box<[T; N]>)
- 100-10K objects: Vec or composite capsule
- 10K-100K objects: Consider preallocated array
- ≥100K objects: Container capsule with infrastructure

### Cross-References

**Framework Documentation**:
- **UCE34_FRAMEWORK.md Q10.5**: Composition architecture decisions
- **UCE34_TIER_REFERENCE.md § 15**: Detailed composition patterns
- **UCE34_EXAMPLES.md**: Composite and container capsule code examples

**Pattern Documentation**:
- **ATOMIC_CAPSULE_PATTERNS.md**: 5 production patterns
- **ATOMIC_CAPSULE_COMPOSITION.md**: Safe composition patterns
- **ATOMIC_CAPSULE_FAILURE_MODES.md**: Failure analysis

**Implementation**:
- **atomic_capsule crate**: Foundation primitives
- **DualAtomicU64**: 67 production uses (kindly_hft)
- **BudgetMetaCapsule**: Container pattern (clapi_core)
- **FullBrain**: 13-zone container (kindly_hft)

### Production Statistics

**Composite Capsules**:
- 67 uses: DualAtomicU64 (T1+T1, 128B, 2.1× vs unaligned)
- 19× speedup: SimdHebbianCapsule (T1+T2, 128B, Hebbian learning)
- 83.4ns: AtomicFixedPointCapsule (T1+T3, 64B, deterministic P&L)
- 2-4× speedup: SimdFixedPointQ16x8 (T2+T3, 64B, Phase 2.1)

**Container Capsules**:
- 1M slots: BudgetMetaCapsule (128MB, 50ms init, 30ns/op)
- 960K neurons: FullBrain (13 zones, 54GB, 100× checkpoint speedup)
- 16K entries: ConcurrentMapCapsule (2MB, 3-59× vs DashMap)

**Anti-Patterns Avoided**:
- Nested capsules (cache thrashing from multiple indirections)
- Unaligned composition (false sharing, 2.1× penalty)
- Container for small scale (<10K objects, overhead > benefit)
- Unbounded Vec in hot path (allocation spikes)

---

**Version**: UCE35 (October 2025)
**Framework**: Q10.5 Meta-Capsule Architecture
**Status**: Production taxonomy with 100+ validated capsules
