# Cargo.toml Modifications for Timeline Integration

**Version**: 1.0
**Date**: 2025-10-21
**Phase**: 5.8 TimelineBridge Integration
**Status**: Feature Flag Additions Required

---

## Required Changes

### 1. Feature Flag Addition

**Location**: After line 252 (after `full` feature)

**Add the following**:

```toml
# ============================================================================
# PHASE 5.8: TIMELINE AGGREGATION (UCE34 T4 Batch Tier)
# ============================================================================
# Timeline aggregation capsule for audit event analytics
# Aggregates events across 1-minute to 1-day time windows
# Zero dependencies, 100% lockfree, SIMD-accelerated histogram operations

# Timeline Aggregation: T4 Batch tier capsule for audit analytics (Phase 5.8)
# Zero new dependencies (reuses atomic_capsule infrastructure)
# Optional SIMD optimization via portable_simd feature
timeline-aggregation = ["portable_simd"]
```

### 2. Update `full` Feature

**Location**: Line 252

**Current**:
```toml
full = ["kindlydb", "oauth", "payments", "compliance", "q34-hash-chain", "payment-optimization"]
```

**Modified**:
```toml
full = ["kindlydb", "oauth", "payments", "compliance", "q34-hash-chain", "payment-optimization", "timeline-aggregation"]
```

### 3. Add Benchmark Definition

**Location**: After line 340 (after last benchmark)

**Add the following**:

```toml
[[bench]]
name = "phase5_8_timeline_aggregation_benchmarks"
harness = false
```

**Note**: This benchmark is already declared in the existing Cargo.toml at line 338, so **no action needed**.

---

## Verification

After applying changes, verify with:

```bash
# Check feature flags are correctly defined
cargo metadata --format-version 1 | jq '.packages[] | select(.name == "clapi_core") | .features'

# Expected output should include:
{
  "timeline-aggregation": ["portable_simd"],
  "full": ["kindlydb", "oauth", "payments", "compliance", "q34-hash-chain", "payment-optimization", "timeline-aggregation"]
}
```

---

## Dependency Analysis

### Zero New Dependencies Required

All TimelineBridge infrastructure exists:
- ✅ `atomic_capsule` (already present, features: `const-hashing`, `async-log`)
- ✅ `tokio` (already present, features: `full`)
- ✅ `portable-atomic` (already present, for AtomicU128 support)
- ✅ SIMD support via `portable_simd` feature (nightly-gated)

### No Changes Needed for Dependencies
The `[dependencies]` section remains unchanged.

---

## Feature Flag Testing

### Test Matrix

| Configuration | Command | Expected Result |
|---------------|---------|-----------------|
| Default (no timeline) | `cargo check --lib` | ✅ Pass (proxy-only) |
| Timeline (scalar) | `cargo check --lib --features timeline-aggregation` | ✅ Pass |
| Timeline (SIMD) | `cargo +nightly check --lib --features timeline-aggregation,portable_simd` | ✅ Pass |
| Full (all features) | `cargo check --lib --features full` | ✅ Pass |

### Backward Compatibility

- ✅ `default = ["proxy-only"]` unchanged
- ✅ Existing features unaffected
- ✅ No breaking changes to existing capsules
- ✅ New feature is **optional** (not in default)

---

## Complete Diff

```diff
# File: Cargo.toml

@@ -252,6 +252,14 @@
 # Full: All features enabled (Week 4)
-full = ["kindlydb", "oauth", "payments", "compliance", "q34-hash-chain", "payment-optimization"]
+full = ["kindlydb", "oauth", "payments", "compliance", "q34-hash-chain", "payment-optimization", "timeline-aggregation"]

+# ============================================================================
+# PHASE 5.8: TIMELINE AGGREGATION (UCE34 T4 Batch Tier)
+# ============================================================================
+# Timeline aggregation capsule for audit event analytics
+# Aggregates events across 1-minute to 1-day time windows
+# Zero dependencies, 100% lockfree, SIMD-accelerated histogram operations
+
+# Timeline Aggregation: T4 Batch tier capsule for audit analytics (Phase 5.8)
+# Zero new dependencies (reuses atomic_capsule infrastructure)
+# Optional SIMD optimization via portable_simd feature
+timeline-aggregation = ["portable_simd"]
+
 # ============================================================================
```

---

## Post-Modification Validation

### Step 1: Verify Feature Flag Syntax
```bash
cargo check --lib --features timeline-aggregation
```
**Expected**: Compilation succeeds (even before TimelineBridge implementation)

### Step 2: Verify Full Feature
```bash
cargo check --lib --features full
```
**Expected**: Compilation succeeds with all features enabled

### Step 3: Verify SIMD Feature Propagation
```bash
cargo +nightly check --lib --features timeline-aggregation,portable_simd
```
**Expected**: Nightly compilation succeeds with SIMD enabled

### Step 4: Verify Benchmark Definition
```bash
cargo bench --no-run --features timeline-aggregation
```
**Expected**: Benchmark compilation succeeds (will be empty until implementation)

---

## Notes

### Why No New Dependencies?

All TimelineBridge infrastructure exists in `atomic_capsule`:
- **AsyncLogCapsule**: T5 Streaming tier for event ingestion
- **const_hash**: 0ns compile-time bucket ID hashing
- **AtomicU64**: Histogram bucket storage (via `portable-atomic`)
- **SIMD**: Optional `portable_simd` for percentile calculations

### Why Optional SIMD?

- **Stable Rust**: Scalar implementation (50ns/bucket scan)
- **Nightly Rust**: SIMD implementation (20ns/bucket scan, 2.5× faster)
- **Graceful Degradation**: Feature-gated, no runtime dependency
- **Low Risk**: Proven pattern from Phase 2.1-2.2 SIMD capsules

---

**End of Document**
