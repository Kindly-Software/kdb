# T9 Persistent Capsule - Required Code Changes

**Quick Reference**: Minimal changes to enable T9 Persistent tier

---

## 1. Cargo.toml - Add Feature Flags

**Location**: After line 494 (before `[dependencies]`), add:

```toml
# T9 Persistent Capsule (Memory-Mapped Atomic State) - NIGHTLY REQUIRED
persistent = ["std", "dep:memmap2", "dep:bytemuck", "nightly-atomic"]
persistent-audit = ["persistent", "audit-trail"]
persistent-recovery = ["persistent"]
persistent-all = ["persistent-audit", "persistent-recovery"]
persistent-minhash = ["persistent-all", "probabilistic"]
```

---

## 2. Cargo.toml - Add Dependencies

**Location**: After line 524 (with other optional dependencies), add:

```toml
# T9 Persistent tier dependencies
memmap2 = { version = "0.9", optional = true }
bytemuck = { version = "1.14", optional = true, features = ["derive"] }
```

**NOTE**: Check if `bytemuck` already exists. If so, merge features instead of duplicating.

---

## 3. lib.rs - Enable Nightly Feature

**Location**: After line 132 (after `nightly-atomic` feature gate), add:

```rust
#![cfg_attr(feature = "persistent", feature(atomic_from_mut))]
```

---

## 4. lib.rs - Add Module Declaration

**Location**: After line 260 (after `probabilistic` module), add:

```rust
// T9 Persistent Capsule - Memory-mapped atomic state (requires nightly-atomic)
#[cfg(feature = "persistent")]
pub mod persistent;
```

---

## 5. lib.rs - Add Re-exports

**Location**: After line 310 (after `atomic_from_mut` re-exports), add:

```rust
// Re-export T9 Persistent capsules
#[cfg(feature = "persistent")]
pub use persistent::{
    PersistentAtomicCapsule,
    PersistentError,
    PersistentResult,
    FlushMode,
};

#[cfg(feature = "persistent-minhash")]
pub use persistent::{
    PersistentMinHashCapsule,
    PersistentDedupIndex,
};
```

---

## 6. build.rs - Create Nightly Detection (OPTIONAL)

**Location**: Create new file `/home/samuel/Primitives/atomic_capsule/build.rs`

**Content**: See T9_BUILD_CONFIGURATION.md § 4

**Purpose**: Warn users if they try to build T9 with stable Rust

---

## 7. Cargo.toml - Add Test/Bench Entries

**Location**: After line 812 (existing benchmarks), add:

```toml
# T9 Persistent Capsule Tests & Benchmarks
[[bench]]
name = "persistent_bench"
harness = false
required-features = ["persistent-all"]

[[test]]
name = "persistent_crash_recovery"
required-features = ["persistent-all"]

[[test]]
name = "persistent_multi_process"
required-features = ["persistent-all"]
```

---

## Quick Verification

After making changes, verify with:

```bash
# 1. Check feature flags compile (no implementation yet)
cargo +nightly check --features persistent-all

# 2. Verify dependencies resolve
cargo +nightly tree --features persistent-all | grep -E "(memmap2|bytemuck)"

# 3. Test stable fallback (should fail gracefully)
cargo check --features persistent-all 2>&1 | grep -i "nightly"
```

**Expected**: Compilation errors about missing `persistent` module (normal until implementation).

---

## Implementation Order

1. **Phase 1**: Add feature flags + dependencies (this document)
2. **Phase 2**: Create `src/persistent/mod.rs` skeleton
3. **Phase 3**: Implement `PersistentAtomicCapsule` (core)
4. **Phase 4**: Implement `PersistentMinHashCapsule` (LLM dedup)
5. **Phase 5**: Write T28 tests (unit/property/integration/production)
6. **Phase 6**: Write B32 benchmarks (vs serde + fs baseline)

---

**Status**: Configuration Complete - Ready for Implementation
