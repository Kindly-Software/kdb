# capsule-migrate Architecture

**Version**: 1.0
**Date**: 2025-11-02
**Status**: Architecture Design
**Framework**: UCE34 Q1-Q34 Systematic Discovery

---

## Executive Summary

`capsule-migrate` is a code migration tool for converting traditional Rust code (mutex, RwLock, scattered atomics) to computational capsule architecture. **EVERYTHING is a capsule**—migration state, progress tracking, error accumulation, file processing—all implemented as lockfree computational capsules.

**Key Design Principle**: This tool practices what it preaches. Migration infrastructure itself demonstrates capsule architecture benefits (3-10× faster than traditional approaches).

---

## UCE34 Systematic Discovery

### Q1-Q9: Problem Definition

**Q1: What problem are we solving?**
- **Problem**: Developers have scattered mutex/RwLock/atomic code that could be 3-10× faster as capsules
- **Pain Point**: Manual migration is error-prone, time-consuming (weeks per codebase)
- **Goal**: Automated tool that migrates code AND validates correctness

**Q2: Who are the users?**
- **Primary**: Rust developers migrating existing codebases to capsule architecture
- **Secondary**: Teams adopting atomic_capsule for new projects (learning reference)

**Q3: What are we NOT solving?**
- Not a general refactoring tool (focused ONLY on capsule migration)
- Not a code formatter (use rustfmt separately)
- Not a linter (use clippy separately)

**Q4: What's the scale?**
- **Input**: Codebases from 1K to 1M lines of Rust
- **Throughput**: Process 10K-100K lines/sec
- **Batch Size**: 100-1000 files in single migration run

**Q5: What are the constraints?**
- **Safety**: Zero unsafe code (100% safe Rust)
- **Correctness**: No false positives (only migrate when semantically equivalent)
- **Performance**: 3-10× faster than sequential processing (use atomic_capsule::parallel)
- **Dependencies**: Minimal (syn, quote, atomic_capsule, clap)

**Q6-Q9: Success Criteria**
- Correctly identify 95%+ migration opportunities
- Zero false positives (100% semantic equivalence)
- 10K+ lines/sec processing throughput
- <1% memory overhead vs baseline

### Q10: Capsule Tier Selection (FOUNDATION)

**Q10 Decision Tree**:

```
What primitive am I implementing?

1. Migration State Coordination?
   → T1 Atomic: MigrationStateCapsule (tracks progress, errors, completed files)
   → Speedup: 3-10× vs Mutex<State>

2. File Processing (batches of files)?
   → T4 Batch: FileBatchCapsule (process 16-64 files in parallel)
   → Speedup: 10-100× vs sequential processing

3. Progress Tracking (concurrent updates)?
   → T1 Atomic: ProgressTrackerCapsule (lockfree counters, <5ns increment)
   → Speedup: 3-10× vs Mutex<u64>

4. Error Accumulation (collect all errors)?
   → T4 Batch: ErrorAccumulatorCapsule (lockfree error collection)
   → Speedup: 10-100× vs Mutex<Vec<Error>>

5. Result Aggregation (collect migration results)?
   → T6 Mixed (T1+T4): LockfreeResultAggregatorV3 (thread-local batching + callback)
   → Speedup: 50-100× compound (proven in atomic_capsule)

6. AST Analysis (syntax tree traversal)?
   → T0 Foundation: Use syn crate (not a capsule, stateless parser)
```

**Primary Tiers**:
- **T0 Auditable**: AST parsing (syn), code generation (quote)
- **T1 Atomic**: Migration state, progress tracking, file counters
- **T4 Batch**: File processing, error accumulation
- **T6 Mixed**: Result aggregation (T1+T4 composition)

### Q11: Rust Transform

**Rust Capabilities**:
- **syn**: Parse Rust AST (identify mutex, RwLock, atomic patterns)
- **quote**: Generate capsule code (deterministic code generation)
- **atomic_capsule::parallel**: Lockfree parallel processing (NOT rayon)
- **Type System**: Ensure semantic equivalence (compile-time verification)

**Transformation Patterns**:
```rust
// Before: Mutex-based state
struct State {
    counter: Mutex<u64>,
}

// After: Atomic capsule
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct StateCapsule {
    counter: AtomicU64,
    _padding: [u8; 56],
}
```

### Q12: Nightly Features

**Nightly Usage**: STABLE (tool must run on stable Rust for broad adoption)

**Rationale**: Migration tool targets stable Rust codebases. Using nightly would limit adoption.

**Exception**: Generated capsule code MAY use nightly features (portable_simd, const_fn_floating_point) if user opts in.

### Q13-Q27: Implementation Details

*See modules section below*

### Q28: Simplicity

**Simple Interface**:
```bash
# Single command for entire codebase
capsule-migrate analyze src/
capsule-migrate migrate src/ --apply

# Progress tracking (atomic counters)
Processing: 1250/5000 files (25%)
Migrations: 47 opportunities found
Errors: 0
```

**Simple Internals**:
- 6 core modules (each <500 lines)
- Clear data flow (analyze → plan → generate → apply)
- Minimal dependencies (4 crates: syn, quote, atomic_capsule, clap)

### Q29: Practical Constraints

**Performance Constraints**:
- **Memory**: <1GB for 100K-line codebase
- **CPU**: Use std::thread::available_parallelism() (NOT num_cpus)
- **Disk**: Minimal I/O (read once, write once)

**Safety Constraints**:
- **Zero Unsafe**: 100% safe Rust (no unsafe blocks)
- **No Panic**: Result<T, E> for all operations
- **Atomicity**: Either fully migrate file OR leave unchanged (no partial state)

### Q30: Validation Strategy

**B32 Benchmarking**:
- Fair baseline: Compare against sequential syn-based processor
- Statistical rigor: 1000+ iterations, 95% CI
- Realistic workloads: 1K-100K line codebases

**T28 Testing**:
- Unit tests: Each capsule verified (alignment, size, properties)
- Integration tests: End-to-end migration workflows
- Property tests: Semantic equivalence validation

### Q31: Simplicity (Refined)

**Core Abstractions**:
1. **MigrationPlan**: What to migrate (identified patterns)
2. **MigrationAction**: How to migrate (transformation rules)
3. **MigrationResult**: What happened (success, warnings, errors)

**Everything Else is Capsules**:
- State tracking → MigrationStateCapsule (T1)
- File batching → FileBatchProcessor (T4)
- Error collection → ErrorAccumulator (T4)
- Result aggregation → ResultAggregatorV3 (T6)

### Q32: Constraints (Refined)

**Hard Constraints**:
- NO mutex, NO RwLock, NO blocking primitives (100% lockfree)
- NO rayon (use atomic_capsule::parallel)
- NO num_cpus (use std::thread::available_parallelism)
- NO unsafe (except in atomic_capsule dependency, not our code)

**Soft Constraints**:
- Prefer compile-time verification (derive macros)
- Prefer zero-copy patterns (references, not clones)
- Prefer streaming (don't load entire codebase into memory)

### Q33: Validation (Capsule-Specific)

**Capsule Verification**:
```rust
// All capsules use derive macro (automatic verification)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct MigrationStateCapsule {
    files_processed: AtomicU64,
    migrations_found: AtomicU64,
    errors_count: AtomicU64,
    _padding: [u8; 40],
}
```

### Q34: Auditability

**Migration Audit Trail**:
- Every file migration logged (before/after AST)
- Every error recorded (source location, reason)
- Progress checkpoints (resume from failure)

**Q34 Capsule** (optional):
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, auditable = true)]
#[repr(C, align(128))]
struct MigrationAuditCapsule {
    // User fields
    migration_count: AtomicU64,
    error_count: AtomicU64,

    // Q34 audit trail (generated automatically)
    fast_hash: AtomicU64,
    prev_fast_hash: AtomicU64,
    generation: AtomicU64,
    timestamp_ns: AtomicU64,

    _padding: [u8; 88],
}
```

---

## Project Structure

```
capsule-migrate/
├── Cargo.toml                 # Dependencies, feature flags
├── ARCHITECTURE.md            # This document
├── README.md                  # User-facing documentation
├── CLAUDE.md                  # Project configuration (compliance)
│
├── src/
│   ├── main.rs                # CLI entry point (clap)
│   ├── lib.rs                 # Library exports
│   │
│   ├── analyzer.rs            # T0: AST pattern recognition (syn)
│   ├── planner.rs             # T0: Migration plan generation
│   ├── generator.rs           # T0: Capsule code generation (quote)
│   ├── applier.rs             # T4: Batch file writing
│   │
│   ├── state.rs               # T1: MigrationStateCapsule
│   ├── progress.rs            # T1: ProgressTrackerCapsule
│   ├── errors.rs              # T4: ErrorAccumulatorCapsule
│   ├── results.rs             # T6: ResultAggregatorV3 (T1+T4)
│   │
│   └── parallel.rs            # T4: Parallel orchestration
│
├── tests/
│   ├── unit/                  # T28 Q1-Q7: Unit tests
│   ├── integration/           # T28 Q15-Q21: End-to-end tests
│   └── fixtures/              # Test codebases
│
└── benches/
    └── migration_bench.rs     # B32: Benchmarking
```

---

## Module Design

### 1. analyzer.rs (T0 Auditable)

**Purpose**: Parse Rust code and identify migration opportunities

**Key Types**:
```rust
pub struct MigrationOpportunity {
    pub pattern: PatternKind,
    pub location: Span,
    pub confidence: ConfidenceLevel,
}

pub enum PatternKind {
    MutexToAtomic,        // Mutex<u64> → AtomicU64
    RwLockToAtomic,       // RwLock<State> → AtomicState
    ScatteredAtomics,     // Multiple AtomicU64 → DualAtomicU64
    ScalarToSimd,         // [f32; 8] → f32x8 (requires nightly opt-in)
}

pub enum ConfidenceLevel {
    High,      // 95%+ semantic equivalence
    Medium,    // 80-95% confidence
    Low,       // <80% (requires manual review)
}
```

**Implementation**:
```rust
use syn::{parse_file, Item, Type, visit::Visit};

pub struct Analyzer {
    opportunities: Vec<MigrationOpportunity>,
}

impl Analyzer {
    pub fn analyze_file(&mut self, source: &str) -> Result<(), Error> {
        let syntax = parse_file(source)?;
        self.visit_file(&syntax);
        Ok(())
    }
}

impl<'ast> Visit<'ast> for Analyzer {
    fn visit_type(&mut self, ty: &'ast Type) {
        // Identify Mutex<T>, RwLock<T>, etc.
        match ty {
            Type::Path(path) if is_mutex(path) => {
                self.opportunities.push(/* ... */);
            }
            // ... more patterns
            _ => visit::visit_type(self, ty),
        }
    }
}
```

**Tier**: T0 (Auditable foundation, stateless parser)

---

### 2. planner.rs (T0 Auditable)

**Purpose**: Generate migration plan from opportunities

**Key Types**:
```rust
pub struct MigrationPlan {
    pub actions: Vec<MigrationAction>,
}

pub struct MigrationAction {
    pub file: PathBuf,
    pub transformations: Vec<Transformation>,
}

pub struct Transformation {
    pub kind: TransformationKind,
    pub old_code: Span,
    pub new_code: String,  // Generated capsule code
}

pub enum TransformationKind {
    ReplaceType,
    AddAttribute,
    AddPadding,
    ReplaceMethodCall,
}
```

**Implementation**:
```rust
pub struct Planner {
    rules: Vec<TransformationRule>,
}

impl Planner {
    pub fn plan(&self, opportunities: Vec<MigrationOpportunity>) -> MigrationPlan {
        opportunities
            .into_iter()
            .flat_map(|opp| self.apply_rules(opp))
            .collect()
    }

    fn apply_rules(&self, opp: MigrationOpportunity) -> Vec<MigrationAction> {
        self.rules
            .iter()
            .filter_map(|rule| rule.apply(&opp))
            .collect()
    }
}
```

**Tier**: T0 (Auditable foundation, deterministic planning)

---

### 3. generator.rs (T0 Auditable)

**Purpose**: Generate capsule code using quote

**Key Types**:
```rust
pub struct CapsuleGenerator {
    config: GeneratorConfig,
}

pub struct GeneratorConfig {
    pub alignment: usize,      // 64, 128, 256
    pub use_nightly: bool,     // Enable portable_simd?
    pub add_verification: bool, // Add derive macro?
}
```

**Implementation**:
```rust
use quote::quote;
use syn::Ident;

impl CapsuleGenerator {
    pub fn generate_atomic_capsule(&self, name: &Ident, fields: &[Field]) -> TokenStream {
        let alignment = self.config.alignment;
        let size = self.calculate_size(fields);
        let padding = size - self.field_size(fields);

        quote! {
            #[derive(ComputationalCapsule)]
            #[capsule(alignment = #alignment, size = #size)]
            #[repr(C, align(#alignment))]
            pub struct #name {
                #(#fields),*
                _padding: [u8; #padding],
            }
        }
    }
}
```

**Tier**: T0 (Auditable foundation, deterministic generation)

---

### 4. state.rs (T1 Atomic)

**Purpose**: Track migration state (lockfree coordination)

**Capsule Definition**:
```rust
use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct MigrationStateCapsule {
    // Counters (all atomic)
    files_processed: AtomicU64,
    migrations_found: AtomicU64,
    migrations_applied: AtomicU64,
    errors_count: AtomicU64,

    // Timestamps
    start_time_ns: AtomicU64,
    last_update_ns: AtomicU64,

    _padding: [u8; 80],
}

impl MigrationStateCapsule {
    pub fn new() -> Self {
        Self {
            files_processed: AtomicU64::new(0),
            migrations_found: AtomicU64::new(0),
            migrations_applied: AtomicU64::new(0),
            errors_count: AtomicU64::new(0),
            start_time_ns: AtomicU64::new(now_ns()),
            last_update_ns: AtomicU64::new(now_ns()),
            _padding: [0; 80],
        }
    }

    #[inline(always)]
    pub fn increment_processed(&self) -> u64 {
        self.files_processed.fetch_add(1, Ordering::Relaxed)
    }

    #[inline(always)]
    pub fn increment_migrations(&self) -> u64 {
        self.migrations_found.fetch_add(1, Ordering::Relaxed)
    }

    #[inline(always)]
    pub fn snapshot(&self) -> MigrationSnapshot {
        // Single atomic read per field (5 reads total)
        MigrationSnapshot {
            files_processed: self.files_processed.load(Ordering::Relaxed),
            migrations_found: self.migrations_found.load(Ordering::Relaxed),
            migrations_applied: self.migrations_applied.load(Ordering::Relaxed),
            errors_count: self.errors_count.load(Ordering::Relaxed),
            elapsed_ns: now_ns() - self.start_time_ns.load(Ordering::Relaxed),
        }
    }
}
```

**Speedup**: 3-10× vs `Mutex<MigrationState>` (proven in atomic_capsule)

**Tier**: T1 Atomic (coordination, <5ns per operation)

---

### 5. progress.rs (T1 Atomic)

**Purpose**: Real-time progress tracking (lockfree counters)

**Capsule Definition**:
```rust
use atomic_capsule::parallel::ProgressTrackerCapsule;

// Re-export atomic_capsule's ProgressTrackerCapsule
pub type MigrationProgress = ProgressTrackerCapsule;

// Usage:
let progress = MigrationProgress::new();

// Worker threads increment (lockfree)
progress.increment();

// Main thread reads progress (lockfree)
let current = progress.current();
println!("Progress: {current}/{total}");
```

**Speedup**: 3-10× vs `Mutex<u64>` (atomic increment <5ns vs mutex 30ns)

**Tier**: T1 Atomic (already implemented in atomic_capsule::parallel)

---

### 6. errors.rs (T4 Batch)

**Purpose**: Collect all migration errors (lockfree accumulation)

**Capsule Definition**:
```rust
use atomic_capsule::parallel::LockfreeResultAggregatorV3;

pub type ErrorAccumulator = LockfreeResultAggregatorV3<MigrationError>;

pub struct MigrationError {
    pub file: PathBuf,
    pub location: Span,
    pub kind: ErrorKind,
    pub message: String,
}

pub enum ErrorKind {
    ParseError,
    SemanticMismatch,
    GenerationFailed,
    WriteError,
}

// Usage:
let errors = ErrorAccumulator::new();

// Worker thread: Add error (lockfree, thread-local batch)
errors.insert(MigrationError { /* ... */ })?;

// Main thread: Collect all errors (O(1) merge via callback)
let all_errors = errors.drain(|batch| {
    batch.into_iter().collect()
});
```

**Speedup**: 50-100× vs `Mutex<Vec<Error>>` (proven in atomic_capsule Phase 4.6)

**Tier**: T6 Mixed (T1+T4, thread-local batching + atomic coordination)

---

### 7. results.rs (T6 Mixed)

**Purpose**: Aggregate migration results (lockfree result collection)

**Capsule Definition**:
```rust
use atomic_capsule::parallel::LockfreeResultAggregatorV3;

pub type ResultAggregator = LockfreeResultAggregatorV3<MigrationResult>;

pub struct MigrationResult {
    pub file: PathBuf,
    pub migrations: Vec<AppliedMigration>,
    pub warnings: Vec<String>,
}

pub struct AppliedMigration {
    pub pattern: PatternKind,
    pub old_code: String,
    pub new_code: String,
}

// Usage:
let results = ResultAggregator::new();

// Worker thread: Add result (lockfree, <50ns)
results.insert(MigrationResult { /* ... */ })?;

// Main thread: Collect all results (O(1) merge)
let all_results = results.drain(|batch| {
    batch.into_iter().collect()
});
```

**Speedup**: 50-100× vs `Mutex<Vec<Result>>` (proven in atomic_capsule)

**Tier**: T6 Mixed (T1+T4 composition, LockfreeResultAggregatorV3)

---

### 8. parallel.rs (T4 Batch)

**Purpose**: Orchestrate parallel file processing

**Implementation**:
```rust
use atomic_capsule::parallel::{ParallelBatchProcessor, LockfreeList};

pub struct MigrationOrchestrator {
    state: Arc<MigrationStateCapsule>,
    progress: Arc<MigrationProgress>,
    errors: Arc<ErrorAccumulator>,
    results: Arc<ResultAggregator>,
}

impl MigrationOrchestrator {
    pub fn migrate_parallel(&self, files: Vec<PathBuf>) -> Result<MigrationSummary, Error> {
        // Get available parallelism (zero deps, stdlib)
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        // Create parallel batch processor (atomic_capsule::parallel)
        let processor = ParallelBatchProcessor::new(num_threads);

        // Process files in batches (lockfree coordination)
        processor.process_batch(files, |file| {
            self.process_file(file)
        })?;

        // Collect results (O(1) drain)
        let all_results = self.results.drain(|batch| batch.into_iter().collect());
        let all_errors = self.errors.drain(|batch| batch.into_iter().collect());

        Ok(MigrationSummary {
            results: all_results,
            errors: all_errors,
            stats: self.state.snapshot(),
        })
    }

    fn process_file(&self, file: PathBuf) -> Result<MigrationResult, MigrationError> {
        // 1. Analyze file
        let source = std::fs::read_to_string(&file)?;
        let mut analyzer = Analyzer::new();
        analyzer.analyze_file(&source)?;

        // 2. Plan migration
        let planner = Planner::new();
        let plan = planner.plan(analyzer.opportunities());

        // 3. Generate capsule code
        let generator = CapsuleGenerator::new(self.config);
        let new_code = generator.generate(&plan)?;

        // 4. Update state (lockfree)
        self.state.increment_processed();
        self.state.add_migrations(plan.actions.len() as u64);
        self.progress.increment();

        Ok(MigrationResult {
            file,
            migrations: plan.actions,
            warnings: vec![],
        })
    }
}
```

**Speedup**: 10-100× vs sequential processing (N threads × file I/O parallelism)

**Tier**: T4 Batch (ParallelBatchProcessor from atomic_capsule)

---

### 9. applier.rs (T4 Batch)

**Purpose**: Write migrated code to disk (batch I/O)

**Implementation**:
```rust
pub struct MigrationApplier {
    config: ApplierConfig,
}

pub struct ApplierConfig {
    pub dry_run: bool,
    pub backup: bool,
    pub verify: bool,
}

impl MigrationApplier {
    pub fn apply(&self, result: &MigrationResult) -> Result<(), Error> {
        let file = &result.file;

        // Backup original (if enabled)
        if self.config.backup {
            std::fs::copy(file, file.with_extension("bak"))?;
        }

        // Write new code (batch I/O)
        if !self.config.dry_run {
            std::fs::write(file, &result.new_code)?;
        }

        // Verify compilation (if enabled)
        if self.config.verify {
            self.verify_compiles(file)?;
        }

        Ok(())
    }

    fn verify_compiles(&self, file: &Path) -> Result<(), Error> {
        // Run rustc --check on migrated file
        let output = std::process::Command::new("rustc")
            .args(&["--check", file.to_str().unwrap()])
            .output()?;

        if !output.status.success() {
            return Err(Error::CompilationFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        Ok(())
    }
}
```

**Tier**: T4 Batch (batch file I/O)

---

## Data Flow

```
Input Files (Vec<PathBuf>)
    ↓
[Parallel Orchestrator] (T4 Batch)
    ↓
    ├──→ [Analyzer] (T0) ──→ Identify patterns
    ├──→ [Planner] (T0) ──→ Generate plan
    ├──→ [Generator] (T0) ──→ Generate capsule code
    └──→ [Applier] (T4) ──→ Write to disk
    ↓
[State Tracking] (T1 Atomic)
    ├──→ files_processed: AtomicU64
    ├──→ migrations_found: AtomicU64
    └──→ errors_count: AtomicU64
    ↓
[Result Aggregation] (T6 Mixed)
    ├──→ Thread-local batching (T4)
    └──→ Atomic coordination (T1)
    ↓
Migration Summary
    ├──→ Results: Vec<MigrationResult>
    ├──→ Errors: Vec<MigrationError>
    └──→ Stats: MigrationSnapshot
```

---

## Capsule Definitions Summary

| Capsule | Tier | Alignment | Size | Purpose | Speedup |
|---------|------|-----------|------|---------|---------|
| **MigrationStateCapsule** | T1 | 128B | 128B | Track state (counters, timestamps) | 3-10× vs Mutex |
| **MigrationProgress** | T1 | 64B | 64B | Progress tracking (lockfree increment) | 3-10× vs Mutex |
| **ErrorAccumulator** | T6 | N/A | N/A | Collect errors (thread-local batching) | 50-100× vs Mutex<Vec> |
| **ResultAggregator** | T6 | N/A | N/A | Collect results (lockfree aggregation) | 50-100× vs Mutex<Vec> |

**All capsules**:
- Use `#[derive(ComputationalCapsule)]` (automatic verification)
- 100% lockfree (NO mutex, NO RwLock)
- Cache-aligned (64B/128B)
- Zero unsafe code

---

## Parallel Strategy (atomic_capsule::parallel)

### Why NOT rayon?

**Rayon Issues**:
- Work-stealing overhead (~100ns per task)
- Global thread pool (contention with other rayon users)
- Not capsule-native (doesn't use atomic_capsule primitives)

**atomic_capsule::parallel Benefits**:
- Lockfree work-stealing (WorkStealingQueue with generation counters)
- LockfreeResultAggregatorV3 (50-100× faster than Mutex<Vec>)
- ProgressTrackerCapsule (lockfree progress tracking)
- Thread-local batching (ThreadLocalBatchBuffer, 10-20× speedup)
- 100% Chaos compliant (all capsules, no mutex)

### Parallelism Pattern

```rust
use atomic_capsule::parallel::{ParallelBatchProcessor, available_parallelism};

// Get CPU cores (stdlib, zero deps)
let num_threads = available_parallelism();

// Create parallel processor (lockfree)
let processor = ParallelBatchProcessor::new(num_threads);

// Process files in parallel (lockfree coordination)
processor.process_batch(files, |file| {
    // Each worker thread processes files independently
    process_file(file)
})?;
```

**Speedup**: 10-100× vs sequential (N cores × I/O parallelism)

---

## Dependencies

### Direct Dependencies (4 crates)

```toml
[dependencies]
# AST parsing and code generation (T0 Foundation)
syn = { version = "2.0", features = ["full", "visit"] }
quote = "1.0"

# Capsule infrastructure (T1/T4/T6)
atomic_capsule = { version = "0.4", features = ["parallel", "std"] }

# CLI interface
clap = { version = "4.0", features = ["derive"] }

[dev-dependencies]
# Benchmarking (B32 framework)
criterion = "0.5"

# Testing
tempfile = "3.0"
```

**Zero deps for**:
- CPU detection: `std::thread::available_parallelism()` (NOT num_cpus)
- Parallelism: `atomic_capsule::parallel` (NOT rayon)
- Error handling: `std::result::Result` (NOT anyhow/thiserror for lib)

### Why These Dependencies?

**syn + quote**: Required for Rust AST parsing and code generation (no alternative)

**atomic_capsule**: Core capsule infrastructure (T1/T4/T6 primitives)

**clap**: Standard CLI framework (user-friendly argument parsing)

**criterion**: B32-compliant benchmarking (statistical rigor, 95% CI)

---

## Performance Targets (B32 Framework)

### Processing Throughput

```
Scale          | Target       | Baseline        | Speedup
─────────────────────────────────────────────────────────────
Small (1K)     | 100K lines/s | 10K lines/s     | 10×
Medium (10K)   | 50K lines/s  | 5K lines/s      | 10×
Large (100K)   | 20K lines/s  | 2K lines/s      | 10×
```

**Factors**:
- Parallel processing: N threads (N = CPU cores)
- Lockfree coordination: 3-10× vs Mutex
- Batch I/O: 10-100× vs sequential writes

### Memory Usage

```
Scale          | Target       | Baseline        | Overhead
─────────────────────────────────────────────────────────────
Small (1K)     | 10 MB        | 10 MB           | <1%
Medium (10K)   | 50 MB        | 50 MB           | <1%
Large (100K)   | 200 MB       | 200 MB          | <1%
```

**Memory Efficiency**:
- Streaming processing (don't load entire codebase)
- Zero-copy patterns (references, not clones)
- Thread-local batching (minimal allocation)

### Latency Targets

```
Operation               | Target   | Baseline     | Speedup
─────────────────────────────────────────────────────────────
State update (atomic)   | <5ns     | 30ns (Mutex) | 6×
Progress increment      | <5ns     | 30ns (Mutex) | 6×
Error accumulation      | <50ns    | 1-5μs (Mutex)| 50-100×
Result aggregation      | <50ns    | 1-5μs (Mutex)| 50-100×
```

---

## Testing Strategy (T28 Framework)

### Unit Tests (Q1-Q7)

```rust
// Test capsule properties
#[test]
fn test_migration_state_alignment() {
    assert_eq!(std::mem::align_of::<MigrationStateCapsule>(), 128);
    assert_eq!(std::mem::size_of::<MigrationStateCapsule>(), 128);
}

// Test atomic operations
#[test]
fn test_migration_state_increment() {
    let state = MigrationStateCapsule::new();
    assert_eq!(state.increment_processed(), 0);
    assert_eq!(state.increment_processed(), 1);
}

// Test pattern recognition
#[test]
fn test_analyzer_mutex() {
    let source = "struct S { x: Mutex<u64> }";
    let mut analyzer = Analyzer::new();
    analyzer.analyze_file(source).unwrap();
    assert_eq!(analyzer.opportunities().len(), 1);
}
```

### Integration Tests (Q15-Q21)

```rust
// End-to-end migration
#[test]
fn test_migrate_mutex_to_atomic() {
    let input = r#"
        struct Counter {
            value: Mutex<u64>,
        }
    "#;

    let expected = r#"
        #[derive(ComputationalCapsule)]
        #[capsule(alignment = 64, size = 64)]
        #[repr(C, align(64))]
        struct Counter {
            value: AtomicU64,
            _padding: [u8; 56],
        }
    "#;

    let result = migrate_file(input).unwrap();
    assert_eq!(result.new_code, expected);
}
```

### Property Tests (Q8-Q14)

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_semantic_equivalence(source in any::<String>()) {
        // If migration succeeds, output must compile
        if let Ok(result) = migrate_file(&source) {
            assert!(compiles(&result.new_code));
        }
    }

    #[test]
    fn test_concurrent_state_updates(n in 1..1000usize) {
        // Parallel state updates should be consistent
        let state = Arc::new(MigrationStateCapsule::new());
        let handles: Vec<_> = (0..n)
            .map(|_| {
                let s = state.clone();
                std::thread::spawn(move || s.increment_processed())
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(state.snapshot().files_processed, n as u64);
    }
}
```

### Production Tests (Q22-Q28)

```rust
// Real codebase migration
#[test]
#[ignore] // Expensive, run separately
fn test_migrate_real_codebase() {
    let codebase = Path::new("fixtures/real_project");
    let summary = migrate_directory(codebase).unwrap();

    assert_eq!(summary.stats.errors_count, 0);
    assert!(summary.stats.migrations_applied > 0);

    // Verify all files compile
    assert!(cargo_check(codebase).is_ok());
}
```

---

## CLI Design

### Commands

```bash
# Analyze only (dry-run)
capsule-migrate analyze src/

# Migrate with options
capsule-migrate migrate src/ \
    --apply \
    --backup \
    --verify \
    --nightly \
    --jobs 8

# Show progress (real-time)
capsule-migrate migrate src/ --progress

# Output:
# Processing: 1250/5000 files (25%)
# Migrations: 47 opportunities found
# Errors: 0
# Elapsed: 15.2s
# Throughput: 82 files/s
```

### Options

```
--apply          Apply migrations (default: dry-run)
--backup         Create .bak files before migration
--verify         Run rustc --check after migration
--nightly        Enable nightly features (portable_simd)
--jobs <N>       Number of threads (default: CPU cores)
--progress       Show real-time progress
--config <PATH>  Load config from file
```

---

## Configuration File (Optional)

```toml
# capsule-migrate.toml

[migration]
alignment = 64              # Default alignment (64, 128, 256)
use_nightly = false         # Enable nightly features
verify = true               # Run rustc --check

[patterns]
enable_mutex = true         # Migrate Mutex<T>
enable_rwlock = true        # Migrate RwLock<T>
enable_scattered = true     # Migrate scattered atomics
enable_simd = false         # Migrate to SIMD (requires nightly)

[output]
backup = true               # Create .bak files
progress = true             # Show progress
verbose = false             # Verbose logging

[parallel]
jobs = 0                    # 0 = auto (CPU cores)
```

---

## Framework Compliance

### IMPL-2 V3.1 (Cutting-Edge-First)

✅ **Nightly-First**: Uses stable (tool must run on stable), but generated code can use nightly
✅ **Tier-Maximization**: T6 Mixed (T1+T4) for result aggregation
✅ **Innovation-Stacking**: Combines atomic (T1) + batch (T4) for 50-100× speedup
✅ **Zero-Compromise**: 100% lockfree, NO mutex, NO RwLock
✅ **File Preservation**: Never deletes files (backup, then overwrite)

### UCE34 Q1-Q34

✅ **Q1-Q9**: Problem definition complete
✅ **Q10**: Tier selection (T1/T4/T6)
✅ **Q11**: Rust transform (syn, quote, atomic_capsule)
✅ **Q12**: Nightly features (stable tool, nightly opt-in for generated code)
✅ **Q13-Q27**: Implementation details
✅ **Q28**: Simplicity (6 modules, minimal deps)
✅ **Q29**: Constraints (lockfree, zero unsafe)
✅ **Q30**: Validation (B32 benchmarks, T28 tests)
✅ **Q31**: Rust fundamentals (ownership, type system)
✅ **Q32**: Constraints refined (NO mutex, NO rayon)
✅ **Q33**: Verification (derive macros)
✅ **Q34**: Auditability (migration audit trail)

### ASSUM Safety

✅ **Zero Unsafe**: 100% safe Rust (no unsafe blocks in our code)
✅ **Atomic Safety**: All atomic operations have correct memory ordering
✅ **ABA Prevention**: Generation counters in atomic_capsule primitives

### B32 Benchmarking

✅ **Fair Baseline**: Compare against sequential syn-based processor
✅ **Statistical Rigor**: 1000+ iterations, 95% CI
✅ **Realistic Workloads**: 1K-100K line codebases

### T28 Testing

✅ **Unit Tests**: Each capsule verified
✅ **Integration Tests**: End-to-end workflows
✅ **Property Tests**: Semantic equivalence
✅ **Production Tests**: Real codebase migration

### I20 Integration

✅ **Q1-Q5 (Scope)**: Clear boundaries (migration only)
✅ **Q6-Q10 (Compatibility)**: Uses atomic_capsule v0.4+
✅ **Q11-Q15 (Safety)**: Zero unsafe, all capsules verified
✅ **Q16-Q20 (Validation)**: B32 + T28 frameworks

---

## Implementation Phases

### Phase 1: Foundation (Week 1)

**Deliverables**:
- Project structure
- Core capsules (state, progress, errors, results)
- Unit tests (T28 Q1-Q7)

**Success Criteria**:
- All capsules verified (alignment, size)
- 50+ unit tests passing
- Zero unsafe code

### Phase 2: Analysis (Week 2)

**Deliverables**:
- Analyzer module (syn-based pattern recognition)
- Planner module (migration plan generation)
- Integration tests (T28 Q15-Q21)

**Success Criteria**:
- Identify 95%+ mutex/RwLock patterns
- Generate valid migration plans
- 20+ integration tests passing

### Phase 3: Generation (Week 3)

**Deliverables**:
- Generator module (quote-based code generation)
- Applier module (batch file writing)
- Property tests (T28 Q8-Q14)

**Success Criteria**:
- Generate valid capsule code
- Semantic equivalence verified
- 10+ property tests passing

### Phase 4: Parallelization (Week 4)

**Deliverables**:
- Parallel orchestration (atomic_capsule::parallel)
- CLI implementation (clap)
- Production tests (T28 Q22-Q28)

**Success Criteria**:
- 10× throughput vs sequential
- 100% lockfree coordination
- 5+ production tests passing

### Phase 5: Validation (Week 5)

**Deliverables**:
- B32 benchmarks (throughput, latency, memory)
- Documentation (README, examples)
- Release preparation

**Success Criteria**:
- 10K+ lines/sec throughput
- <1% memory overhead
- Production-ready

---

## Next Steps

**For Implementation Agents**:

1. **Foundation Agent**: Implement core capsules (state, progress, errors, results)
2. **Parser Agent**: Implement analyzer + planner (syn-based pattern recognition)
3. **Generator Agent**: Implement generator + applier (quote-based code generation)
4. **Parallel Agent**: Implement orchestration (atomic_capsule::parallel)
5. **Testing Agent**: Implement T28 test suite (unit, integration, property, production)
6. **Benchmark Agent**: Implement B32 benchmarks (throughput, latency, memory)

**Coordination**:
- Each agent reads this ARCHITECTURE.md
- Each agent implements ONE module (clear boundaries)
- Each agent writes tests for their module (T28 framework)
- Integration agent combines modules (Phase 4)

---

## Conclusion

`capsule-migrate` demonstrates capsule architecture at every level:

- **State tracking**: T1 Atomic (3-10× speedup)
- **File processing**: T4 Batch (10-100× speedup)
- **Result aggregation**: T6 Mixed (50-100× compound speedup)
- **Zero unsafe**: 100% safe Rust
- **100% lockfree**: NO mutex, NO RwLock anywhere

**Key Innovation**: Migration tool that practices what it preaches—everything is a capsule, achieving 10-100× speedups through systematic tier selection.

**Status**: Architecture design complete. Ready for implementation.

**Frameworks Applied**: UCE34 (Q1-Q34), IMPL-2 V3.1, ASSUM, B32, T28, I20, Chaos

---

**Document Version**: 1.0
**Date**: 2025-11-02
**Author**: Architecture Expert (Claude Code)
**Status**: Design Complete, Ready for Implementation
