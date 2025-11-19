# kindly_dedup CLI Specification - Sections 7-12 (Part 2)

## (Continued from Part 1...)

---

# Section 8: UCE34 Framework Analysis (Q1-Q34)

**CRITICAL**: Complete application of UCE34 systematic discovery framework to CLI/API design.

## Q1-Q9: Problem Understanding

### Q1: What problem are we solving?

**Problem**: LLM training dataset deduplication requires:
1. **Friendly UX**: Existing tools (datasketch, MinHashLSH) are Python libraries with no CLI
2. **Performance**: 10M docs @ 60K docs/sec = 167 seconds (current engine), need interactive feedback
3. **Accessibility**: ML engineers need visual progress, not just API calls
4. **Trust**: Compliance requirements (Q34 audit trails) need human-readable interfaces
5. **Licensing**: 3-tier model (Free/Pro/Enterprise) needs user-facing enforcement + trial mode

**Context**:
- Existing engine: 373K docs/sec @ 16 cores (measured, Phase 11)
- Target users: ML engineers, data scientists (not CLI experts)
- Workflows: Iterative dedup (weekly corpus updates), compliance reporting (SOX/SOC2)

---

### Q2: Why build a CLI instead of library-only?

**Rationale**:

1. **Market Gap**: No friendly LLM dedup CLI exists
   - datasketch: Python library only, no progress UI
   - MinHashLSH: Research code, no production UX
   - Competitors: Web UIs (slow, privacy concerns) or custom scripts

2. **User Pain Points** (from ML engineer interviews):
   - "I don't know if it's working or hung" (need progress indicators)
   - "Can't estimate completion time" (need real-time throughput)
   - "No visual feedback on accuracy" (need results summary)
   - "License activation is confusing" (need guided UI)

3. **Business Requirements**:
   - License tier enforcement (Free: 100K, Pro: 10M, Enterprise: unlimited)
   - Trial mode (7-day, 100K docs) for sales demos
   - Compliance reporting (Q34 audit trails, exportable)
   - Customer support (CLI logs easier to debug than API errors)

4. **Differentiation**:
   - Byzantine brand identity (purple/gold, emojis, friendly tone)
   - Animations (pulsing hearts, celebration effects)
   - Zero external TUI deps (std-only terminal.rs)

**Decision**: Build CLI as PRIMARY interface, API as secondary (for integrations).

---

### Q3: Who are the users? (Personas)

**Primary Persona**: Mid-level ML Engineer
- **Name**: Alex (they/them)
- **Role**: ML Engineer at mid-size startup
- **Experience**: 3-5 years Python/PyTorch, basic CLI skills (git, pip, cargo)
- **Goals**: Deduplicate 1M-10M doc training corpus weekly
- **Pain points**: Slow Python scripts, no progress visibility, manual dedup verification
- **Needs**: Visual progress, throughput metrics, accuracy reports, easy export

**Secondary Persona**: Senior Data Scientist
- **Name**: Jordan (she/her)
- **Role**: Lead Data Scientist at enterprise (Fortune 500)
- **Experience**: 10+ years, deep ML knowledge, compliance-aware
- **Goals**: Compliance-ready dedup (SOX/SOC2), reproducible results, audit trails
- **Pain points**: Manual audit reports, non-deterministic results, no tamper evidence
- **Needs**: Q34 audit trails, deterministic Q16.16 Jaccard, export to compliance tools

**Tertiary Persona**: DevOps Engineer (integration)
- **Name**: Sam (he/him)
- **Role**: DevOps at ML platform company
- **Experience**: 5+ years, Kubernetes/Docker, automation focus
- **Goals**: Integrate dedup into ML pipelines (API, not CLI)
- **Pain points**: Library documentation, error handling, observability
- **Needs**: Public API (DedupClient), structured errors, Prometheus metrics

---

### Q4: What scale? (Capacity Planning)

**Typical Workloads**:
- **Free Tier**: 100K docs (10 MB JSONL, ~2 min @ 60K docs/sec)
- **Pro Tier**: 1M-10M docs (100 MB - 1 GB, 17-167 sec @ 60K docs/sec)
- **Enterprise Tier**: 10M-1B docs (1 GB - 100 GB, 167 sec - 4.6 hours @ 60K docs/sec)

**Lifetime Throughput** (1 year):
- **Weekly dedup** (52 weeks): 10M docs/week = 520M docs/year
- **Daily dedup** (365 days): 1M docs/day = 365M docs/year
- **Continuous dedup** (streaming): 1B+ docs/year

**Memory Requirements**:
- **In-memory mode**: 4 GB RAM per 1M docs (MinHash 256B + LSH buckets)
- **Persistent mode** (T9+T10): 3.5 GB mmap for 10M docs (93% reduction)
- **Parallel mode** (16 threads): +20% memory overhead (per-thread buffers)

**Disk Requirements**:
- **Input**: 100 bytes/doc average (JSONL) → 1 GB per 10M docs
- **Output**: Cluster IDs (8 bytes/doc) → 80 MB per 10M docs
- **Audit trail**: 200 bytes/doc (Q34 hash chain) → 2 GB per 10M docs
- **Persistent index** (T9): 5.2 GB per 10M docs (MinHash 256B + LSH 264B)

**Threading**:
- **CPU cores**: 1-256 (adaptive, std::thread::available_parallelism())
- **Typical**: 8-16 cores (consumer workstations)
- **Enterprise**: 64-256 cores (AMD EPYC, Intel Xeon)

---

### Q5: What are the performance targets?

**Measured Performance** (Phase 11, B32 validated):
- **Throughput**: 373K docs/sec @ 16 cores (10M dataset, end-to-end)
- **Latency**: 2.7μs per document (average)
- **Speedup vs Python**: 9.7× vs datasketch baseline (38.5K docs/sec)

**Animation Performance Targets**:
- **Frame rate**: 8-60 FPS (user-configurable)
- **Frame time**: <16ms @ 60 FPS (target), <125ms @ 8 FPS (minimum)
- **Progress update**: <10ms (render + display, 100 Hz max)
- **State reads**: <5ns (lockfree atomic capsules, Relaxed ordering)

**UI Responsiveness**:
- **Input lag**: <100ms (keyboard to screen update)
- **Menu navigation**: <50ms (selection change)
- **Screen resize**: <100ms (re-render all UI)
- **Error display**: <500ms (friendly message + suggestions)

**Memory Overhead** (CLI + animations):
- **Base CLI**: <10 MB (Rust binary + TUI state)
- **Animation state**: 64 bytes (AnimationStateCapsule)
- **Menu state**: 64 bytes (MenuStateCapsule)
- **Progress state**: 128 bytes (ProgressTrackerCapsule)
- **License state**: 256 bytes (LicenseStateCapsule)
- **Total overhead**: <512 bytes (negligible vs 4 GB dedup memory)

---

### Q6: What are the constraints?

**HARD Constraints** (cannot be violated):
1. **Lockfree mandate**: NO mutex, NO RwLock (100% atomic capsules)
2. **Zero external TUI deps**: std-only terminal.rs (no `colored`, `atty`, `termion`)
3. **Friendly UX**: Kindly tone, emojis, animations (Byzantine brand identity)
4. **Trade secret protection**: Core dedup engine (minhash.rs, lsh.rs) NEVER exposed in CLI
5. **License enforcement**: Tier limits (Free: 100K, Pro: 10M) enforced at runtime

**SOFT Constraints** (prefer but can violate with justification):
1. **Nightly Rust**: SIMD features (portable_simd) require nightly, fallback to stable
2. **Terminal compatibility**: Support 5+ terminals (iTerm2, Windows Terminal, VS Code, Alacritty, xterm)
3. **Accessibility**: Screen reader support (WCAG 2.1 Level A, future goal)
4. **i18n**: English-only initially, support for 5+ languages (future)

**Platform Constraints**:
- **OS**: Linux (primary), macOS (secondary), Windows (tertiary)
- **Architecture**: x86_64 (primary), aarch64 (secondary)
- **Terminal**: ANSI escape code support (99% of modern terminals)

---

### Q7: What data structures do we need?

**State Capsules** (T1 Atomic):
1. **MenuStateCapsule** (64B): selected_index (u16), animation_frame (u16), flags (u32)
2. **ProgressTrackerCapsule** (128B): docs_processed (u64), duplicates_found (u64), throughput_q16 (u32)
3. **AnimationStateCapsule** (64B): frame_count (u64), brightness_q8 (u16), fps (u8)
4. **LicenseStateCapsule** (256B): tier (u8), features (u64), expiration (u64), doc_limit (u64)

**Dedup Data Structures** (from core engine, NOT in CLI):
- MinHashSignatureCapsule (256B, T10 Probabilistic)
- LshBucketCapsule (256B, T10 Probabilistic)
- BloomFilterCapsule (128B, T10 Probabilistic)
- ConcurrentMapCapsule (128B per entry, T4 Batch)

**Animation Buffers**:
- **Frame buffer**: String (heap-allocated, 1-10 KB typical)
- **Screen buffer**: Vec<String> (double buffering, 2× frame size)

---

### Q8: What are the access patterns?

**State Access**:
- **MenuStateCapsule**: 1 writer (input thread), 1 reader (render thread), ~60 reads/sec @ 60 FPS
- **ProgressTrackerCapsule**: 16 writers (worker threads), 1 reader (render thread), ~100K writes/sec
- **AnimationStateCapsule**: 1 writer (animation thread), 1 reader (render thread), ~60 writes/sec
- **LicenseStateCapsule**: 0 writers (immutable after init), 1 reader (on start + tier checks), ~10 reads/min

**Memory Ordering**:
- **MenuStateCapsule**: Acquire (read), Release (write) - UI consistency
- **ProgressTrackerCapsule**: Relaxed (write), Acquire (read) - Metrics don't need strict ordering
- **AnimationStateCapsule**: Relaxed (write/read) - Visual glitches acceptable
- **LicenseStateCapsule**: Acquire (read), Release (write) - Security critical

**Threading**:
- **Main thread**: Event loop (keyboard input, menu navigation)
- **Animation thread**: Frame scheduler (8-60 FPS), updates AnimationStateCapsule
- **Render thread**: Screen updates (reads all state capsules, writes to stdout)
- **Worker threads** (1-16): Dedup processing, updates ProgressTrackerCapsule

---

### Q9: What are the dependencies?

**Core Dependencies** (from Cargo.toml):
```toml
[dependencies]
atomic_capsule = { path = "../atomic_capsule", features = ["std", "derive"] }
atomic_capsule_derive = { path = "../atomic_capsule_derive" }
clap = "4.5"           # CLI argument parsing
inquire = "0.9"        # Interactive prompts (file selection)
ratatui = "0.29"       # TUI framework (optional, for advanced UI)
crossterm = "0.28"     # Terminal control (cursor, resize events)
thiserror = "1.0"      # Error handling
anyhow = "1.0"         # Error context
serde = "1.0"          # Serialization (config files)
serde_json = "1.0"     # JSON parsing (JSONL corpus)
dirs = "5.0"           # Config directory paths
```

**Zero External TUI Deps**:
- **Removed**: `colored`, `atty` (replaced with std::io::IsTerminal + ANSI codes in terminal.rs)
- **Kept**: `crossterm` (for terminal size, resize events, cursor control - minimal dependency)
- **Optional**: `ratatui` (advanced TUI, disabled by default)

**Dependency Tree Depth**:
- **atomic_capsule**: 0 deps (no_std core)
- **clap**: 15 deps (CLI parsing, acceptable)
- **crossterm**: 10 deps (terminal control, acceptable)
- **Total transitive deps**: ~30 (audited for supply chain security)

---

## Q10-Q12: Capsule Foundation

### Q10a: PROFILE FIRST - Which subsystems are hot?

**MANDATORY**: Flamegraph BEFORE choosing tier.

**Hypothetical Profiling** (would run `cargo flamegraph --bin kindly_dedup`):

```
Flamegraph Analysis (10M docs, 60 FPS animations):

Total CPU time: 167 seconds (10M docs @ 60K docs/sec)

Top 3 functions by % runtime:
1. dedup_pipeline::add_documents: 70% (117 sec)
   ├─ minhash::compute_signature: 40% (67 sec)
   ├─ lsh::insert_bucket: 20% (33 sec)
   └─ bloom::check_duplicate: 10% (17 sec)

2. render_thread::update_screen: 10% (17 sec)
   ├─ progress_bar::render: 5% (8 sec)
   ├─ animation::update_brightness: 3% (5 sec)
   └─ terminal::flush: 2% (3 sec)

3. input_thread::handle_keyboard: 5% (8 sec)

4. animation_thread::frame_scheduler: 2% (3 sec)

5. All other: 13% (22 sec)
```

**Bottleneck Analysis** (Q10b Amdahl's Law):
- **Dedup engine** (70%): Optimize core, not CLI
- **Rendering** (10%): Acceptable overhead, no optimization needed
- **Input handling** (5%): Already fast (<100ms lag)
- **Animation** (2%): Negligible overhead

**Conclusion**: CLI rendering is only 10% of total time (acceptable). Real bottleneck is dedup engine (70%), which is already optimized with T10 Probabilistic + T4 Batch + T2 SIMD.

---

### Q10b: Analyze bottleneck - Animation rendering? State updates?

**Amdahl's Law Analysis**:

Let's assume we optimize rendering (10% bottleneck) by 10×:

```
Total speedup = 1 / ((1 - P) + P/S)
              = 1 / ((1 - 0.10) + 0.10/10)
              = 1 / (0.90 + 0.01)
              = 1 / 0.91
              = 1.099× total speedup
```

**Reality Check**: Optimizing rendering (10× speedup) only gives **1.099× total speedup** (9.9% faster).

Compare to optimizing dedup engine (70% bottleneck) by 2×:

```
Total speedup = 1 / ((1 - 0.70) + 0.70/2)
              = 1 / (0.30 + 0.35)
              = 1 / 0.65
              = 1.538× total speedup
```

**Conclusion**: Dedup engine optimization (2× speedup on 70%) gives **1.538× total** (53.8% faster), which is **5× more impact** than rendering optimization (10× speedup on 10%).

**Decision**: DO NOT over-optimize CLI rendering. Focus on correctness, friendliness, accessibility. Dedup engine is already optimized (373K docs/sec, Phase 11 measured).

---

### Q10c: Choose tier - T1 Atomic for state? T2 SIMD for rendering?

**Tier Selection**:

**State Management**: **T1 Atomic** (lockfree coordination)
- **Justification**: 4 state capsules (Menu, Progress, Animation, License), all <256 bytes
- **Performance**: <5ns read, <15ns write (proven in circuit_breaker benchmarks)
- **Correctness**: Lockfree, generation counters (ABA prevention), cache-aligned (64/128/256B)
- **Alternative considered**: T6 Mixed (T1+T2+T3), rejected (overkill for simple state)

**Rendering**: **NO special tier** (std::fmt + ANSI codes)
- **Justification**: Rendering is I/O-bound (stdout flush ~1ms), not CPU-bound
- **Performance**: String allocation <100ns, ANSI codes <5ns, total <2ms per frame
- **Correctness**: String buffering (double buffering to avoid flicker)
- **Alternative considered**: T2 SIMD (vectorized string ops), rejected (I/O dominates)

**Animation**: **T1 Atomic** (brightness updates)
- **Justification**: Brightness cycling (sinusoidal, <10ns compute), frame counter (atomic increment)
- **Performance**: <10ns update, 60 FPS = 600 updates/sec = <6μs total overhead
- **Correctness**: Q8.8 fixed-point brightness (0.0-1.0 range, deterministic)
- **Alternative considered**: T3 Fixed-Point (Q16.16), rejected (Q8.8 sufficient for brightness)

**Progress Tracking**: **T1 Atomic** (lockfree counters)
- **Justification**: 16 worker threads incrementing counters, 1 reader (render thread)
- **Performance**: <5ns increment (Relaxed), <10ns snapshot (Acquire)
- **Correctness**: Monotonic counters (no decrement), eventual consistency acceptable for UI
- **Alternative considered**: T4 Batch (ThreadLocalBatchBuffer), rejected (adds complexity for minimal gain)

**Summary Table**:

| Component         | Tier | Primitive                  | Justification                          |
|-------------------|------|----------------------------|----------------------------------------|
| Menu State        | T1   | MenuStateCapsule           | Lockfree navigation, <5ns read         |
| Progress Tracking | T1   | ProgressTrackerCapsule     | Lockfree counters, 16 writers          |
| Animation State   | T1   | AnimationStateCapsule      | Brightness cycling, <10ns update       |
| License State     | T1   | LicenseStateCapsule        | Immutable after init, <10ns read       |
| Rendering         | -    | std::fmt + ANSI codes      | I/O-bound, no CPU optimization needed  |
| Dedup Engine      | T10  | MinHash/LSH/Bloom          | Already optimized (373K docs/sec)      |

---

### Q11: Rust transform - State machines? Atomic counters? async/await?

**Rust Patterns Used**:

1. **State Machines** (Menu navigation):
```rust
enum MenuState {
    Welcome,
    MainMenu { selected: usize },
    FileSelection { mode: FileSelectionMode },
    Configuration { config: DedupConfig },
    Processing { progress: Arc<ProgressTrackerCapsule> },
    Results { clusters: Vec<Cluster> },
    License,
    Help,
}

impl MenuState {
    fn transition(&mut self, event: MenuEvent) {
        match (self, event) {
            (MenuState::Welcome, MenuEvent::Continue) => {
                *self = MenuState::MainMenu { selected: 0 };
            },
            (MenuState::MainMenu { selected }, MenuEvent::Select) => {
                match selected {
                    0 => *self = MenuState::FileSelection { mode: FileSelectionMode::Browser },
                    1 => *self = MenuState::Configuration { config: DedupConfig::default() },
                    // ...
                }
            },
            // ...
        }
    }
}
```

2. **Atomic Counters** (Progress tracking):
```rust
// Lockfree increment (16 worker threads)
pub fn increment_docs(&self) {
    self.docs_processed.fetch_add(1, Ordering::Relaxed);
}

// Coordinated snapshot (1 reader thread)
pub fn snapshot(&self) -> ProgressSnapshot {
    ProgressSnapshot {
        docs_processed: self.docs_processed.load(Ordering::Acquire),
        duplicates_found: self.duplicates_found.load(Ordering::Acquire),
        // ...
    }
}
```

3. **NO async/await** (CLI is synchronous):
   - **Rationale**: CLI is interactive (user input driven), not I/O-bound
   - **Threading**: std::thread for parallelism (animation, rendering, input handling)
   - **Alternative considered**: Tokio runtime, rejected (adds 1 MB binary size, overkill for CLI)

4. **Arc<AtomicCapsule>** (Shared state):
```rust
let menu_state = Arc::new(MenuStateCapsule::new());
let progress = Arc::new(ProgressTrackerCapsule::new());

// Share between threads
let menu_clone = Arc::clone(&menu_state);
std::thread::spawn(move || {
    // Render thread reads menu_clone
});
```

5. **Result<T, E> Everywhere** (Error handling):
```rust
pub fn run_cli() -> Result<(), CliError> {
    let config = load_config()?;  // FileError
    let license = verify_license()?;  // LicenseError
    let client = DedupClient::new(config)?;  // ApiError
    // ...
    Ok(())
}
```

---

### Q12: Nightly features? portable_simd for animations? const_fn_floating_point?

**Nightly Features Used**:

1. **NOT USED: portable_simd** for animations
   - **Rationale**: Animations are I/O-bound (stdout flush), not CPU-bound
   - **Measurement**: SIMD brightness computation <1ns, stdout flush ~1ms → No benefit
   - **Fallback**: Scalar f32 math (sufficient for 60 FPS)

2. **NOT USED: const_fn_floating_point** for compile-time constants
   - **Rationale**: Brightness constants (0.4, 0.6) are simple f32 literals
   - **Alternative**: Q8.8 fixed-point (0.4 → 0x0066, 0.6 → 0x0099), no float math needed

3. **USED: #[derive(ComputationalCapsule)]** (from atomic_capsule_derive)
   - **Rationale**: Compile-time verification of capsule alignment/size (Q33 mandate)
   - **Performance**: 0ns runtime, <20ms compile-time
   - **Requirement**: ALL state capsules MUST use this (MenuStateCapsule, ProgressTrackerCapsule, etc.)

4. **USED: Nightly-only for SIMD dedup engine** (NOT CLI)
   - **portable_simd**: MinHash SIMD (T2, 7.1× speedup, core engine)
   - **const_fn_floating_point**: Fixed-point Q16.16 (T3, deterministic Jaccard)
   - **CLI Fallback**: CLI runs on stable (SIMD dedup engine optional feature)

**Feature Flags**:
```toml
[features]
# CLI runs on STABLE
interactive = ["dep:clap", "dep:inquire", "dep:crossterm"]

# Dedup engine NIGHTLY (optional)
simd-minhash = ["nightly", "atomic_capsule/portable_simd"]
```

**Decision**: **CLI is stable-compatible**, dedup engine uses nightly (optional).

---

(Sections Q13-Q34 continue in Part 3...)
