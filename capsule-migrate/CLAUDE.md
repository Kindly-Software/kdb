# capsule-migrate - Automated Migration Tool

**Version**: 0.1.0 (Architecture Complete, Implementation In Progress)
**Date**: 2025-11-02
**Status**: Architecture Design Complete, Core Implementation Started
**Framework**: UCE34 + IMPL-2 V3.1 + Chaos

---

## Overview

Automated migration of 618 manual macro call sites to `#[derive(ComputationalCapsule)]`. Migrates traditional Rust code (Mutex, RwLock, scattered atomics) to computational capsule architecture with 3-100× proven speedups.

**Key Innovation**: The tool itself uses capsule architecture—demonstrating Chaos benefits in the migration infrastructure itself.

---

## Mandatory Reading

**BEFORE any work**, read in order:

1. `/home/samuel/CLAUDE.md` - UCE34, IMPL-2 V3.1, all frameworks
2. `/home/samuel/Primitives/CLAUDE.md` - Primitives project structure
3. `/home/samuel/Primitives/atomic_capsule/CLAUDE.md` - 110+ capsule primitives (T0-T10)
4. `/home/samuel/Primitives/atomic_capsule_derive/CLAUDE.md` - Derive macro internals
5. `/home/samuel/Docs/The Computational Capsule.md` - Chaos philosophy
6. `/home/samuel/Primitives/capsule-migrate/ARCHITECTURE.md` - **THIS PROJECT'S DESIGN**

---

## Current Status (2025-11-02)

### ✅ Complete

- **Architecture Design**: UCE34 Q1-Q34 systematic discovery complete (1,260 lines)
- **Documentation**: ARCHITECTURE.md, README.md, CLAUDE.md complete
- **Core Types**: MigrationTier, MigrationStrategy, MigrationContext defined
- **Detector Module**: Pattern detection infrastructure (300+ lines)
- **Const Functions**: Compile-time validation (0ns runtime)
- **Test Infrastructure**: T28 framework structure in place

### 🚧 In Progress

- **Pattern Detection**: AST parsing via syn (detector.rs, 50% complete)
- **Capsule Definitions**: Core coordination capsules needed
- **CLI**: clap integration (main.rs skeleton exists)

### ❌ Not Started

- **Code Generation**: quote-based transformation
- **Parallel Processing**: atomic_capsule::parallel integration
- **Migration Application**: File writing and backup
- **Validation Pipeline**: Compile-time verification
- **B32 Benchmarks**: Performance validation
- **Production Testing**: Real codebase migration

**Implementation Progress**: ~15% complete (architecture + documentation done, core logic needed)

---

## Features (Target)

### Migration Patterns (8 Strategies)

1. **MutexToAtomic**: `Mutex<u64>` → `AtomicU64` (3-10× speedup)
2. **RwLockToDualAtomic**: `RwLock<T>` → `DualAtomicU64` (3-10× speedup)
3. **CellToAtomic**: `Cell<T>` → `AtomicU64` (thread-safe)
4. **ManualToAutoPad**: Manual `_padding` → `auto_pad = true` (zero errors)
5. **ManualToDerive**: Manual macros → `#[derive(ComputationalCapsule)]` (87.5% reduction)
6. **InferTier**: Add explicit tier annotations (Q10 compliance)

### Detection Capabilities (8 Manual Macros)

```rust
// Detected manual verification patterns
verify_capsule_properties!        // Full verification
verify_alignment_only!            // Alignment check only
verify_dual_atomic_u64!           // DualAtomicU64 specific
verify_simd_capsule!              // T2 SIMD
verify_auditable_capsule!         // T0 Auditable
verify_atomic_simd_capsule!       // T6 Mixed
verify_fixed_point_capsule!       // T3 Fixed-Point
verify_batch_capsule!             // T4 Batch
```

### Complexity Levels (Automatic Classification)

- **Simple**: Single-field atomics, <64B
- **Dual**: DualAtomicU64, two-channel coordination, 128B
- **SIMD**: Vectorized operations, portable_simd, T2
- **Auditable**: Hash/serialize support, Q34 audit trails, T0
- **Complex**: Multi-tier composites, T6 Mixed, >256B

---

## Architecture Summary

See [ARCHITECTURE.md](ARCHITECTURE.md) for complete 1,260-line design.

### Tier Selection (UCE34 Q10)

- **T0 Auditable**: AST parsing (syn), code generation (quote)
- **T1 Atomic**: Migration state tracking, progress counters (3-10× vs Mutex)
- **T4 Batch**: Parallel file processing (10-100× vs sequential)
- **T6 Mixed**: Result aggregation (T1+T4 compound, 50-100× speedup)

### Key Capsules (Target Implementation)

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

// T1 Atomic: Progress tracking (use atomic_capsule::parallel::ProgressTrackerCapsule)
pub type MigrationProgress = atomic_capsule::parallel::ProgressTrackerCapsule;

// T6 Mixed: Error accumulation (use atomic_capsule::parallel::LockfreeResultAggregatorV3)
pub type ErrorAccumulator = atomic_capsule::parallel::LockfreeResultAggregatorV3<MigrationError>;

// T6 Mixed: Result aggregation
pub type ResultAggregator = atomic_capsule::parallel::LockfreeResultAggregatorV3<MigrationResult>;
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
```

### Generate Report

```bash
# JSON report (requires 'reports' feature)
capsule-migrate report /path/to/project --format json > report.json
```

---

## Module Structure

```
src/
├── main.rs          # CLI entry (clap) - SKELETON
├── lib.rs           # Library exports - SKELETON
│
├── detector.rs      # T0: Pattern detection (syn) - IN PROGRESS
├── analysis.rs      # T0: AST analysis - STUB
├── transformer.rs   # T0: Code transformation - STUB
├── migration.rs     # T4: Migration execution - STUB
│
├── cli.rs           # CLI types - STUB
├── patterns.rs      # Pattern definitions - STUB
├── orchestrator.rs  # T4: Parallel orchestration - STUB
├── validator.rs     # Validation pipeline - STUB
├── validation.rs    # Validation logic - STUB
└── reporting.rs     # Report generation - STUB
```

**Implementation Order** (Target):

1. **Phase 1** (Week 1): Core capsules (state, progress, errors, results) - NOT STARTED
2. **Phase 2** (Week 2): Pattern detection (analyzer complete) - IN PROGRESS
3. **Phase 3** (Week 3): Code generation (transformer, applier) - NOT STARTED
4. **Phase 4** (Week 4): Parallelization (orchestration) - NOT STARTED
5. **Phase 5** (Week 5): Validation (B32 benchmarks, T28 tests) - NOT STARTED

---

## Dependencies

```toml
[dependencies]
# AST parsing and generation (T0 Foundation)
syn = { version = "2.0", features = ["full", "parsing", "extra-traits"] }
quote = "1.0"
proc-macro2 = "1.0"

# CLI interface
clap = { version = "4.5", features = ["derive", "cargo", "env"] }

# File system traversal
walkdir = "2.4"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Optional: Parallel processing (T4 Batch)
rayon = { version = "1.10", optional = true }

# Optional: Regex for pattern matching
regex = { version = "1.10", optional = true }

# Optional: Serialization for reports
serde = { version = "1.0", optional = true, features = ["derive"] }
serde_json = { version = "1.0", optional = true }
```

**Note**: Currently uses `rayon` for parallelism. **MUST migrate to `atomic_capsule::parallel`** for Chaos compliance (100% lockfree mandate).

**Zero deps for**:
- CPU detection: `std::thread::available_parallelism()` (NOT num_cpus)
- Future: Parallelism via `atomic_capsule::parallel` (NOT rayon)

---

## Key Design Principles

### 1. Everything is a Capsule (Chaos Mandate)

**NO exceptions**:

```rust
// ❌ WRONG: Traditional Mutex
struct State {
    counter: Mutex<u64>,
}

// ✅ CORRECT: Atomic Capsule
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct StateCapsule {
    counter: AtomicU64,
    _padding: [u8; 56],
}
```

### 2. 100% Lockfree (No Mutex Anywhere)

**Absolute requirement**:
- NO `Mutex<T>`
- NO `RwLock<T>`
- NO `std::sync::mpsc` (use `atomic_capsule::collections::ring_broadcast`)
- NO blocking primitives

**Use instead**:
- `AtomicU64` for counters
- `DualAtomicU64` for dual-channel coordination
- `LockfreeResultAggregatorV3` for result collection
- `atomic_capsule::parallel::*` for parallelism

### 3. Use atomic_capsule::parallel (NOT rayon)

**Current State**: Uses `rayon` (temporary, needs migration)

**Target State**:
```rust
use atomic_capsule::parallel::{
    ParallelBatchProcessor,
    LockfreeResultAggregatorV3,
    ProgressTrackerCapsule,
    ThreadLocalBatchBuffer,
    available_parallelism,  // NOT num_cpus
};
```

**Why NOT rayon**:
- Work-stealing overhead (~100ns per task)
- Global thread pool (contention)
- Not capsule-native
- Violates Chaos 100% lockfree mandate

### 4. Automatic Verification (Derive Macro)

**All capsules MUST use**:
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct MyCapsule {
    // fields...
    _padding: [u8; N],
}
```

**Zero manual verification** (derive macro handles it)

---

## Performance Targets (B32 Framework)

### Throughput (Target)

```
Scale          | Target       | Baseline     | Speedup
─────────────────────────────────────────────────────
Small (1K)     | 100K lines/s | 10K lines/s  | 10×
Medium (10K)   | 50K lines/s  | 5K lines/s   | 10×
Large (100K)   | 20K lines/s  | 2K lines/s   | 10×
```

### Latency (Target)

```
Operation           | Target | Baseline      | Speedup
─────────────────────────────────────────────────────
State update        | <5ns   | 30ns (Mutex)  | 6×
Progress increment  | <5ns   | 30ns (Mutex)  | 6×
Error accumulation  | <50ns  | 1-5μs (Mutex) | 50-100×
Result aggregation  | <50ns  | 1-5μs (Mutex) | 50-100×
```

---

## Testing Strategy (T28 Framework)

### Unit Tests (Q1-Q7)

```bash
cargo test --lib --test unit_tests
```

**Coverage** (Target):
- Capsule properties (alignment, size)
- Atomic operations (increment, load, store)
- Pattern recognition (Mutex, RwLock, scattered atomics)

### Integration Tests (Q15-Q21)

```bash
cargo test --test integration_tests
```

**Coverage** (Target):
- End-to-end migration workflows
- Semantic equivalence validation
- File I/O (backup, verify, apply)

### Property Tests (Q8-Q14)

```bash
cargo test --test property_tests
```

**Coverage** (Target):
- Concurrent state updates (proptest)
- Migration correctness (compile validation)

### Production Tests (Q22-Q28)

```bash
cargo test --ignored
```

**Coverage** (Target):
- Real codebase migration
- Performance regression detection
- Tail latency (p99, p999)

---

## Framework Compliance

### IMPL-2 V3.1 (Cutting-Edge-First)

✅ **Nightly-First**: Stable tool, nightly opt-in for generated code
⚠️ **Tier-Maximization**: T6 Mixed designed, NOT yet implemented
⚠️ **Innovation-Stacking**: Target 50-100× speedup, NOT yet validated
❌ **Zero-Compromise**: Currently uses `rayon` (must migrate to atomic_capsule::parallel)
✅ **File Preservation**: Backup strategy designed (not implemented)

### UCE34 Q1-Q34

✅ **Q1-Q9**: Problem definition complete
✅ **Q10**: Tier selection (T1/T4/T6) complete
✅ **Q11**: Rust transform (syn, quote) designed
✅ **Q12**: Nightly (stable tool, nightly opt-in)
✅ **Q28**: Simplicity (6 modules designed)
✅ **Q29**: Constraints (lockfree, zero unsafe) defined
❌ **Q30**: Validation (B32 + T28) NOT implemented
⚠️ **Q33**: Verification (derive macros) - design only
⚠️ **Q34**: Auditability (migration audit trail) - design only

### ASSUM Safety

✅ **Zero Unsafe**: 100% safe Rust (target)
⚠️ **Atomic Safety**: Memory orderings defined, NOT implemented
❌ **ABA Prevention**: Generation counters (atomic_capsule dep, not yet integrated)

### B32 Benchmarking

⚠️ **Fair Baseline**: Sequential processor defined, NOT implemented
❌ **Statistical Rigor**: 1000+ iter, 95% CI NOT yet validated
❌ **Realistic**: 1K-100K line codebases NOT yet tested

### T28 Testing

⚠️ **Unit**: Test structure in place, core logic needed
❌ **Integration**: NOT implemented
❌ **Property**: NOT implemented
❌ **Production**: NOT implemented

### I20 Integration

✅ **Q1-Q5 (Scope)**: Clear boundaries defined
✅ **Q6-Q10 (Compatibility)**: atomic_capsule v0.4+ target
⚠️ **Q11-Q15 (Safety)**: Zero unsafe designed, NOT validated
❌ **Q16-Q20 (Validation)**: B32 + T28 NOT implemented

---

## Known Constraints

### Hard Constraints

❌ **NO mutex/RwLock**: 100% lockfree mandate (currently violated by rayon)
⚠️ **NO rayon**: Must migrate to atomic_capsule::parallel
✅ **NO num_cpus**: Uses std::thread::available_parallelism
✅ **NO unsafe**: Zero unsafe blocks (target)

### Soft Constraints

✅ **Prefer streaming**: Design doesn't load entire codebase
✅ **Prefer zero-copy**: References, not clones (design)
✅ **Prefer compile-time**: Derive macros, const fn validation (design)

---

## Migration Patterns (Reference)

### Pattern 1: Mutex → Atomic (3-10×)

```rust
// Before
Mutex<u64>

// After
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct CounterCapsule {
    value: AtomicU64,
    _padding: [u8; 56],
}
```

### Pattern 2: RwLock → DualAtomic (5-10×)

```rust
// Before
RwLock<State>

// After
use atomic_capsule::patterns::DualAtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct StateCapsule {
    dual: DualAtomicU64,
    _padding: [u8; 112],
}
```

### Pattern 3: Manual Macros → Derive (87.5% Reduction)

```rust
// Before (618 call sites across 7 projects)
verify_capsule_properties!(MyCapsule, 64, 64);

// After (automatic)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
struct MyCapsule { /* ... */ }
```

---

## Code Quality Status

### ✅ Strengths

- **Architecture**: Comprehensive UCE34 Q1-Q34 systematic discovery complete
- **Documentation**: 1,260-line ARCHITECTURE.md, detailed README.md
- **Type System**: Clean enums (MigrationTier, MigrationStrategy)
- **Const Functions**: Compile-time validation (0ns runtime)
- **Framework Compliance**: Full UCE34/ASSUM/B32/T28/I20 consideration

### ⚠️ Issues

- **Incomplete Implementation**: ~85% of core logic missing
- **Compilation Errors**: Missing function exports (analyze_project, migrate_capsule)
- **Rayon Dependency**: Violates Chaos 100% lockfree mandate (must use atomic_capsule::parallel)
- **No Capsule Definitions**: Core coordination capsules not implemented
- **No Tests**: 0/530 target tests implemented
- **No Benchmarks**: B32 validation not implemented

### 📊 Code Duplication

**Analysis**: 0% duplication (minimal code exists)

**Total Lines**: 2,986 lines (mostly stubs and documentation)

**Actual Logic**: ~300 lines (detector.rs partial implementation)

**Target**: 3,000-5,000 lines for complete implementation

---

## Next Steps for Completion

### Immediate (Phase 1: Foundation)

1. **Implement Core Capsules** (state.rs, progress.rs, errors.rs, results.rs)
   - MigrationStateCapsule (T1 Atomic, 128B)
   - Use atomic_capsule::parallel::ProgressTrackerCapsule
   - Use atomic_capsule::parallel::LockfreeResultAggregatorV3
   - Unit tests (T28 Q1-Q7)

2. **Fix Compilation Errors**
   - Export `analyze_project` and `migrate_capsule` functions
   - Implement function stubs for CLI

3. **Complete Pattern Detection** (detector.rs)
   - Finish syn AST visitor implementation
   - Add all 8 macro pattern detections
   - Integration tests

### Short-Term (Phase 2-3: Analysis & Generation)

4. **Code Generation** (transformer.rs, migration.rs)
   - quote-based capsule generation
   - File backup and writing
   - Property tests (semantic equivalence)

5. **Parallel Processing** (orchestrator.rs)
   - **CRITICAL**: Replace rayon with atomic_capsule::parallel
   - Implement ParallelBatchProcessor integration
   - Parallel file processing

### Medium-Term (Phase 4-5: Validation)

6. **B32 Benchmarks**
   - Throughput benchmarks (1K-100K lines)
   - Latency benchmarks (state, progress, errors)
   - Memory usage validation

7. **T28 Testing**
   - 530 total tests (unit, property, integration, production)
   - Real codebase migration (atomic_capsule, kindly_hft, etc.)
   - Semantic equivalence validation

8. **Production Deployment**
   - Migrate 618 manual macro call sites across 7 projects
   - Performance validation (10-100× speedup targets)
   - Documentation updates

---

## Estimated Completion Time

**Current Progress**: 15% (architecture + documentation complete)

**Remaining Work**:
- **Phase 1 (Foundation)**: 2-3 weeks (core capsules, pattern detection)
- **Phase 2-3 (Generation)**: 3-4 weeks (code generation, migration logic)
- **Phase 4-5 (Validation)**: 2-3 weeks (benchmarks, testing, deployment)

**Total**: 7-10 weeks to production-ready

**Blockers**: None (atomic_capsule v0.4+ provides all infrastructure)

---

## Contact

**Repository**: https://github.com/kindly-ai/primitives
**Maintainer**: Samuel <samuel@kindly.ai>
**Project**: capsule-migrate
**Framework**: UCE34 + IMPL-2 V3.1 + Chaos

---

**Version**: 0.1.0 | **Date**: 2025-11-02 | **Status**: Architecture Complete, Implementation In Progress (15%)
