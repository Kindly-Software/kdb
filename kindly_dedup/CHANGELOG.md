# Changelog - kindly_dedup

All notable changes to kindly_dedup are documented in this file.

## [3.1.0] - 2025-11-26

### Added
- **License Generator Binary** (`src/bin/generate_license.rs`): HMAC-SHA256 license key generation with tier-based expiration
- **Commercial Tier Limiter** (`src/protection/commercial_limiter.rs`): T1 Atomic capsule for Demo/Basic/Pro/Enterprise tier enforcement
- **Consolidated Documentation**:
  - USER_GUIDE.md (comprehensive user documentation)
  - LICENSING.md (commercial licensing model)
  - DEPLOYMENT.md (production deployment guide)
  - API_REFERENCE.md (complete API documentation)
- **Test Common Utilities** (`tests/common/mod.rs`): 15 shared test utilities reducing test code duplication
- **Integration Test Suite** (`tests/integration/`): End-to-end, protection, GPU fallback, stress tests

### Fixed
- GPU initialization timeout (5-second limit with graceful CPU fallback)
- Background monitor SIGSEGV (`shutdown_and_join()` pattern replacing `join().unwrap()`)
- ParallelDedupPipeline deprecation warning (runtime notification instead of silencing)
- Compiler warnings reduced from 1370 to <100 (97.7% reduction)
- Test isolation via `tests/common/mod.rs` (eliminates cross-test interference)

### Changed
- Protection system fully wired to UniversalDedupPipeline (4-layer META_CAPSULE integration)
- DedupPipeline, ParallelDedupPipeline, PersistentDedupPipeline marked deprecated (v3.0+)
- UniversalDedupPipeline is now the RECOMMENDED default (replaces all legacy pipelines)
- Documentation structure reorganized (4 user-facing guides + archived developer docs)

### Framework Compliance
- **UCE34**: Q1-Q34 complete (tier selection, systematic discovery)
- **Chaos**: 100% lockfree (99.9% with documented Mutex<File> exception in transaction log)
- **ASSUM**: 99.99% safe (all assumptions documented and verified)
- **B32**: 60K docs/sec validated (single-threaded baseline)
- **T28**: 1,272+ tests passing (unit/property/integration/production/determinism)
- **I20**: 20/20 integration validated (zero breaking changes)

### Performance
- Single-threaded throughput: 60,000 docs/sec (VALIDATED)
- Per-document latency: 16.7 µs end-to-end
- Speedup vs Python datasketch: 38× (EXCEPTIONAL)
- Accuracy: ≥90% F1 score (duplicate detection)

### Known Issues
- ParallelDedupPipeline requires redesign (12.8× slower than sequential, NOT production-ready)
- GPU acceleration available but requires specific hardware (Vulkan/Metal/DX12)
- Interactive TUI requires `interactive` feature flag

### Migration Notes
- Users on v2.x should upgrade to UniversalDedupPipeline
- Legacy pipelines will be removed in v4.0 (February 2026)
- See `docs/MIGRATION_v3.md` for detailed migration guide

## [3.0.0] - 2025-11-20

### Added
- Adaptive GPU/CPU Pipeline (T6 Mixed + T7 Heterogeneous)
- 4,756 LOC in `src/adaptive/` (CrossoverDetectorCapsule, WorkStealingCapsule, MemoryBudgetCapsule, AdaptivePipelineCapsule)
- 142 adaptive tests (106 inline + 36 T28)
- EMA-based crossover detection with Q16.16 fixed-point math
- 5-phase state machine coordinator for GPU/CPU transitions

### Performance
- CrossoverDetector update: <100ns (10M+ ops/sec)
- WorkStealing steal_work: <50ns (20M+ ops/sec)
- MemoryBudget allocate: <20ns (50M+ ops/sec)
- AdaptivePipeline record_batch: <200ns (5M+ batches/sec)

## [2.4.0] - 2025-11-24

### Added
- GPU Acceleration (wgpu T7 Heterogeneous tier)
- HybridDedupPipeline implementation (CPU-GPU coordination)
- 62 GPU tests (Phase GPU-1C complete)
- Backend priority: Vulkan > Metal > DX12 > WebGPU > CPU fallback

### Performance Targets
- iGPU (Ryzen): 150K docs/sec (2× baseline)
- GTX 1650: 300K docs/sec (4× baseline)
- RTX 3060: 500K docs/sec (7× baseline)
- RTX 4090: 1M docs/sec (14× baseline)

## [2.1.0] - 2025-11-18

### MAJOR: Complete serde removal - 100% atomic_capsule serialization

**Migration to pure atomic_capsule serialization**:
- Removed: `serde`, `serde_json`, `serde_derive` dependencies
- Removed: ~30 transitive serde ecosystem dependencies
- Added: 35 atomic_capsule serialization capsules (T0-T2 tiers)
- Dependency reduction: 43 → 25 direct deps (42% reduction)

### Serialization Architecture

**Format Implementations**:
- **JsonWriterCapsule** (T1 Atomic): Fast JSON output buffer coordination
- **JsonParserCapsule** (T5 Streaming): Incremental JSON parsing, O(1) per token
- **BincodeWriterCapsule** (T1 Atomic): Deterministic binary serialization
- **CsvWriterCapsule** (T5 Streaming): Row-based CSV export
- **HexEncoderCapsule** (T2 SIMD): 4× hex encoding speedup (portable_simd)
- **HexDecoderCapsule** (T2 SIMD): 4× hex decoding speedup (portable_simd)

**Derive Macro**:
- `#[derive(CapsuleSerialize, CapsuleDeserialize)]` for all types
- Field attributes: `#[capsule(default)]`, `#[capsule(skip)]`, `#[capsule(with = "hex")]`
- Enum support: All variant types (unit, tuple, struct)
- Zero boilerplate vs serde (same ergonomics, better performance)

### Performance Improvements

- **Zero-copy deserialization**: 10-50× speedup
- **SIMD hex encoding**: 4× speedup for 32-byte hashes
- **Deterministic serialization**: 100% reproducible for Q34 audit trails
- **Compile-time overhead**: <20ms per type (negligible)
- **JSON output**: 1.5-4× faster than serde_json baseline

### Breaking Changes

**API Level**: NONE
- HTTP JSON API output format preserved exactly (Q34 compliance)
- Audit trail format unchanged (hash chain integrity maintained)
- JSONL corpus streaming format unchanged
- CLI interfaces unchanged (serialization is internal)

**Internal Only**:
- Binary serialization format changed (deterministic encoding)
- Custom serializers moved to `CapsuleSerialize` (not serde adapters)
- Error types changed from `serde::error::Error` to `CapsuleError`

### Framework Compliance

✅ **UCE34**: Q10 T0+T1+T2 tier selection, Q34 audit trails preserved
✅ **Chaos**: 100% lockfree (zero mutex in serialization layer)
✅ **ASSUM**: 99.99% safe (zero unsafe in hot paths, SIMD verified)
✅ **B32**: Fair baseline comparison (serde vs atomic_capsule), 1.5-4× validated
✅ **T28**: 280+ tests (unit/property/integration/production)
✅ **I20**: 20/20 integration validation (Big Bang deployment)

### Testing

**Unit Tests** (80 tests):
- Primitive serialization (u8-u64, bool, f32, f64, String)
- Struct composition (nested types, Option, Vec)
- Enum variants (unit, tuple, struct)

**Property Tests** (60 tests):
- Roundtrip preservation (serialize → deserialize → equal original)
- Determinism verification (same input → identical JSON bytes)
- Error handling (invalid JSON, type mismatches, bounds violations)

**Integration Tests** (80 tests):
- Production types (BenchmarkAuditEntry, EnvironmentInfo, etc.)
- Q34 audit trail integrity (hash chain verification)
- HTTP API compatibility (JSON output format validation)
- Large corpus processing (10K, 100K, 1M document JSONL)

**Production Tests** (60 tests):
- Audit trail verification (hash chain integrity after deserialization)
- HTTP API exact match (capsule JSON vs serde_json byte-for-byte)
- Performance vs serde (1.5× minimum speedup, 4× SIMD operations)
- Backward compatibility (legacy audit trail parsing)

### Files Changed

**Removed**:
- Serde dependency from Cargo.toml (was: ~30 transitive deps)

**Added**:
- 35 new capsule implementations in atomic_capsule
- `src/serialize_helpers.rs` (150 lines): Utility functions
- 280+ unit/property/integration/production tests

**Modified** (27 files):
- All types: `#[derive(Serialize, Deserialize)]` → `#[derive(CapsuleSerialize, CapsuleDeserialize)]`
- 66 serialization sites: `serde_json::to_*` → `obj.to_json()`
- 76 deserialization sites: `serde_json::from_*` → `Type::from_json()`
- Error handling: New `CapsuleError` type with rich context

### Dependencies

**Removed** (~30 dependencies):
```
serde, serde_json, serde_derive
indexmap, itoa, ryu, dtoa, (and 25+ more transitive deps)
```

**Added** (0 new external dependencies):
```
# All serialization now uses atomic_capsule (path dependency)
atomic_capsule = { path = "../atomic_capsule", features = ["serialize", "serialize-simd"] }
```

**Net Effect**: 43 → 25 direct deps (42% reduction)

### Security & Compliance

- **Q34 Auditability**: Hash-chained audit trails preserved exactly
- **Determinism**: Field ordering locked in, no randomization
- **Tamper Detection**: JSON modification breaks hash chain
- **SOX/SOC2/GDPR/HIPAA**: Audit trail format unchanged (full compliance)

### Migration Guide

**For Users**: NO CHANGES REQUIRED
- Library API unchanged (DedupPipeline, StreamingDedupPipeline work identically)
- HTTP JSON output unchanged (clients see same format)
- Audit trails unchanged (compliance maintained)

**For Developers** (if using kindly_dedup as a library):
- Serialization is internal only
- If custom types, use `#[derive(CapsuleSerialize, CapsuleDeserialize)]` (same as before)
- Error handling: Catch `anyhow::Error` (serde_json wrapped in thiserror)

### Known Issues

None. All tests passing, full compatibility validated.

### Future Work

- **v2.2**: Persistent streaming (T9+T10, mmap serialization)
- **v2.3**: Distributed serialization (T8 Network tier)
- **v3.0**: GPU acceleration (T7 Heterogeneous)

---

## [2.0.0] - 2025-11-15

### MAJOR: T5 Streaming Pipeline - 14.46× breakthrough performance

See `docs/CHANGELOG_v2.0.0.md` for complete v2.0 release notes.

**Summary**:
- StreamingDedupPipeline: 575,491 docs/sec (vs 39,788 in v1.14)
- 5-stage lockfree pipeline (Ingest → Tokenize → MinHash → LSH → Verify)
- 100% backward compatible (opt-in upgrade)
- 99.99% ASSUM safe, 100% Chaos lockfree

### Framework Compliance

✅ **UCE34**: Q10 T5 Streaming tier selection, Q34 audit trails
✅ **Chaos**: 100% lockfree (zero mutex/RwLock)
✅ **ASSUM**: 99.99% safe (verified stress tests)
✅ **B32**: EXCEPTIONAL tier validated (14.46× vs v1.14)
✅ **T28**: 11/11 tests passing
✅ **I20**: 20/20 integration validated

---

## [1.14.0] - 2025-10-15

### SIMD Text Hashing + Bloom K=3 Optimization (9.3× compound speedup)

See `docs/CHANGELOG_v1.14.0.md` for complete v1.14 release notes.

**Performance**:
- 60,000 docs/sec single-threaded (vs 1,600 Python baseline)
- 38× speedup vs Python datasketch
- 98.89% F1 accuracy maintained

**Features**:
- SIMD MinHash: 7.1× vectorized signatures (portable_simd)
- Bloom pre-filter: 2-10× on duplicate-heavy corpora
- Runtime CPU dispatch: <0.1% overhead
- Adaptive LSH scaling: 12.6× @ 10M docs

---

## [1.0.0] - 2025-09-01

### Initial Release

**Features**:
- MinHash + LSH deduplication
- Python datasketch compatibility (API, output format)
- Deterministic Q16.16 fixed-point arithmetic
- Bloom pre-filtering
- Audit trail logging (Q34 ready)

**Performance**:
- 60K docs/sec (38× vs Python)
- 90%+ F1 score accuracy
- <4GB RAM for 1M documents

---

## Format Legend

- `[MAJOR.MINOR.PATCH]`: Semantic versioning
- **MAJOR**: Architecture changes, breaking changes
- **MINOR**: Features, optimizations, non-breaking additions
- **PATCH**: Bug fixes, documentation

## Framework Abbreviations

- **UCE34**: Systematic discovery (Q1-Q34 questions)
- **Chaos**: Computational Capsule Architecture
- **ASSUM**: Unsafe assumptions framework (99.99% safety)
- **B32**: Fair benchmarking (95% CI, 1000+ iterations)
- **T28**: Comprehensive testing (4 tiers: Unit/Property/Integration/Production)
- **I20**: Integration validation (20 questions per capsule)

---

**Current Version**: v2.1.0 (2025-11-18)
**Status**: PRODUCTION READY
**Maintainer**: Samuel
