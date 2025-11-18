# Incorrect In-Memory Validation (LEGACY)

**Date**: 2025-11-17
**Status**: DEPRECATED - Moved to legacy due to incorrect architecture testing

## What Was Wrong

These validation reports tested the **WRONG pipeline architecture**:
- Used: `DedupPipeline` (in-memory, T10 Probabilistic only)
- Should use: `PersistentDedupPipeline` (T9 mmap + T10, default enabled)

## Impact

### Incorrect RAM Projections
The agent projected **linear in-memory scaling**:
- 354K docs: 996 MB RAM (measured)
- 1M docs: 2.8 GB RAM (projected)
- 10M docs: **28 GB RAM** (projected)
- 100M docs: **280 GB RAM** (projected) ❌ WRONG!

### Correct Architecture (T9 Persistent)
From validated persistent-dedup benchmarks:
- 10M docs: **3.5 GB RAM** + 52 GB disk
- 100M docs: **35 GB RAM** + 520 GB disk (NOT 280 GB RAM!)
- 1B docs: **~50 GB RAM** + 5.2 TB disk

**Error magnitude**: 8× overestimate of RAM requirements (280 GB vs 35 GB)

## Why This Happened

1. **c4_corpus_validation.rs** used `DedupPipeline::new()` (in-memory)
2. Agent didn't check which pipeline was being tested
3. Linear extrapolation from in-memory measurements
4. Ignored that `persistent-dedup` is enabled by default in Cargo.toml

## Lesson Learned

**B32 Validation Principle**: Always verify WHICH implementation you're benchmarking.

- ❌ Assume default features are active
- ✅ Explicitly test the production configuration
- ✅ Check Cargo.toml default features BEFORE validation
- ✅ Document architecture tier (T9 vs T10) in every benchmark

## Files in This Legacy Directory

1. **ENTERPRISE_VALIDATION_1M_REPORT.md** (17 KB)
   - Technical analysis with INCORRECT 280 GB RAM claim
   - Correct throughput (98K docs/sec still valid)
   - Wrong memory architecture

2. **EXECUTIVE_SUMMARY_354K_VALIDATION.md** (19 KB)
   - Executive summary with INCORRECT scaling projections
   - Correct performance numbers (throughput, latency)
   - Wrong enterprise tier recommendations

3. **SCALING_ANALYSIS_SUMMARY.txt** (7 KB)
   - Metrics table with linear RAM projections
   - Throughput analysis is still valid
   - Memory efficiency claims are WRONG

## What's Still Valid

From these reports:
- ✅ Throughput: 98,638 docs/sec (measured correctly)
- ✅ Latency: 10.14 µs/doc (measured correctly)
- ✅ Superlinear scaling: 12.7× improvement (still valid finding)
- ✅ Intel 155H vs AMD 6900HX: 1.6× advantage (correct)
- ❌ RAM projections: ALL WRONG (used in-memory not persistent)

## Correct Validation

See new validation files:
- `examples/c4_persistent_validation.rs` - Uses PersistentDedupPipeline
- `docs/enterprise/PERSISTENT_VALIDATION_REPORT.md` - Correct T9 mmap architecture
- Expected RAM: ~5 GB for 354K docs (vs 996 MB in-memory)

## Enterprise Impact

**CRITICAL**: These incorrect projections would have:
- Overestimated RAM requirements 8×
- Made 100M tier look infeasible (280 GB vs commodity 64 GB servers)
- Undermined competitive advantage (we claim 93% memory reduction!)

**User catch saved the pitch!** Big corporate clients would have rejected based on 280 GB RAM claim.
