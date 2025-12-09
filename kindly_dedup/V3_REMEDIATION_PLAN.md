# kindly_dedup v3.0.0 - Remediation Action Plan

**Date**: November 19, 2025
**Mission**: Fix compilation errors and validate v3.0.0 before production deployment
**Status**: ACTIVE - IN PROGRESS

---

## Overview

v3.0.0 promises significant improvements (121K docs/sec, O(1) memory, 1B+ documents) but was merged to `HEAD` without compilation verification. This plan addresses the 4 critical errors and establishes a testing protocol.

---

## Phase 1: Fix Compilation Errors (4 Hours)

### Fix #1: Missing `get_hash()` Method on MinHashSignatureCapsule

**File**: `src/universal/pipeline.rs` (line 701)
**Error**: Method not found in `atomic_capsule::probabilistic::MinHashSignatureCapsule`

**Current Code**:
```rust
assert!(signature.get_hash(0) > 0 || signature.get_hash(0) == 0);
```

**Solution A** (Recommended - Use Public API):
```rust
// Check if MinHashSignatureCapsule exposes a public accessor
// Option 1: Direct field access (if public)
// assert!(signature.hashes[0] > 0 || signature.hashes[0] == 0);

// Option 2: Iterator or method that exists
// for hash in signature.iter() {
//     assert!(hash > 0 || hash == 0); // Always true, so simplify
// }

// Option 3: Just test that it's created successfully (simplest)
assert!(!signature.is_empty()); // Validate non-empty signature
```

**Solution B** (Add wrapper method to capsule):
```rust
impl UniversalPipelineTestHelper {
    fn validate_signature_hashes(signature: &MinHashSignatureCapsule) -> bool {
        // Validate signature through public API
        signature.iter().all(|hash| hash >= 0)
    }
}
```

**Time**: 15 minutes | **Priority**: Critical

---

### Fix #2: Type Mismatch in Constructor (u32 vs &integer)

**File**: `src/streaming/pipeline.rs` (line 1010)
**Error**: Expected `u32`, found `&{integer}`

**Current Code**:
```rust
let pipeline = StreamingDedupPipelineCapsule::new(
    "corpus.jsonl",
    size,  // ❌ Expected u32, got &integer
    0.85
)?;
```

**Solution**:
```rust
let pipeline = StreamingDedupPipelineCapsule::new(
    "corpus.jsonl",
    size as u32,  // ✅ Explicit cast to u32
    0.85
)?;
```

**Verification**:
```bash
# Verify function signature
grep -n "fn new(" src/streaming/pipeline.rs | head -5
# Should show: pub fn new(path: &str, num_docs: u32, threshold: f64) -> ...
```

**Time**: 10 minutes | **Priority**: Critical

---

### Fix #3: Missing `PartialEq` for `MmapLshError`

**File**: `src/universal/lsh_bucket.rs` (line 811)
**Error**: Cannot use `==` on `Result<(), MmapLshError>` without `PartialEq`

**Current Code**:
```rust
assert_eq!(header.validate(), Ok(()), "Header validation should pass");
```

**Solution A** (Add derive macro - Recommended):
```rust
// In MmapLshError definition:
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MmapLshError {
    // ... existing variants
}
```

**Solution B** (Use pattern matching - If MmapLshError can't derive PartialEq):
```rust
match header.validate() {
    Ok(()) => {}, // ✅ Validation passed
    Err(e) => panic!("Header validation should pass: {:?}", e),
}

// Or simpler:
assert!(header.validate().is_ok(), "Header validation should pass");
```

**Recommended**: Solution B (more explicit error handling)

**Time**: 15 minutes | **Priority**: Critical

---

### Fix #4: Missing `add_documents()` Method

**File**: `src/streaming_dedup_pipeline.rs` (line 1608)
**Error**: No method named `add_documents` (only `add_document`)

**Current Code**:
```rust
pipeline.add_documents(documents).unwrap();  // ❌ Plural
```

**Solution A** (Use singular method - Simplest):
```rust
for (doc_id, text) in documents {
    pipeline.add_document(doc_id, text)?;
}
```

**Solution B** (Add batch wrapper method):
```rust
impl StreamingDedupPipelineCapsule {
    pub fn add_documents<I>(&mut self, docs: I) -> Result<()>
    where
        I: IntoIterator<Item = (u32, String)>,
    {
        for (id, text) in docs {
            self.add_document(id, &text)?;
        }
        Ok(())
    }
}
```

**Recommended**: Solution A (less code, works with existing API)

**Time**: 10 minutes | **Priority**: Critical

---

## Phase 2: Fix Warnings (2 Hours)

### Priority Warnings to Fix

#### 1. Unused Variables (51 instances)

**Command to find all**:
```bash
cargo build --lib 2>&1 | grep "unused variable" | wc -l
```

**Example Fixes**:
```rust
// Before:
let base_id = 0;  // ❌ Unused

// After:
let _base_id = 0;  // ✅ Intentionally unused
// Or remove entirely if not needed
```

**Time**: 30 minutes

---

#### 2. Unused Mutable Bindings (12 instances)

**Example**:
```rust
// Before:
let mut buffer = [[u16::MAX; 128]; 1000];  // ❌ Not mutated

// After:
let buffer = [[u16::MAX; 128]; 1000];  // ✅ Remove mut
```

**Time**: 20 minutes

---

#### 3. Unused Assignments in Inline Assembly

**File**: `src/protection/tamper_detection.rs:618`

```rust
// Before:
inout("eax") eax,  // ❌ Assigned but never read

// After:
inout("eax") _eax,  // ✅ Underscore prefix
// Or use: out("eax") _ if truly unused output
```

**Time**: 10 minutes

---

## Phase 3: Run Test Suite (3 Hours)

### Tier 1: Compile-Only Test
```bash
cargo build --lib --no-default-features --features "std"
# Expected: 0 errors, 0 warnings
```

**Success Criteria**: ✅ Clean compilation

---

### Tier 2: Unit Tests (Q1-Q7)
```bash
cargo test --lib --no-default-features --features "std"
# Expected: ~100+ tests pass
```

**Success Criteria**: ✅ 95%+ pass rate

---

### Tier 3: Property Tests (Q8-Q14)
```bash
cargo test --features "testing" proptest
# Expected: ~50+ property tests pass
```

**Success Criteria**: ✅ 100% pass rate (properties must hold)

---

### Tier 4: Integration Tests (Q15-Q21)
```bash
cargo test --test '*' --features "std,streaming"
# Expected: ~30+ integration tests pass
```

**Success Criteria**: ✅ 90%+ pass rate

---

### Tier 5: Production Validation (Q22-Q28)

**Benchmark**: 10.2M C4 Corpus (from v2.2 baseline)

```bash
# Build release binary
cargo build --release --no-default-features \
    --features "std,streaming,simd-minhash"

# Run production test
time ./target/release/client_demo --corpus c4_10m.jsonl --validate

# Expected output:
# Throughput: 121K docs/sec (±5%)
# Memory: <7 GB RSS
# Accuracy: ≥90% F1 score
# Duration: ~80-100 seconds
```

**Success Criteria**: ✅ Achieves 115-130K docs/sec on 10.2M corpus

---

## Phase 4: Performance Validation (4 Hours)

### Benchmark Suite

#### Benchmark 1: v2.2 vs v3.0 Throughput Comparison
```bash
# v2.2 baseline (checkout and build)
git stash
git checkout de6417d -- src/streaming/
cargo build --release --features "std"
time ./target/release/... < c4_10m.jsonl > /tmp/v2_2_results.json

# v3.0 (restore and build)
git stash pop
cargo build --release --features "std"
time ./target/release/... < c4_10m.jsonl > /tmp/v3_0_results.json

# Compare
paste <(jq '.docs_per_sec' /tmp/v2_2_results.json) \
      <(jq '.docs_per_sec' /tmp/v3_0_results.json) | \
    awk '{print "v2.2: " $1 " docs/sec, v3.0: " $2 " docs/sec, speedup: " $2/$1 "x"}'
```

**Expected Result**: ✅ 110% speedup (121K vs 110K, 1.1×)

---

#### Benchmark 2: Memory Usage Validation
```bash
# Monitor RSS during execution
# Use MemoryMonitorCapsule from testing framework

cargo build --release --features "std,testing"
./target/release/memory_monitor_test 10000000  # 10M docs

# Expected output:
# Initial: 50 MB
# Peak: 6.5-7.0 GB (v3 should be ≤222 MB for true O(1))
# Final: ~50 MB (cleanup)
```

**Success Criteria**: ✅ Peak memory <7 GB for v3 (or 222 MB if truly O(1))

---

#### Benchmark 3: Accuracy Parity (v2.2 vs v3.0)
```bash
# Generate 10K document ground truth corpus
cargo run --release --bin generate_synthetic_corpus -- \
    --size 10000 --dup-rate 30 \
    --output /tmp/groundtruth.jsonl

# Compute ground truth duplicates
cargo run --release --bin validate_accuracy -- \
    --corpus /tmp/groundtruth.jsonl \
    --output /tmp/ground_truth.json

# Test v2.2
cargo run --release --features "std" -- \
    deduplicate /tmp/groundtruth.jsonl --output /tmp/v2_2_dedup.json

# Test v3.0
cargo run --release --features "std" -- \
    deduplicate /tmp/groundtruth.jsonl --output /tmp/v3_0_dedup.json

# Compare F1 scores
jq '.metrics.f1_score' /tmp/v2_2_dedup.json  # Expected: ≥0.90
jq '.metrics.f1_score' /tmp/v3_0_dedup.json  # Expected: ≥0.90 (parity)
```

**Success Criteria**: ✅ F1 score parity ±2% between v2.2 and v3.0

---

## Phase 5: Documentation & Release (1 Hour)

### Update Release Notes
```markdown
# v3.0.0 - Universal Zero-Copy Pipeline

## Features
- 121K docs/sec throughput (+10% vs v2.2)
- O(1) memory target (222 MB constant)
- 1B+ document capability
- 100% lockfree Chaos architecture

## Breaking Changes
- DedupPipeline (v1.x) deprecated
- StreamingDedupPipeline (v2.x) replaced by UniversalDedupPipeline
- New API: `UniversalDedupPipeline::new(corpus_path, num_docs, threshold)`

## Migration Guide
See docs/MIGRATION_V2_TO_V3.md

## Validation
- ✅ T28 testing: 180+ tests (Q1-Q28)
- ✅ B32 benchmarking: Fair baseline, 1000+ iterations
- ✅ ASSUM safety: 99.99% (all assumptions documented)
- ✅ Production: 10.2M C4 corpus validated
```

---

## Timeline & Resource Allocation

| Phase | Duration | Start | End | Owner |
|-------|----------|-------|-----|-------|
| Phase 1: Fix Errors | 4 hrs | Now | Now+4h | Developer |
| Phase 2: Fix Warnings | 2 hrs | Now+4h | Now+6h | Developer |
| Phase 3: Test Suite | 3 hrs | Now+6h | Now+9h | QA/Developer |
| Phase 4: Performance | 4 hrs | Now+9h | Now+13h | Benchmarking/Developer |
| Phase 5: Release | 1 hr | Now+13h | Now+14h | Release Manager |
| **Total** | **14 hrs** | | | |

**Estimated Completion**: ~2 business days (assuming 7 hours/day)

---

## Rollback Strategy

If v3.0.0 cannot be fixed or validated within timeline:

```bash
# Immediate rollback to v2.2.0 (stable)
git checkout de6417d

# Rebuild and redeploy
cargo build --release --features "std,streaming"
./deploy.sh

# Announce v3.0.0 as "research" / "experimental" branch
# Plan for v3.1.0 with lessons learned
```

**Rollback Time**: ~15 minutes
**Risk**: Low (v2.2 is proven stable)

---

## Success Metrics

### Compilation
- ✅ 0 compilation errors
- ✅ 0 warnings (with `RUSTFLAGS="-D warnings"`)

### Testing
- ✅ T1 (Unit): 95%+ pass rate (~100 tests)
- ✅ T2 (Property): 100% pass rate (~50 tests)
- ✅ T3 (Integration): 90%+ pass rate (~30 tests)
- ✅ T4 (Production): Validates real corpus (10.2M C4)

### Performance
- ✅ Throughput: 115-130K docs/sec (121K ±5%)
- ✅ Memory: Peak <7 GB (v3 should improve vs v2)
- ✅ Accuracy: F1 score ≥0.90 (parity with v2.2)

### Code Quality
- ✅ Framework compliance: UCE34, Chaos, ASSUM, B32, T28, I20
- ✅ No unsafe code in fast paths
- ✅ All assumptions documented with #VERIFY

---

## Appendix: Compilation Command Reference

### Clean Build
```bash
cargo clean
cargo build --lib --no-default-features --features "std"
```

### With Warnings as Errors
```bash
RUSTFLAGS="-D warnings" cargo build --lib
```

### Feature Matrix
```bash
# Minimal (fastest builds)
cargo build --lib --no-default-features --features "std"

# Standard (most features)
cargo build --lib --features "std,streaming,simd-minhash"

# Full (all features - may fail on TPM)
cargo build --lib --all-features
```

### Test Execution
```bash
# Unit tests only
cargo test --lib --no-default-features --features "std"

# With benchmarks
cargo test --lib --features "std,benchmarking"

# Ignored production tests
cargo test --lib --features "std,testing" -- --ignored
```

---

## References

- **V3 Release Notes**: `/home/samuel/Primitives/kindly_dedup/RELEASE_NOTES_v3.0.0.md`
- **CLAUDE.md**: `/home/samuel/Primitives/kindly_dedup/CLAUDE.md`
- **v2.2 Last Commit**: `de6417d` (2025-11-17)
- **v3.0 First Commit**: `df671c2` (2025-11-19)

---

**Plan Created**: 2025-11-19 22:50 UTC
**Status**: Ready for Implementation
**Priority**: CRITICAL - Blocks Production Deployment
