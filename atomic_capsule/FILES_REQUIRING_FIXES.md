FILES REQUIRING FIXES FOR VERIFICATION MIGRATION
================================================

=== ISSUE #1: Missing "std" Feature (P0, 5 min fix) ===

File: /home/samuel/Primitives/atomic_capsule_derive/Cargo.toml
Action: Add "std" to [features] section
Current Line 15-27:
  [features]
  default = []
  nightly-const-fn = []
  nightly-specialization = []
  nightly-const-traits = []
  nightly-all = ["nightly-const-fn", "nightly-specialization", "nightly-const-traits"]
  parallel = ["rayon"]

Should be:
  [features]
  default = []
  std = []  # ADD THIS LINE
  nightly-const-fn = []
  ...

=== ISSUE #2: Test Failure - Task Counting (P0, 1-2 hr fix) ===

File: /home/samuel/Primitives/atomic_capsule/src/parallel/tests/property.rs
Line: 368 (assertion left == right failed: len() 864 != actual count 1000)

Files to Debug:
  1. src/parallel/progress_tracker.rs
     - ProgressTrackerCapsule::len() implementation
     - Check generation counter synchronization
  
  2. src/parallel/work_stealing.rs or similar
     - Task execution counter logic
     - Check for ABA races or incomplete atomic updates
  
  3. src/parallel/tests/property.rs (line ~320-370)
     - Review test setup and assertion

Action: Add detailed logging to:
  - Every atomic load/store in generation counter
  - CAS operation results
  - Task completion callbacks

=== ISSUE #3: SIGSEGV - Memory Safety (P0, 2-4 hr fix) ===

Affected Test Suite: All tests in src/parallel/tests/

Files Potentially Responsible:
  1. src/parallel/work_stealing.rs
     - AtomicPtr<Task> manipulation
     - Dangling pointer risk
  
  2. src/parallel/scoped.rs
     - Task scope lifecycle
     - Drop order issues
  
  3. src/alignment.rs or src/arch.rs
     - Alignment calculation
     - Could cause SIMD access violations

  4. src/patterns/cache_aligned.rs (if exists)
     - Cache-line padding calculation

Action: Run with valgrind/ASAN to pinpoint exact location
  valgrind --leak-check=full --show-leak-kinds=all cargo test --lib --features std 2>&1 | grep -A 20 "Invalid"

=== ISSUE #4: Feature-Gated impl Blocks (P0, 1-2 hr fix) ===

FILES REQUIRING REFACTOR (48 total with #[cfg(feature = "derive")]):

Critical Files (highest priority):
  1. src/http/state.rs
     Lines: Contains #[cfg_attr(feature = "derive", capsule(...))]
     Action: Remove feature gate from impl blocks
  
  2. src/mmap/region.rs
     Lines: Feature-gated derive and impl blocks
     Action: Make derive ALWAYS present, unconditional impl
  
  3. src/patterns/wiring.rs
     Lines: #[cfg(feature = "derive")]
     Action: Remove feature gate
  
  4. src/protection/obfuscation.rs
     Action: Refactor to unconditional impl

Complete List of Files (from grep output):
  - src/protection/obfuscation.rs
  - src/primitives/simd_vectorization.rs
  - src/protection/audit_log_q34.rs
  - src/patterns/lockfree_task_executor.rs
  - src/platform/native/network/shard_capsule.rs
  - src/platform/native/persistence/region.rs
  - src/mmap/region.rs
  - src/network/shard_capsule.rs
  - src/parallel/result_aggregator_v2.rs
  - src/core/atomic/dual_atomic.rs
  - src/http/state.rs
  - src/collections/lockfree_btree/node.rs
  - src/collections/lockfree_btree/stats.rs
  - src/persistence/simd_vector.rs
  - src/collections/lockfree_table_old.rs
  - src/collections/cache.rs
  - src/collections/cache_integrated.rs
  - src/collections/concurrent_map_v2.rs
  - src/collections/lockfree_btree/cow_leaf.rs
  - src/collections/lockfree_btree/hybrid.rs
  - src/collections/lockfree_btree/mod.rs
  - src/collections/lockfree_table.rs
  - src/collections/concurrent_map.rs
  - src/hash/simd_hash_capsule.rs
  - src/protection/kernel_coordination.rs
  - src/protection/remote_attestation.rs
  - src/protection/audit_trail.rs
  - src/protection/orchestrator.rs
  - src/protection/anomaly_detector.rs
  - src/primitives/progress_tracker.rs
  - src/primitives/atomic_simd_fixed.rs
  - src/primitives/coordination/phase_coordinator.rs
  - src/primitives/coordination/hash_bucket.rs
  - src/primitives/coordination/parallel_partition.rs
  - src/streaming/strategy_labeler.rs
  - src/probabilistic/hyperloglog.rs
  - src/composite/full_compound.rs
  - src/composite/tier1_tier2_tier3.rs
  - src/composite/tier1_tier2.rs
  - src/composite/simd_fixed_point.rs
  - src/composite/tier2_tier3.rs
  - src/composite/atomic_simd.rs
  - src/parallel/lockfree_list.rs
  - src/platform/wasm/simd_nightly.rs
  - src/platform/wasm/simd.rs
  - src/patterns/dual_atomic.rs
  - src/patterns/wiring.rs
  - src/patterns/circuit_breaker/diag.rs

Refactoring Pattern (apply to each file):
  BEFORE:
    #[cfg(feature = "derive")]
    impl MyStruct {
      fn method1() { ... }
    }
    #[cfg(not(feature = "derive"))]
    impl MyStruct {
      fn method1_fallback() { ... }
    }
  
  AFTER:
    impl MyStruct {
      fn method1() { ... }  # ALWAYS PRESENT
    }

=== ISSUE #5: Untracked Directory (P1, 2 min fix) ===

File: src/primitives/fixed_point/
  - src/primitives/fixed_point/mod.rs (23,369 bytes)
  - src/primitives/fixed_point/quantizer.rs (13,233 bytes)

Action: git add src/primitives/fixed_point/

=== ISSUE #6: Compilation Warnings (P2, 15 min fix) ===

Individual Warnings:

1. src/error.rs:11
   Unused import: std::io
   Fix: Remove line 11

2. src/collections/append_only_map_optimized.rs:100
   Dead method: is_occupied()
   Fix: Remove fn is_occupied(&self) -> bool

3. src/collections/concurrent_map.rs:216
   Dead method: try_claim()
   Fix: Remove fn try_claim() method

4. src/collections/entry.rs:453
   Dead method: try_get()
   Fix: Remove pub(crate) fn try_get() method

5. atomic_capsule_derive/src/audit.rs:39
   Unused enum: AuditError
   Fix: Mark with #[allow(dead_code)] or remove

6. atomic_capsule_derive/src/audit.rs:96
   Unused enum: MigrationStatus
   Fix: Mark with #[allow(dead_code)] or remove

7. src/collections/lockfree_table.rs (multiple lines: 1771, 1798, 1979, 2045, 2075)
   Unused Result warnings
   Fix: Add let _ = before each insert() call

Bulk Fix:
  cargo fix --lib -p atomic_capsule --allow-dirty
  cargo fix --lib -p atomic_capsule_derive --allow-dirty

=== PRIORITY EXECUTION ORDER ===

CRITICAL (blocks verification migration):
  1. Fix Issue #3 (SIGSEGV) - 2-4 hours
     Run: valgrind to find exact location
  
  2. Fix Issue #2 (task counter bug) - 1-2 hours
     Debug: Add logging to generation counter code
  
  3. Fix Issue #1 (missing std feature) - 5 minutes
     Edit: /home/samuel/Primitives/atomic_capsule_derive/Cargo.toml
  
  4. Fix Issue #4 (feature gating) - 1-2 hours
     Refactor: Remove #[cfg(feature = "derive")] from impl blocks

RECOMMENDED (before merge):
  5. Fix Issue #5 (untracked directory) - 2 minutes
     Action: git add src/primitives/fixed_point/
  
  6. Fix Issue #6 (warnings) - 15 minutes
     Action: cargo fix --lib --allow-dirty

