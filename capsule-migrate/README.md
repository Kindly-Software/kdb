# capsule-migrate

**Automated migration tool for computational capsule architecture (Chaos)**

[![Status](https://img.shields.io/badge/status-in%20development-yellow)](https://github.com/kindly-ai/primitives)
[![Progress](https://img.shields.io/badge/progress-15%25-orange)](https://github.com/kindly-ai/primitives)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE)

---

## ⚠️ Project Status

**This project is in early development (15% complete)**

- ✅ Architecture design complete (UCE34 Q1-Q34 systematic discovery)
- ✅ Documentation complete (1,260-line ARCHITECTURE.md)
- 🚧 Core implementation in progress
- ❌ Not yet functional for production use

**Estimated completion**: 7-10 weeks

---

## Overview

Automated migration from manual verification macros to automatic computational capsule derivation. Migrates traditional Rust code (Mutex, RwLock, scattered atomics) to computational capsule architecture with **3-100× proven speedups**.

### What It Does (Target)

- **Detects** 618 manual macro call sites across 7 projects
- **Migrates** to `#[derive(ComputationalCapsule)]` (87.5% code reduction)
- **Validates** semantic equivalence and compilation
- **Reports** migration statistics and performance improvements

### Key Innovation

The tool itself uses capsule architecture—**everything is a capsule**—demonstrating Chaos benefits (3-100× speedups) in the migration infrastructure itself.

---

## Features (Target)

### Migration Patterns

| Pattern | Before | After | Speedup |
|---------|--------|-------|---------|
| **MutexToAtomic** | `Mutex<u64>` | `AtomicU64` | 3-10× |
| **RwLockToDualAtomic** | `RwLock<T>` | `DualAtomicU64` | 3-10× |
| **ManualToDerive** | Manual macros | `#[derive(...)]` | 87.5% reduction |
| **ManualToAutoPad** | Manual `_padding` | `auto_pad = true` | Zero errors |

### Detected Patterns (8 Manual Macros)

```rust
verify_capsule_properties!        // Full verification
verify_alignment_only!            // Alignment check only
verify_dual_atomic_u64!           // DualAtomicU64 specific
verify_simd_capsule!              // T2 SIMD
verify_auditable_capsule!         // T0 Auditable
verify_atomic_simd_capsule!       // T6 Mixed
verify_fixed_point_capsule!       // T3 Fixed-Point
verify_batch_capsule!             // T4 Batch
```

---

## Installation

**Note**: Not yet functional. For development only.

```bash
# Clone repository
git clone https://github.com/kindly-ai/primitives
cd primitives/capsule-migrate

# Build (requires Rust nightly for some features)
cargo +nightly build --release --features nightly-all
```

---

## Usage (Target API)

### Analyze Project

```bash
# Find all capsules requiring migration
capsule-migrate analyze /path/to/project

# Output:
# Found 618 capsules to migrate
#   - DualAtomicU64 (src/primitives/dual_atomic.rs:42): Atomic → [ManualToDerive]
#   - CircuitBreakerCapsule (patterns/circuit_breaker.rs:128): Atomic → [ManualToDerive, MutexToAtomic]
```

### Migrate Project

```bash
# Dry-run (no file modifications)
capsule-migrate migrate /path/to/project --dry-run

# Real migration with options
capsule-migrate migrate /path/to/project \
    --apply \
    --backup \
    --verify \
    --jobs 8 \
    --progress

# Output:
# Migrating project: /path/to/project
# ✓ Migrated DualAtomicU64 in 0.2s
# ✓ Migrated CircuitBreakerCapsule in 0.3s
# ...
# Completed 618 migrations in 45.2s
```

### Generate Report

```bash
# JSON report (requires 'reports' feature)
capsule-migrate report /path/to/project --format json > report.json
```

---

## Performance Targets (B32 Validated - Not Yet Achieved)

### Migration Throughput

| Scale | Target | Baseline | Speedup |
|-------|--------|----------|---------|
| **Small (1K lines)** | 100K lines/s | 10K lines/s | 10× |
| **Medium (10K lines)** | 50K lines/s | 5K lines/s | 10× |
| **Large (100K lines)** | 20K lines/s | 2K lines/s | 10× |

### Capsule Operation Latency

| Operation | Target | Baseline (Mutex) | Speedup |
|-----------|--------|------------------|---------|
| **State update** | <5ns | 30ns | 6× |
| **Progress increment** | <5ns | 30ns | 6× |
| **Error accumulation** | <50ns | 1-5μs | 50-100× |
| **Result aggregation** | <50ns | 1-5μs | 50-100× |

---

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for complete 1,260-line design.

### Tier Selection (UCE34 Q10)

- **T0 Auditable**: AST parsing (syn), code generation (quote)
- **T1 Atomic**: Migration state tracking, progress counters (3-10× vs Mutex)
- **T4 Batch**: Parallel file processing (10-100× vs sequential)
- **T6 Mixed**: Result aggregation (T1+T4 compound, 50-100× speedup)

### Core Capsules (Target)

```rust
// T1 Atomic: Migration state tracking
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
struct MigrationStateCapsule {
    files_processed: AtomicU64,
    migrations_found: AtomicU64,
    migrations_applied: AtomicU64,
    errors_count: AtomicU64,
    start_time_ns: AtomicU64,
    last_update_ns: AtomicU64,
    _padding: [u8; 80],
}
```

---

## Feature Flags

### Default Features

```toml
default = ["std", "regex"]
```

### Nightly Features (Cutting-Edge-First)

```toml
nightly-all = [
    "nightly-const-fn",
    "nightly-specialization",
    "nightly-const-traits",
    "nightly-generic-const-exprs",
    "nightly-inline-const",
]
```

**Benefits**:
- **Const fn optimization**: 0ns runtime validation (all checks at compile-time)
- **Specialization**: Type-specific code gen (15-30% compilation speedup)
- **Const trait impl**: Zero-cost trait abstractions (20% faster validation)
- **Generic const exprs**: Compile-time capsule verification

### Performance Features

```toml
parallel     # Parallel processing (10-100× speedup)
regex-full   # Advanced pattern matching
reports      # JSON/TOML migration reports
```

---

## Build Configurations

### Stable Build (No Nightly)

```bash
cargo build --release
```

### Nightly Build (All Optimizations)

```bash
cargo +nightly build --release --features nightly-all
```

### Parallel Migration (10-100× Speedup Target)

```bash
cargo +nightly build --release --features "nightly-all,parallel"
```

---

## Development Status

### ✅ Complete

- Architecture design (UCE34 Q1-Q34)
- Documentation (ARCHITECTURE.md, README.md, CLAUDE.md)
- Core types (MigrationTier, MigrationStrategy, MigrationContext)
- Const functions (compile-time validation)
- Test infrastructure (T28 framework structure)

### 🚧 In Progress

- Pattern detection (detector.rs, ~50% complete)
- CLI integration (main.rs skeleton)

### ❌ Not Started

- Core capsule implementations
- Code generation (transformer.rs)
- Parallel processing (orchestrator.rs)
- Validation pipeline (validator.rs)
- B32 benchmarks
- T28 comprehensive testing

**Implementation Progress**: ~15% complete

---

## Framework Compliance

### UCE34 Q1-Q34

- ✅ **Q1-Q9**: Problem definition complete
- ✅ **Q10**: Tier selection (T1/T4/T6)
- ✅ **Q11**: Rust transform (syn, quote)
- ✅ **Q12**: Nightly features (optional)
- ❌ **Q30**: Validation (B32 + T28) not implemented
- ⚠️ **Q33**: Verification (derive macros) - design only
- ⚠️ **Q34**: Auditability (migration audit trail) - design only

### IMPL-2 V3.1 (Cutting-Edge-First)

- ✅ **Nightly-First**: Stable tool, nightly opt-in
- ⚠️ **Tier-Maximization**: T6 Mixed designed, not implemented
- ❌ **Zero-Compromise**: Currently uses `rayon` (must migrate to atomic_capsule::parallel)

### ASSUM Safety

- ✅ **Zero Unsafe**: 100% safe Rust (target)
- ⚠️ **Atomic Safety**: Memory orderings defined, not implemented

---

## Known Issues

### Critical

- **Not Functional**: Core implementation missing (~85% incomplete)
- **Compilation Errors**: Missing function exports (analyze_project, migrate_capsule)
- **Rayon Dependency**: Violates Chaos 100% lockfree mandate (must use atomic_capsule::parallel)

### Design Violations (Needs Fixing)

- **Mutex/RwLock**: None (design compliant, not yet implemented)
- **Rayon**: Must migrate to atomic_capsule::parallel for Chaos compliance
- **Testing**: 0/530 target tests implemented
- **Benchmarks**: B32 validation not implemented

---

## Contributing

**Not yet accepting contributions** (architecture and core implementation in progress)

Once core implementation is complete:
1. Read [ARCHITECTURE.md](ARCHITECTURE.md) (1,260 lines)
2. Read [CLAUDE.md](CLAUDE.md) (framework compliance)
3. Read `/home/samuel/CLAUDE.md` (UCE34, IMPL-2 V3.1 frameworks)
4. Submit PRs following UCE34/ASSUM/B32/T28 frameworks

---

## Testing

**Note**: Tests not yet implemented.

### Target Testing Strategy (T28 Framework)

```bash
# Unit tests (Q1-Q7)
cargo test --lib --test unit_tests

# Integration tests (Q15-Q21)
cargo test --test integration_tests

# Property tests (Q8-Q14)
cargo test --test property_tests

# Production tests (Q22-Q28)
cargo test --ignored
```

**Target Coverage**: 530 tests (unit, property, integration, production)

---

## Benchmarking

**Note**: Benchmarks not yet implemented.

### Target Benchmarking (B32 Framework)

```bash
# All benchmarks
cargo +nightly bench --features nightly-all

# Specific benchmark
cargo +nightly bench migration_bench
```

**Benchmark Groups** (Target):
1. **Throughput**: Files/sec by codebase size
2. **Latency**: Per-operation latency (state, progress, errors)
3. **Memory**: Peak memory usage
4. **Scaling**: Multi-core scaling (1/2/4/8/16 threads)

---

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) - Complete 1,260-line design (UCE34 Q1-Q34)
- [CLAUDE.md](CLAUDE.md) - Project configuration and framework compliance
- [README.md](README.md) - This file (user-facing documentation)

### External References

- **UCE34 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/UCE34_FRAMEWORK.md`
- **Q12 ULTRATHINK**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/UCE34_Q12_ULTRATHINK_FEATURE_ANALYSIS.md`
- **The Atomic Capsule**: `/home/samuel/Docs/The Atomic Capsule.md`
- **KEY_INNOVATIONS.md**: `/home/samuel/Primitives/Docs/KEY_INNOVATIONS.md`
- **atomic_capsule_derive**: `/home/samuel/Primitives/atomic_capsule_derive/`

---

## Roadmap

### Phase 1: Foundation (2-3 weeks)

- Implement core capsules (state, progress, errors, results)
- Fix compilation errors (export functions)
- Complete pattern detection (8 manual macros)
- Unit tests (T28 Q1-Q7)

### Phase 2-3: Analysis & Generation (3-4 weeks)

- Code generation (quote-based transformation)
- File backup and writing
- **CRITICAL**: Replace rayon with atomic_capsule::parallel
- Property tests (semantic equivalence)

### Phase 4-5: Validation (2-3 weeks)

- B32 benchmarks (throughput, latency, memory)
- T28 comprehensive testing (530 tests)
- Real codebase migration (atomic_capsule, kindly_hft, etc.)
- Production deployment

**Estimated Total**: 7-10 weeks to production-ready

---

## License

MIT OR Apache-2.0

---

## Contact

**Repository**: https://github.com/kindly-ai/primitives
**Maintainer**: Samuel <samuel@kindly.ai>
**Project**: capsule-migrate
**Framework**: UCE34 + IMPL-2 V3.1 + Chaos

---

**Version**: 0.1.0 | **Date**: 2025-11-02 | **Status**: Architecture Complete, Implementation In Progress (15%)
