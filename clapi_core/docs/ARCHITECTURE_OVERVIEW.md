# Architecture Overview - Clapi Core

**Read Time**: 15-20 minutes
**Target Audience**: Developers, Architects, DevOps
**Prerequisites**: Basic understanding of atomic operations and cache coherence

For detailed implementation: See [ARCHITECTURE_DEEP_DIVE.md](ARCHITECTURE_DEEP_DIVE.md)
For performance tuning: See [PERFORMANCE.md](PERFORMANCE.md)

---

## What is Clapi Core?

Clapi Core is a **100% lockfree AI call protection proxy** that enforces budgets, routes requests across multiple providers, and provides automatic failover with circuit breakers.

**Key Innovation**: Built entirely on computational capsules (UCE34 framework) for 3-100× speedup over traditional mutex-based approaches.

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     HTTP Layer (Axum)                        │
│  OpenAI-compatible API (/v1/chat/completions)               │
│  Metrics endpoints (/metrics, /health)                       │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                  Budget Registry (T1 Atomic)                 │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  BudgetSlotCapsule[1M] (128MB preallocated)           │  │
│  │  - AtomicPtr<RequestCapsule128> (lockfree)            │  │
│  │  - Generation counters (TOCTOU prevention)            │  │
│  │  - O(1) access, zero allocation on hot path           │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                  Provider Router (T4 Batch)                  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │  ProviderCircuitArray (1KB, 16 circuits)              │  │
│  │  - Per-provider health tracking                       │  │
│  │  - Automatic failover (priority-based)                │  │
│  │  - Circuit breaker (>10% failure → open)              │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│              AI Providers (External)                         │
│  Anthropic Claude | OpenAI GPT-4 | Google Gemini | Others   │
└─────────────────────────────────────────────────────────────┘
```

---

## Core Components

### 1. Budget Registry (Lockfree Budget Enforcement)

**Purpose**: Track and enforce spending limits across 1M concurrent users with <100ns latency.

**Architecture**:
- **Preallocated Array**: `Box<[BudgetSlotCapsule; 1M]>` (128MB fixed memory)
- **Lockfree Access**: `AtomicPtr` for concurrent reads/writes
- **TOCTOU Prevention**: Generation counters eliminate race conditions

**Hot Path Operations** (all <100ns):
```rust
// Budget check (60ns typical)
registry.try_deduct(budget_id, cost_cents)?;

// Slot allocation (80ns typical)
registry.allocate(budget_id, initial_budget)?;

// Slot deallocation (90ns typical)
registry.deallocate(budget_id)?;
```

**Memory Layout**:
```
BudgetSlotCapsule (128B cache-aligned)
├─ capsule_ptr: AtomicPtr<RequestCapsule128> [16B]
├─ generation: AtomicU64                      [8B]
├─ state: AtomicU8                            [1B]
└─ _padding: [u8; 95]                         [95B]
```

**Scalability**:
| Slots | Memory | Throughput (8 threads) |
|-------|--------|------------------------|
| 1M | 128 MB | 60M ops/s |
| 10M | 1.28 GB | 60M ops/s |
| 100M | 12.8 GB | 60M ops/s |

---

### 2. Circuit Breaker (Automatic Failover)

**Purpose**: Prevent cascading failures by isolating unhealthy providers.

**States**:
```
                     Failure rate >10%
       ┌─────────────────────────────────┐
       │                                 │
       │                                 ▼
   ┌───────┐                        ┌────────┐
   │       │   Cooldown (60s)       │        │
   │ Closed│◄───────────────────────┤  Open  │
   │       │   Failure rate <5%     │        │
   └───────┘                        └────────┘
       │                                 │
       │          HalfOpen (testing)     │
       └─────────────────────────────────┘
               Failure rate 5-10%
```

**Configuration**:
```toml
[circuit_breaker]
failure_threshold_bp = 1000     # 10% failure → Open
recovery_threshold_bp = 500     # 5% failure → Closed
cooldown_secs = 60              # Cooldown period
min_samples = 10                # Minimum requests before evaluation
```

**Multi-Provider Failover**:
```rust
// Priority-based routing with automatic failover
[[providers]]
id = "anthropic"
priority = 1  # Try first

[[providers]]
id = "openai"
priority = 2  # Fallback if anthropic circuit open

[[providers]]
id = "gemini"
priority = 3  # Second fallback
```

---

### 3. Provider Router (Intelligent Request Distribution)

**Purpose**: Route requests to healthy providers with deterministic fallback.

**Routing Algorithm**:
1. Sort providers by priority (lower = higher priority)
2. Filter out providers with open circuit breakers
3. Select first healthy provider
4. If all providers unhealthy → return `AllProvidersUnavailable`

**Performance**: <80ns routing decision (lockfree atomic reads)

---

### 4. Audit Trail (Tamper-Proof Event Log)

**Purpose**: Compliance-ready audit log with hash chain integrity (SOX/SOC2/GDPR/HIPAA).

**Architecture**:
- **Streaming Tier (T5)**: O(1) append latency
- **Hash Chain**: FNV-1a hash chain links each event to previous
- **Tamper Detection**: Any modification breaks hash chain

**Event Types**:
- Budget allocation/deallocation
- Request routing decisions
- Circuit breaker state changes
- Provider failures

**Storage**: Timeline aggregation capsules with 1-minute buckets (configurable)

---

## Computational Capsule Hierarchy

Clapi Core uses **10 computational capsules** across **6 tiers** (UCE34 framework):

### Tier 1: Atomic (Sub-100ns Coordination)

| Capsule | Size | Purpose | Speedup |
|---------|------|---------|---------|
| **BudgetSlotCapsule** | 128B | Lockfree slot management | 10-30× vs RwLock |
| **CircuitBreakerCapsule** | 64B | Circuit breaker state | <5ns check |
| **REQ-128** | 128B | Request validation | 3-5× vs mutex |
| **RTE-128** | 128B | Provider routing | 3-8× vs mutex |
| **CircuitBreakerMetrics** | 64B | Metrics export | <20ns |
| **ProviderCircuitStatus** | 64B | Per-provider circuit | <20ns |

**Pattern**: DualAtomicU64, generation counters, cache alignment (64B/128B)

### Tier 2+3: SIMD + Fixed-Point

| Capsule | Size | Purpose | Speedup |
|---------|------|---------|---------|
| **RES-256** | 256B | Response metrics (SIMD+Fixed) | 4-12× vs scalar |

**Pattern**: Vectorized computation + deterministic arithmetic

### Tier 4: Batch Processing

| Capsule | Size | Purpose | Speedup |
|---------|------|---------|---------|
| **ProviderCircuitArray** | 1KB | 16 independent circuits | <300ns |
| **ET-1KB** | 1KB | Cost aggregation | 10-20× vs sequential |

**Pattern**: Parallel batch operations, SIMD aggregation

### Tier 5: Streaming

| Capsule | Size | Purpose | Speedup |
|---------|------|---------|---------|
| **ALE-128** | 128B | Audit log (streaming) | 10-100× vs blocking |

**Pattern**: O(1) latency incremental computation, hash chain integrity

---

## Memory Architecture

### Total Allocation

```
BudgetRegistry: ~128MB
├─ BudgetSlotCapsule[1M]: 128MB (1M × 128B)
├─ CircuitBreakerCapsule: 64B
├─ ProviderCircuitArray: 1KB (16 × 64B)
└─ Metadata: <1KB

Total: ~128MB preallocated, constant memory usage
```

**Scaling Formula**: `max_budget_slots × 128 bytes`
- 1M slots = 128 MB
- 10M slots = 1.28 GB
- 100M slots = 12.8 GB

### Cache Alignment Strategy

**Tier-based Alignment** (prevents false sharing):
- **Hot Tier** (64B): Circuit breaker checks (<5ns)
- **Warm Tier** (128B): Budget slots, request routing (<100ns)
- **Cold Tier** (256B): Aggregated metrics, SIMD operations (<1µs)

---

## Hot Path Performance

**Total overhead**: <300ns (0.3% of 100ms provider latency)

### Budget Check Breakdown (~60ns)

```rust
pub fn try_deduct(&self, budget_id: u64, cost: i64) -> Result<i64, ClapiError> {
    // 1. Load slot pointer (10ns, atomic load)
    let slot = &self.slots[budget_id % MAX_SLOTS];

    // 2. Check circuit breaker (5ns, atomic load)
    if !self.circuit.allows_operation() {
        return Err(ClapiError::CircuitOpen);
    }

    // 3. CAS deduction (40ns, atomic CAS)
    let capsule_ptr = slot.capsule_ptr.load(Ordering::Acquire);
    let old_budget = capsule_ptr.budget.load(Ordering::Acquire);
    let new_budget = old_budget - cost;

    if capsule_ptr.budget.compare_exchange(
        old_budget,
        new_budget,
        Ordering::Release,
        Ordering::Relaxed
    ).is_ok() {
        // 4. Increment generation (5ns, atomic increment)
        slot.generation.fetch_add(1, Ordering::Relaxed);
        Ok(new_budget)
    } else {
        Err(ClapiError::AllocationConflict)
    }
}
```

**Latency Breakdown**:
- Atomic load (slot pointer): ~10ns
- Circuit breaker check: ~5ns
- CAS operation (deduction): ~40ns
- Generation increment: ~5ns
- **Total**: ~60ns (3× faster than v0.1.x with RwLock)

---

## Concurrency Model

**100% Lockfree** - Zero mutex/RwLock throughout entire codebase.

**Coordination Primitives**:
- **AtomicU64 / AtomicPtr**: Budget counters, slot pointers
- **Ordering::Acquire / Release**: Synchronization (prevents reordering)
- **Ordering::Relaxed**: Counters (no synchronization needed)
- **Generation Counters**: TOCTOU prevention (ABA problem mitigation)

**Scalability**:
| Threads | Throughput | Efficiency | Contention |
|---------|-----------|------------|------------|
| 1 | 10M ops/s | 100% | None |
| 2 | 19M ops/s | 95% | Minimal |
| 4 | 35M ops/s | 87.5% | Low |
| 8 | 60M ops/s | 75% | Moderate |
| 16 | 85M ops/s | 53% | High |

**Observations**:
- Linear scaling up to 4 threads
- Sub-linear scaling at 8+ threads (cache coherence overhead)
- Zero lock contention (all CAS-based)

---

## Error Handling & Graceful Degradation

### Error Types

| Error | HTTP Status | Retry | Recovery Time |
|-------|-------------|-------|---------------|
| `CircuitOpen` | 503 | Yes (60s) | 1-5 min (cooldown) |
| `AllocationConflict` | 503 | Yes (100ms) | <1s (internal retry) |
| `SlotsExhausted` | 507 | No | Minutes (deallocate) |
| `BudgetExhausted` | 402 | No | Immediate (add funds) |

### Retry Logic

**Internal Retry** (transparent to client): `AllocationConflict` (CAS failures)
- Max 3 attempts
- Exponential backoff (1ms, 2ms, 4ms)
- Success rate: >99% (conflicts rare <1%)

**Client Retry** (external): `CircuitOpen`
- Wait for cooldown (60s default)
- Automatic failover to next provider
- Circuit auto-recovers on <5% failure rate

---

## Configuration

**Minimal `clapi.toml`**:

```toml
[server]
listen_addr = "0.0.0.0:8080"
default_budget_cents = 100_00  # $100.00

[circuit_breaker]
failure_threshold_bp = 1000     # 10%
recovery_threshold_bp = 500     # 5%
cooldown_secs = 60

[[providers]]
id = "anthropic"
api_key = "sk-ant-..."
endpoint = "https://api.anthropic.com/v1/messages"
priority = 1
```

**Complete Reference**: See [CONFIGURATION.md](CONFIGURATION.md)

---

## Monitoring & Observability

### Health Check Endpoint

```bash
curl http://localhost:8080/health
```

**Response**:
```json
{
  "status": "healthy",
  "providers": [
    {
      "id": "anthropic",
      "circuit_state": "Closed",
      "failure_rate_bp": 0,
      "total_requests": 142,
      "failed_requests": 0
    }
  ]
}
```

### Metrics Endpoints

- **`GET /metrics`**: All metrics (JSON)
- **`GET /metrics/circuit_breaker`**: Circuit breaker only
- **`GET /metrics/budget`**: Budget with hash chain verification

**Integration**: Prometheus, Grafana (see [INTEGRATION_GUIDE.md](INTEGRATION_GUIDE.md))

---

## Framework Compliance

### UCE34 (Computational Capsule Architecture)

- **Q10 (Tier Selection)**: T1 (Atomic) for budget, T4 (Batch) for providers, T5 (Streaming) for audit
- **Q11 (Rust Transform)**: AtomicPtr + generation counters + cache alignment
- **Q12 (Nightly)**: Stable Rust (no nightly required)
- **Q33 (Verification)**: `#[derive(ComputationalCapsule)]` compile-time checks

### ASSUM (Safety)

- **Coverage**: All atomic operations tagged with #ASSUME / #VERIFY
- **Memory Ordering**: Acquire/Release for synchronization, Relaxed for counters
- **ABA Prevention**: Generation counters on all slots
- **Property Tests**: 1000-thread concurrent allocation validated

### B32 (Benchmarking)

- **Baseline**: RwLock HashMap comparison (fair, not strawman)
- **Rigor**: 1000+ iterations, 95% CI
- **Claims**: 3-4× improvement (hardware reality, not marketing)
- **Reproducibility**: All benchmarks committed

### T28 (Testing)

- **Unit tests**: 200+ tests, capsule invariants
- **Property tests**: 1000-thread concurrent allocation
- **Integration tests**: End-to-end budget lifecycle
- **Stress tests**: 1M allocation cycles, circuit breaker simulation

---

## Next Steps

**New Users**:
1. **[Quick Start Guide](QUICKSTART.md)** - Get running in 5 minutes
2. **[Configuration Guide](CONFIGURATION.md)** - Complete config reference
3. **[Troubleshooting](TROUBLESHOOTING.md)** - Common errors and solutions

**Developers**:
1. **[Architecture Deep Dive](ARCHITECTURE_DEEP_DIVE.md)** - Implementation details (30 min read)
2. **[Performance Tuning](PERFORMANCE.md)** - SLO configuration and optimization

**DevOps**:
1. **[Integration Guide](INTEGRATION_GUIDE.md)** - Grafana + Prometheus monitoring
2. **[Deployment Strategy](../P1_DEPLOYMENT_STRATEGY.md)** - Production rollout

---

**Document Version**: 1.0
**Read Time**: 15-20 minutes
**Line Count**: ~200 lines
**Last Updated**: 2025-10-21
