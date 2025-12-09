# KIANG - Kindly Intel Arc Native Graphics

**Atomic capsule-based graphics driver for Intel Arc GPUs**

KIANG implements a modern, lockfree graphics driver architecture for Intel Arc GPUs (Xe architecture) using atomic capsule patterns from "The Atomic Capsule" framework.

## Architecture

KIANG follows the **"One word → One read → One decision"** principle where GPU state is represented as cache-aligned atomic snapshots enabling:

- **Deterministic latency**: All critical operations are O(1) atomic reads
- **Lockfree coordination**: Zero mutex/RwLock usage in hot paths
- **Graceful degradation**: Circuit breakers prevent cascading failures
- **Audit-native design**: Hash-chained ledger for all state transitions

## Hardware Support

- Intel Arc A-Series (Alchemist)
- Intel Arc Meteor Lake-P (integrated graphics)
- Kernel driver: Intel Xe (modern) or i915 (legacy)

## Core Components

### Capsules (Atomic GPU State Snapshots)

- **GpuStateCapsule (AGS-128)**: Primary GPU state (frequency, power, temperature, utilization)
- **CommandCapsule (ACC-128)**: Command buffer submission metadata
- **MemoryCapsule (AMC-256)**: Memory allocation and GGTT state

### Circuit Breaker (Graceful Degradation)

Quality levels for automatic GPU protection:
- **L0**: Normal operation (full quality, all features)
- **L1**: Reduced quality (75% quality, thermal management)
- **L2**: Minimal quality (50% quality, emergency mode)
- **L3**: Paused (operations suspended, critical thermal)

Automatic degradation based on:
- Thermal readings (triggers at 75°C, 85°C, 95°C)
- Error rates (triggers at 20, 50, 100 errors/sec)
- Memory pressure (triggers at 85%, 95%)

### DRM/GEM Interface

Linux Direct Rendering Manager interface for Intel Xe driver:
- Zero-copy buffer management
- Memory-mapped GPU access
- Atomic coordination for device operations

### Memory Management

Lockfree memory allocation with atomic coordination:
- GGTT (Global Graphics Translation Table) management
- VM_BIND for address space coordination
- Atomic bump allocation for fast path
- Deferred reclaim without locks

### Command Submission

Lockfree MPSC queue for command buffers:
- Atomic slot reservation
- Zero-contention submission
- Batch coordinator for amortization
- Multiple command types (render, compute, copy, video)

## Usage

```rust
use kiang::KiangGpu;

// Initialize GPU
let mut gpu = KiangGpu::new()?;
gpu.open("/dev/dri/card0")?;

// Read GPU state (single atomic load)
let state = gpu.read_state();
if state.is_ready() {
    println!("GPU ready: {}MHz @ {}°C",
        state.frequency_mhz,
        state.temp_celsius);
}

// Check circuit breaker
match gpu.quality_level() {
    QualityLevel::L0 => println!("Full quality"),
    QualityLevel::L1 => println!("Reduced quality (thermal)"),
    QualityLevel::L2 => println!("Minimal quality (emergency)"),
    QualityLevel::L3 => println!("GPU paused (critical)"),
}
```

## Features

- `default`: DRM backend enabled
- `drm-backend`: Linux DRM/GEM support
- `async-runtime`: Tokio async coordination

## Dependencies

- **atomic_breaker**: Production-grade circuit breaker (existing crate)
- **drm**: Linux DRM bindings
- **nix**: System call interface
- **memmap2**: Memory-mapped file support
- **tracing**: Structured logging

## Performance Targets

Based on atomic capsule principles:

- Atomic operations: <15ns (hardware CAS latency)
- Coordination operations: <100ns
- Circuit breaker checks: <5ns
- Memory allocation: <1µs (lockfree bump)
- Zero allocation in hot paths

## Development

```bash
# Build library
cargo build

# Run tests
cargo test

# Build with release optimizations
cargo build --release

# Run with DRM backend
cargo build --features drm-backend
```

## Architecture Documentation

KIANG follows the atomic capsule architecture patterns documented in:
- `/home/samuel/Docs/The Atomic Capsule.md` - Foundational patterns
- `/home/samuel/Docs/The Complete Catalog of Discoveries.md` - Implementation catalog
- `/home/samuel/Docs/What Becomes Newly Possible.md` - Capabilities unlocked

### Key Patterns Applied

1. **DualAtomicU64**: Cache-separated dual-channel coordination (128-byte aligned)
2. **Generation Counters**: TOCTOU prevention through monotonic versioning
3. **Circuit Breakers**: Six Sigma quality monitoring for resilience
4. **48-bit Atomics**: Perfect match for x86_64 virtual address space
5. **Lockfree Queues**: Zero-contention command submission

## Trade Secret Protection

This implementation contains proprietary GPU coordination patterns:
- Advanced thermal management algorithms
- Novel lockfree command submission
- Atomic capsule-based state coordination
- Zero-copy memory management patterns

**Do not** commit to public repositories without review.

## License

MIT OR Apache-2.0

## Safety

KIANG is designed with safety as a primary concern:

- **100% lockfree mandate**: No mutex/RwLock in coordination paths
- **ASSUM framework**: All atomic operations documented with safety assumptions
- **Generation counters**: Prevents TOCTOU races in all capsules
- **Cache alignment**: Prevents false sharing (64-byte, 128-byte)
- **Circuit breakers**: Graceful degradation prevents cascading failures

## Roadmap

- [x] Core atomic capsule architecture
- [x] Circuit breaker integration
- [x] GPU state capsules
- [x] Memory management primitives
- [x] Command submission queues
- [ ] Full DRM/GEM integration
- [ ] GuC/HuC firmware coordination
- [ ] VM_BIND implementation
- [ ] Performance benchmarking
- [ ] Production validation

## Contributing

Contributions welcome! Please:

1. Follow atomic capsule patterns from documentation
2. Maintain 100% lockfree coordination
3. Add tests for all new capsules
4. Document safety assumptions (ASSUM framework)
5. Validate performance claims with benchmarks

## Acknowledgments

Built on the atomic capsule architecture and leveraging the existing `atomic_breaker` crate for production-grade circuit breaking.

---

**KIANG** - Modern, lockfree graphics drivers for Intel Arc
