# CLAUDE.md - KIANG Project Configuration

## Project Overview

**KIANG** (Kindly Intel Arc Native Graphics) - Atomic capsule-based graphics driver for Intel Arc GPUs.

## Mandatory Reading

Before working on KIANG, agents MUST read:

1. `/home/samuel/Docs/The Atomic Capsule.md` - **CRITICAL** - Foundational patterns
2. `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/UCE32_FRAMEWORK.md` - Systematic discovery
3. `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/UCE_D7_DEBUGGING_FRAMEWORK.md` - Debugging framework
4. `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/ASSUM_SAFETY.md` - Safety validation
5. `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/B32_BENCHMARK_FRAMEWORK.md` - Performance validation

## Core Architecture Principles

### Atomic Capsule Patterns

KIANG implements GPU state as atomic capsules following "One word → One read → One decision":

1. **GpuStateCapsule (AGS-128)**: 128-bit atomic GPU state
   - Frequency, power, temperature, utilization
   - Single atomic read for state decisions
   - Two-phase commit publishing

2. **CommandCapsule (ACC-128)**: Command buffer metadata
   - Buffer ID, size, priority, type
   - Lockfree submission tracking

3. **MemoryCapsule (AMC-256)**: Memory allocation state
   - VRAM total/used/available
   - Atomic allocation tracking

### Lockfree Mandate

**100% lockfree coordination** - NO mutex/RwLock usage:

- ✅ Use: `AtomicU64`, `AtomicU32`, `AtomicBool`
- ✅ Use: `compare_exchange`, generation counters
- ✅ Use: Existing `atomic_breaker` crate
- ❌ Never: `Mutex`, `RwLock`, spin locks
- ❌ Never: Blocking operations in hot paths

### Circuit Breaker Integration

KIANG uses the existing `atomic_breaker` crate at `/home/samuel/Primitives/atomic_breaker`:

- **DO NOT** reimplement circuit breaker functionality
- **DO** wrap `AtomicBreakerSWeMR` for GPU-specific needs
- **DO** map GPU metrics (thermal, errors, memory) to breaker levels
- **DO** follow graceful degradation patterns (L0→L3)

### Cache Alignment

- 64-byte alignment for single atomics (prevent false sharing)
- 128-byte alignment for DualAtomicU64 patterns
- Explicit padding to cache line boundaries

## Hardware Architecture

### Intel Xe Driver

KIANG targets the modern Intel Xe kernel driver:

- **Device paths**: `/dev/dri/card0` (primary), `/dev/dri/renderD128` (render node)
- **Kernel module**: `xe` (modern) or `i915` (legacy fallback)
- **Firmware**: GuC (scheduling), HuC (video codec)
- **Memory**: VM_BIND for GPU virtual memory, GGTT for global translation

### Supported GPUs

- Intel Arc A-Series (Alchemist) - Discrete GPUs
- Intel Meteor Lake-P (integrated Arc graphics)
- Future: Battlemage, Celestial, Druid architectures

## Performance Targets

Based on atomic capsule principles and B32 framework:

- Atomic operations: <15ns (hardware CAS latency)
- Coordination operations: <100ns
- Circuit breaker checks: <5ns
- Memory allocation: <1µs (lockfree bump allocator)
- Command submission: <500ns (atomic queue reservation)

## Development Guidelines

### Adding New Capsules

1. Read "The Atomic Capsule" documentation first
2. Define decision the capsule answers ("Is GPU ready?")
3. Size appropriately (64-512 bits typical)
4. Implement two-phase commit (odd→even version)
5. Add reader acceptance rules
6. Write comprehensive tests

### Safety Requirements

Every atomic operation must follow ASSUM framework:

```rust
// #ASSUME: Single writer updates this capsule
// #VERIFY: Readers check commit bit and version match
pub fn publish(&self, state: GpuState) {
    // Two-phase commit implementation
}
```

### Testing Requirements

1. **Unit tests**: Bit packing, range limits, fixed-point conversions
2. **Property tests**: Version consistency, generation counter invariants
3. **Stress tests**: Concurrent readers, writer flood, contention
4. **Benchmarks**: Per-operation latency targets

## Module Organization

```
kiang/
├── src/
│   ├── lib.rs              # Public API
│   ├── capsules.rs         # Atomic state capsules
│   ├── circuit_breaker.rs  # GPU circuit breaker wrapper
│   ├── drm_interface.rs    # DRM/GEM bindings
│   ├── memory.rs           # Lockfree memory allocator
│   ├── command.rs          # Command submission queues
│   ├── firmware.rs         # GuC/HuC coordination
│   └── metrics.rs          # Performance metrics
├── examples/               # Example applications
├── benches/                # Performance benchmarks
└── tests/                  # Integration tests
```

## Dependencies

### Core Dependencies

- `atomic_breaker` - **Local path dependency** at `../atomic_breaker`
- `drm` - Linux DRM bindings
- `nix` - System call interface
- `memmap2` - Memory-mapped files
- `tracing` - Structured logging

### Optional Dependencies

- `tokio` - Async runtime (feature: `async-runtime`)

## Common Patterns

### GPU State Reading

```rust
// Single atomic load for decision
let state = gpu.read_state();
if state.is_ready() {
    // Proceed with command submission
}
```

### Circuit Breaker Auto-Adjustment

```rust
// Automatic degradation based on metrics
breaker.auto_adjust(
    thermal_mc,        // Temperature in millicelsius
    errors_per_sec,    // Error rate
    memory_used_pct,   // Memory pressure
    utilization        // GPU utilization
);
```

### Lockfree Memory Allocation

```rust
// Atomic reservation without locks
let alloc = allocator.allocate(size, MemoryDomain::Vram)?;
// On failure, returns None (OOM) without blocking
```

## Debugging

When debugging issues:

1. **Read UCE-D7** framework first (max 3 files, 50 lines, 0 dependencies)
2. Check atomic operation ordering (Acquire/Release/Relaxed)
3. Verify generation counters prevent TOCTOU races
4. Validate cache alignment (use `#[repr(C, align(64))]`)
5. Test under concurrent load (stress tests)

## Trade Secret Protection

KIANG contains proprietary innovations:

- Novel GPU coordination patterns
- Advanced thermal management algorithms
- Lockfree command submission techniques
- Zero-copy memory management

**Mark commits with [TRADE SECRET]** tag if they contain sensitive algorithms.

## Version Strategy

- **0.1.x**: Core architecture, basic DRM integration
- **0.2.x**: Full GuC/HuC firmware coordination
- **0.3.x**: Production-ready memory management
- **1.0.0**: Validated for production use

## Contact

- Maintainer: Samuel <samuel@kindly.software>
- Repository: https://github.com/kindly-ai/kiang
- Documentation: See README.md and inline docs

---

**Remember**: This is an atomic capsule-based project. Every design decision should follow the principles in "The Atomic Capsule" documentation.
