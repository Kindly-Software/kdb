# Circuit Breaker Implementations: Comparison Guide

**Purpose**: Clarify when to use each circuit breaker implementation in the Primitives ecosystem.

## TL;DR - Decision Matrix

| Use Case | Implementation | Why |
|----------|---------------|-----|
| **General-purpose** circuit breaking | `atomic_capsule::patterns::circuit_breaker` | ✅ Universal, adaptive policies, hardware telemetry |
| **HTTP/REST client** protection | `clapi_core::CircuitBreakerCapsule` | ✅ Application-specific, metrics integration |
| **Educational** learning | ~~`atomic_capsule_tier1::CircuitBreakerCapsule`~~ | ❌ Removed in v0.5.0 (use atomic_capsule instead) |

---

## Implementation Comparison

### 1. atomic_capsule::patterns::circuit_breaker (Universal) ✅

**Origin**: Migrated from standalone `atomic_breaker` crate (v0.1.0)
**Status**: ✅ **PRODUCTION-READY** (battle-tested in trading, UI, audio)
**LOC**: 1,117 lines
**Location**: `/home/samuel/Primitives/atomic_capsule/src/patterns/circuit_breaker/`

#### Features

**Core** (Standard64 - default):
- ✅ 9 packed fields in single AtomicU64
- ✅ Dual layouts: Standard64 (full) + Compact48 (embedded)
- ✅ Fixed-point metrics: Q8.8 or Q6.10
- ✅ 8 cause flags: THERM, NET, IO, CPU, LAT, MEM, GPU, DISK
- ✅ Exponential backoff: 6-bit index (0-63 levels)
- ✅ Fractal degradation: L0-L3 quality tiers

**Advanced** (feature-gated):
- ✅ MPMC variant (multi-writer support)
- ✅ Hardware telemetry (Linux perf-events)
- ✅ Adaptive policies (auto-calibration from history)
- ✅ FixedPointSerialize (native serialization, <30ns)
- ✅ Serde support (optional, JSON/HTTP diagnostics)

#### Performance (B32 Validated)

| Operation | Latency | Notes |
|-----------|---------|-------|
| Load (relaxed) | **<5ns** | Single AtomicU64 load |
| Load (acquire) | <8ns | Acquire semantics |
| Update (SWeMR) | **<15ns** | Single store |
| Update (MPMC) | <50ns | Bounded CAS (8 retries) |
| Memory | **8 bytes** | Standalone packed u64 |
| Serialize (FixedPointSerialize) | **<30ns** | vs ~500ns serde JSON |
| Deserialize (FixedPointSerialize) | **<30ns** | vs ~600ns serde JSON |
| Hash (FNV-1a) | **<15ns** | Deterministic |

#### When to Use

✅ **Use atomic_capsule::patterns::circuit_breaker when**:
- You need general-purpose circuit breaking
- Hardware telemetry integration (Linux perf-events)
- Adaptive policy auto-tuning from historical data
- Multiple cause tracking (thermal, network, I/O, CPU, latency, memory, GPU, disk)
- Embedded systems (Compact48 layout, 48-bit packed state)
- MPMC scenarios (multi-writer support)
- Audit trails with FixedPointSerialize (deterministic serialization)

#### Example Usage

```rust
use atomic_capsule::patterns::circuit_breaker::{CircuitBreaker, State, Policy, evaluate};

// Create breaker
let breaker = CircuitBreaker::new(State::Closed);

// Policy-driven evaluation
let pol = Policy::ui_holographic();
let mut last_change = 0;
evaluate(&breaker, mu, sigma, err_inc, timestamp, &mut last_change, &pol);

// State inspection
let guard = breaker.guard();
match guard.state() {
    State::Closed => { /* Normal operation */ },
    State::HalfOpen => { /* Recovery mode */ },
    State::Open => { /* Circuit open, reject */ },
    State::ForcedOpen => { /* Emergency stop */ },
}

// Cause analysis
if guard.cause() & cause::LAT != 0 {
    // Latency-triggered circuit opening
}

// Serialization (FixedPointSerialize)
#[cfg(feature = "capsule-serialize")]
{
    let snapshot = BreakerStateSnapshot::from_guard(&guard);
    let binary = snapshot.serialize_binary()?;
    let hash = snapshot.compute_hash();  // <15ns deterministic hash
}
```

#### Feature Flags

```toml
[dependencies]
atomic_capsule = { version = "0.3", features = [
    "circuit-breaker-standard64",  # Default: Full 64-bit layout
    # "circuit-breaker-compact48",  # Alternative: 48-bit for embedded (mutually exclusive)
    "circuit-breaker-mpmc",         # Multi-writer variant
    "circuit-breaker-pmu",          # Linux perf-event telemetry
    "circuit-breaker-auto-tune",    # Adaptive policy calibration
    "capsule-serialize",            # FixedPointSerialize (native, <30ns)
    # "circuit-breaker-serde",      # Serde support (optional, prefer FixedPointSerialize)
]}
```

---

### 2. clapi_core::CircuitBreakerCapsule (Application-Specific) ✅

**Origin**: clapi_core internal (v0.2.0+)
**Status**: ✅ **PRODUCTION-READY** (clapi-specific)
**LOC**: 614 lines
**Location**: `/home/samuel/Primitives/clapi_core/src/capsules/circuit_breaker_capsule.rs`

#### Features

**Core**:
- ✅ Simple state machine: Closed/Open/HalfOpen
- ✅ Per-window counters (failures, successes) with saturation
- ✅ Cooldown period (5s default before half-open)
- ✅ Generation counter (22-bit, TOCTOU prevention)
- ✅ Tight integration with `CircuitBreakerMetrics` (feature-gated)

**Limitations**:
- ❌ No multi-layout support (single 64-bit layout only)
- ❌ No MPMC variant
- ❌ No hardware telemetry
- ❌ No adaptive policies
- ❌ No cause tracking (THERM, NET, IO, etc.)
- ❌ No FixedPointSerialize support

#### Performance

| Operation | Latency | Notes |
|-----------|---------|-------|
| Check | <10ns | Single load + unpack |
| Record | <20ns | CAS loop (100 retries) |
| Transition | <30ns | CAS + generation increment |
| Memory | 64 bytes | 2× AtomicU64 + 48B padding |

#### When to Use

✅ **Use clapi_core::CircuitBreakerCapsule when**:
- You're already using clapi_core
- Simple HTTP client protection (request/response tracking)
- Tight integration with clapi_core metrics
- No need for cause tracking or adaptive policies
- Application-specific use case (not general-purpose)

#### Example Usage

```rust
use clapi_core::capsules::{CircuitBreakerCapsule, CircuitBreakerMetrics};

// Simple usage
let breaker = CircuitBreakerCapsule::new();

if breaker.allows_operation() {
    // ... do work ...
    breaker.record_success();
} else {
    breaker.record_failure();
}

// With metrics (feature-gated)
#[cfg(feature = "metrics")]
{
    static METRICS: CircuitBreakerMetrics = CircuitBreakerMetrics::new();
    let breaker = CircuitBreakerCapsule::with_metrics(&METRICS);

    let snapshot = METRICS.snapshot();
    println!("Failure rate: {} bp", snapshot.failure_rate_bp);
}
```

---

### 3. ~~atomic_capsule_tier1::CircuitBreakerCapsule (Educational)~~ ❌

**Origin**: atomic_capsule_tier1 demo patterns
**Status**: ❌ **REMOVED in v0.5.0**
**LOC**: 374 lines (deleted)
**Replacement**: Use `atomic_capsule::patterns::circuit_breaker::CircuitBreaker` instead

#### Migration

**Before** (tier1 - removed):
```rust
use atomic_capsule_tier1::patterns::{CircuitBreakerCapsule, QualityLevel};
let breaker = CircuitBreakerCapsule::new();
```

**After** (atomic_capsule - production-ready):
```rust
use atomic_capsule::patterns::circuit_breaker::{CircuitBreaker, State};
let breaker = CircuitBreaker::new(State::Closed);
```

---

## Feature Comparison Table

| Feature | atomic_capsule | clapi_core | tier1 (removed) |
|---------|---------------|------------|-----------------|
| **Status** | ✅ Production | ✅ Production | ❌ Removed |
| **LOC** | 1,117 | 614 | 374 |
| **Layouts** | Dual (Standard64 + Compact48) | Single | Single |
| **Packed Fields** | 9 | 5 | 4 |
| **MPMC** | ✅ | ❌ | ❌ |
| **Telemetry** | ✅ (PMU) | ❌ | ❌ |
| **Adaptive Policies** | ✅ | ❌ | ❌ |
| **Cause Tracking** | ✅ (8 flags) | ❌ | ✅ (6 bits) |
| **Fixed-Point Metrics** | ✅ (Q8.8, Q6.10) | ❌ | ❌ |
| **Exponential Backoff** | ✅ (6-bit, 0-63) | ❌ | ❌ |
| **FixedPointSerialize** | ✅ (<30ns) | ❌ | ❌ |
| **Serde** | ✅ (optional) | ❌ | ❌ |
| **Generation Counter** | Implicit | ✅ (22-bit) | ✅ (64-bit sep.) |
| **Read Latency** | **<5ns** | <10ns | <10ns |
| **Write Latency** | **<15ns** | <20ns | <15ns |
| **Memory** | **8 bytes** | 64 bytes | 64 bytes |
| **Production Use** | Trading, UI, Audio | HTTP clients | ❌ Demo only |

---

## Serialization Comparison

### atomic_capsule::patterns::circuit_breaker (FixedPointSerialize)

**Performance** (B32 validated):
- `serialize_binary()`: **<30ns** (18 bytes)
- `deserialize_binary()`: **<30ns**
- `compute_hash()`: **<15ns** (FNV-1a, deterministic)
- `serialize_decimal()`: <80ns (human-readable)

**Benefits**:
- ✅ Zero dependencies (serde optional)
- ✅ Deterministic (field order guaranteed by #[repr(C)])
- ✅ Fixed-point compatible (Q8.8, Q6.10)
- ✅ Audit trail integration (hash chains)
- ✅ 16× faster than serde JSON

**Example**:
```rust
use atomic_capsule::patterns::circuit_breaker::BreakerStateSnapshot;

#[cfg(feature = "capsule-serialize")]
{
    let snapshot = BreakerStateSnapshot::from_guard(&guard);

    // Binary (18 bytes, <30ns)
    let binary = snapshot.serialize_binary()?;

    // Hash (<15ns, deterministic)
    let hash = snapshot.compute_hash();

    // Decimal (human-readable)
    let decimal = snapshot.serialize_decimal()?;
    // "1,2,100,500,300,2,5,15032385535827435134"
}
```

### clapi_core::CircuitBreakerCapsule (No serialization)

**Status**: No built-in serialization support

**Workaround**: Manual snapshot + serde (if needed)

---

## Recommendations

### General-Purpose ✅

**Use**: `atomic_capsule::patterns::circuit_breaker::CircuitBreaker`

**Why**:
- Universal circuit breaking (not tied to specific application)
- Adaptive policies (auto-tuning from history)
- Hardware telemetry (Linux perf-events)
- Multiple cause tracking (8 flags)
- MPMC support (multi-writer scenarios)
- Embedded systems (Compact48 layout)
- Audit trails (FixedPointSerialize, <30ns)

### HTTP Client Protection ✅

**Use**: `clapi_core::CircuitBreakerCapsule`

**Why**:
- Already using clapi_core
- Tight metrics integration
- Simple failure tracking (no cause analysis needed)
- Application-specific (HTTP-centric)

### Learning & Education ❌

**Use**: `atomic_capsule::patterns::circuit_breaker::CircuitBreaker`

**Why**:
- tier1 implementation removed in v0.5.0
- Production-grade code is better for learning
- Real-world patterns (trading, UI, audio tested)

---

## Migration Paths

### From tier1 → atomic_capsule

```diff
-use atomic_capsule_tier1::patterns::{CircuitBreakerCapsule, QualityLevel};
+use atomic_capsule::patterns::circuit_breaker::{CircuitBreaker, State};

-let breaker = CircuitBreakerCapsule::new();
+let breaker = CircuitBreaker::new(State::Closed);

-let level = breaker.load_level();
+let guard = breaker.guard();
+let level = guard.level();
```

### From atomic_breaker → atomic_capsule

```diff
-use atomic_breaker::breaker::{AtomicBreakerSWeMR, AtomicBreakerGuard, State};
+use atomic_capsule::patterns::circuit_breaker::{CircuitBreaker, State};

-let breaker = AtomicBreakerSWeMR::new(State::Closed);
+let breaker = CircuitBreaker::new(State::Closed);
```

### clapi_core (No Migration Required)

clapi_core's `CircuitBreakerCapsule` is application-specific and will **NOT** be migrated.
It serves a different purpose and is tightly integrated with clapi_core metrics.

---

## Framework Compliance

### atomic_capsule::patterns::circuit_breaker

- **UCE34**: Q1-Q34 complete (tier selection, adaptive policies, Q34 auditability)
- **ASSUM**: 15+ tags, 99.99% safe (no unsafe code)
- **T28**: 50+ tests (unit/property/integration/real-world datasets)
- **B32**: Fair baselines (vs mutex/RwLock), 95% CI, 1000+ iterations
- **I20**: All 20 integration questions validated
- **Chaos**: 100% lockfree, DualAtomicU64 patterns

### clapi_core::CircuitBreakerCapsule

- **UCE34**: Q1-Q28 (no Q29-Q34 advanced features)
- **ASSUM**: 8+ tags, 99.9% safe
- **T28**: 15+ tests (unit + concurrent)
- **B32**: Fair baselines (no RwLock comparison)
- **Chaos**: 100% lockfree, generation counters

---

## Production Use Cases

### atomic_capsule::patterns::circuit_breaker

1. **Trading Systems** (MES/MNQ scalping):
   - Latency-based circuit breaking (<5ns check)
   - Position sizing degradation (L0-L3 quality tiers)
   - Adaptive backoff with market data metrics

2. **Real-Time UI** (holographic rendering):
   - Frame-drop prevention (mu/sigma thresholds)
   - Progressive quality degradation
   - Thermal throttling integration (THERM cause)

3. **Audio Pipelines** (sub-millisecond):
   - Buffer underrun prevention
   - Adaptive sample rate degradation
   - CPU load monitoring (CPU cause)

### clapi_core::CircuitBreakerCapsule

1. **HTTP Clients**:
   - Request failure tracking
   - Cooldown-based recovery
   - Per-provider circuit status

2. **API Gateway**:
   - Backend service protection
   - Failure rate monitoring
   - Graceful degradation

---

## Conclusion

**Universal**: Use `atomic_capsule::patterns::circuit_breaker::CircuitBreaker` for general-purpose circuit breaking.

**Application-Specific**: Use `clapi_core::CircuitBreakerCapsule` if already using clapi_core for HTTP clients.

**Educational**: ~~tier1 removed~~ → Use atomic_capsule (production-grade is better for learning).

**Motto**: "One circuit breaker to rule them all" - `atomic_capsule::patterns::circuit_breaker::CircuitBreaker`
