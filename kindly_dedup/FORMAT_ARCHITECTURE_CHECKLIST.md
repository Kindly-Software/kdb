# Format Architecture Implementation Checklist

**Date**: 2025-11-12
**Status**: READY FOR IMPLEMENTATION
**Design Doc**: `/home/samuel/Primitives/kindly_dedup/FORMAT_ARCHITECTURE_UCE34_DESIGN.md` (2,958 lines)
**Summary**: `/home/samuel/Primitives/kindly_dedup/FORMAT_ARCHITECTURE_SUMMARY.md` (282 lines)
**Diagram**: `/home/samuel/Primitives/kindly_dedup/FORMAT_ARCHITECTURE_DIAGRAM.txt` (520 lines)

---

## Pre-Implementation (Day 0)

### Reading & Understanding
- [ ] Read `FORMAT_ARCHITECTURE_UCE34_DESIGN.md` (complete design, 2,958 lines)
- [ ] Read `FORMAT_ARCHITECTURE_SUMMARY.md` (executive summary, 282 lines)
- [ ] Review `FORMAT_ARCHITECTURE_DIAGRAM.txt` (visual architecture)
- [ ] Review `JSON_CAPSULE_VS_SIMD_JSON_ANALYSIS.md` (71% bottleneck proof)
- [ ] Review UCE34 framework (Q1-Q34 systematic discovery)
- [ ] Review Chaos mandate (lockfree, capsule-based)

### Repository Setup
- [ ] Create feature branch: `git checkout -b phase2.5-format-architecture`
- [ ] Backup current state: `cp -r src/custom_data.rs src/custom_data.rs.backup`
- [ ] Create `src/format/` module directory
- [ ] Update `.gitignore` if needed

---

## Week 1: Core Architecture + 3 Formats (18 hours)

### Day 1: Core Trait Definition (2 hours)

#### File: `src/format/mod.rs`
- [ ] Create module structure
- [ ] Define `FormatReaderCapsule` trait (3 methods: stream_documents, format_name, extensions)
- [ ] Add trait documentation (examples, performance characteristics)
- [ ] Add module-level documentation (architecture overview)
- [ ] Export public API (re-export trait, error types)

**Expected Output**: ~200 lines, trait compiles

#### File: `src/format/error.rs`
- [ ] Define `FormatError` enum (Io, JsonParse, CsvParse, Parquet, EmptyFile, UnknownFormat, SchemaMapping)
- [ ] Implement `Display` for `FormatError`
- [ ] Implement `Error` for `FormatError`
- [ ] Implement `From<std::io::Error>` for `FormatError`
- [ ] Add error documentation (causes, solutions)

**Expected Output**: ~80 lines, errors compile

#### File: `src/format/tests.rs`
- [ ] Test trait dispatch (Box<dyn FormatReaderCapsule>)
- [ ] Test error conversion (io::Error → FormatError)
- [ ] Test error Display formatting

**Expected Output**: ~50 lines, tests pass

---

### Day 2: Registry + Progress (2 hours)

#### File: `src/format/registry.rs`
- [ ] Define `FormatRegistryCapsule` struct (HashMap<&'static str, Arc<dyn FormatReaderCapsule>>)
- [ ] Implement `new()` (register all available formats based on feature gates)
- [ ] Implement `auto_detect(path)` (extract extension, lookup in registry)
- [ ] Implement `get_reader(format)` (case-insensitive lookup)
- [ ] Implement `list_formats()` (sorted list of format names)
- [ ] Implement `Default` for `FormatRegistryCapsule`
- [ ] Add registry documentation (examples, feature gates)

**Expected Output**: ~150 lines, registry compiles

#### File: `src/format/progress.rs`
- [ ] Define `ProgressTrackerCapsule` struct (AtomicU64 count)
- [ ] Implement `new()` (initialize to 0)
- [ ] Implement `increment()` (fetch_add(1, Relaxed))
- [ ] Implement `current()` (load(Relaxed))
- [ ] Implement `reset()` (store(0, Relaxed))
- [ ] Implement `Default` for `ProgressTrackerCapsule`
- [ ] Add progress documentation (performance, lockfree guarantees)

**Expected Output**: ~60 lines, progress compiles

#### File: `src/format/tests.rs` (continued)
- [ ] Test registry auto-detection (all supported extensions)
- [ ] Test registry get_reader (case-insensitive)
- [ ] Test registry list_formats (sorted, deduplicated)
- [ ] Test progress increment (1000 iterations)
- [ ] Test progress current/reset
- [ ] Test progress concurrent access (10 threads)

**Expected Output**: +100 lines, tests pass

---

### Day 3: Core Architecture Testing (2 hours)

#### File: `src/format/tests.rs` (T28 Tier 1, Unit Tests)
- [ ] Q1: Trait dispatch compiles and dispatches correctly
- [ ] Q2: Registry auto-detects all formats correctly
- [ ] Q3: Progress tracker increments correctly
- [ ] Q4: Error types convert correctly (io → FormatError)
- [ ] Q5: Error Display formatting is user-friendly
- [ ] Q6: Registry handles unknown formats gracefully
- [ ] Q7: Progress tracker resets correctly

**Expected Output**: 7 unit tests (Q1-Q7), all pass

#### Integration with Cargo.toml
- [ ] Add feature flags: `format-json = ["simd-json"]`
- [ ] Add feature flags: `format-csv = ["csv"]`
- [ ] Add feature flags: `format-parquet = ["parquet", "arrow"]`
- [ ] Add feature flags: `format-all = ["format-json", "format-csv", "format-parquet"]`
- [ ] Verify default = [] (plain text only, zero deps)

#### Update src/lib.rs
- [ ] Add `pub mod format;` (feature-gated)
- [ ] Export public API: `pub use format::{FormatReaderCapsule, FormatError, FormatRegistryCapsule, ProgressTrackerCapsule};`

**Day 1-3 Deliverable**: 690 lines, core architecture complete

---

### Day 4: JSONL Reader Implementation (3 hours)

#### Add simd-json dependency
- [ ] Update `Cargo.toml`: `simd-json = "0.13"` (under `[dependencies.simd-json]`, optional = true)
- [ ] Verify simd-json compiles: `cargo build --features format-json`

#### File: `src/format/jsonl.rs`
- [ ] Define `JsonlReaderCapsule` struct (buffer_size: usize)
- [ ] Implement `new()` (default 64KB buffer)
- [ ] Implement `with_buffer_size(size)` (custom buffer)
- [ ] Implement `Default` for `JsonlReaderCapsule`
- [ ] Implement `FormatReaderCapsule` for `JsonlReaderCapsule`:
  - [ ] `stream_documents()`: BufReader → lines → filter empty → simd_json::from_slice → increment progress
  - [ ] `format_name()`: "JSONL"
  - [ ] `extensions()`: &["jsonl"]
- [ ] Add comprehensive documentation (examples, performance, ASSUM safety)

**Expected Output**: ~150 lines, JSONL reader compiles

#### File: `tests/format_jsonl.rs` (Integration Tests)
- [ ] Test valid JSONL (2 documents)
- [ ] Test empty lines (should skip)
- [ ] Test malformed JSON (should error gracefully)
- [ ] Test progress tracking (matches document count)
- [ ] Test Unicode/UTF-8 (emoji, Chinese characters)
- [ ] Test large file (10K documents, O(1) memory)

**Expected Output**: ~150 lines, integration tests pass

---

### Day 5: JSON Reader + Benchmarking (3 hours)

#### File: `src/format/json.rs`
- [ ] Define `JsonReaderCapsule` struct (buffer_size: usize)
- [ ] Implement `new()` (default 64KB buffer)
- [ ] Implement `with_buffer_size(size)` (custom buffer)
- [ ] Implement `Default` for `JsonReaderCapsule`
- [ ] Implement `FormatReaderCapsule` for `JsonReaderCapsule`:
  - [ ] `stream_documents()`: Read entire file → simd_json::from_slice → update progress
  - [ ] `format_name()`: "JSON"
  - [ ] `extensions()`: &["json"]
- [ ] Add memory limit check (> 1GB → error)
- [ ] Add comprehensive documentation

**Expected Output**: ~120 lines, JSON reader compiles

#### File: `tests/format_json.rs` (Integration Tests)
- [ ] Test valid JSON array (2 documents)
- [ ] Test malformed JSON (should error)
- [ ] Test empty array (should error)
- [ ] Test memory limit (> 1GB → error)
- [ ] Test progress tracking (updates after load)

**Expected Output**: ~100 lines, integration tests pass

#### File: `benches/format_jsonl_vs_serde.rs` (B32 Benchmarking)
- [ ] Benchmark serde_json (baseline, 100K docs)
- [ ] Benchmark simd-json (target, 100K docs)
- [ ] Calculate speedup (mean, 95% CI)
- [ ] Validate 2.31× speedup (B32 EXCEPTIONAL tier)
- [ ] Document results (throughput, latency, classification)

**Expected Output**: ~100 lines, benchmark validates 2.31× speedup

**Day 4-5 Deliverable**: 520 lines, JSONL/JSON complete, 2.31× speedup validated

---

### Day 6: CSV Reader Implementation (3 hours)

#### Add csv dependency
- [ ] Update `Cargo.toml`: `csv = "1.3"` (under `[dependencies.csv]`, optional = true)
- [ ] Verify csv crate compiles: `cargo build --features format-csv`

#### File: `src/format/csv_config.rs`
- [ ] Define `CsvConfig` struct (id_column, text_column, url_column, has_headers, delimiter)
- [ ] Implement `Default` for `CsvConfig` (id=0, text=1, url=None, headers=false, delimiter=',')
- [ ] Add configuration documentation (examples, schema mapping)

**Expected Output**: ~80 lines, config struct compiles

#### File: `src/format/csv.rs`
- [ ] Define `CsvReaderCapsule` struct (config: CsvConfig)
- [ ] Implement `new(config)` (custom configuration)
- [ ] Implement `Default` for `CsvReaderCapsule` (default config)
- [ ] Implement `FormatReaderCapsule` for `CsvReaderCapsule`:
  - [ ] `stream_documents()`: csv::ReaderBuilder → records → extract columns → parse ID → increment progress
  - [ ] `format_name()`: "CSV"
  - [ ] `extensions()`: &["csv", "tsv"]
- [ ] Handle schema mapping errors (missing columns)
- [ ] Handle ID parsing errors (invalid usize)
- [ ] Add comprehensive documentation

**Expected Output**: ~180 lines, CSV reader compiles

#### File: `tests/format_csv.rs` (Integration Tests)
- [ ] Test valid CSV (with headers, 2 rows)
- [ ] Test valid CSV (no headers, 2 rows)
- [ ] Test schema mapping (custom column order)
- [ ] Test TSV (tab delimiter)
- [ ] Test missing columns (should error)
- [ ] Test invalid ID (should error)
- [ ] Test progress tracking (matches record count)
- [ ] Test Unicode/UTF-8

**Expected Output**: ~200 lines, integration tests pass

---

### Day 7: CSV Benchmarking + Week 1 Wrap-Up (3 hours)

#### File: `benches/format_csv.rs` (B32 Benchmarking)
- [ ] Benchmark csv crate (baseline, 100K records)
- [ ] Benchmark CsvReaderCapsule (wrapper overhead)
- [ ] Validate < 5% overhead (B32 validation)
- [ ] Document results (throughput 5-10 MB/s)

**Expected Output**: ~80 lines, benchmark validates no regression

#### Update Registry
- [ ] Register JsonlReaderCapsule (feature = "format-json")
- [ ] Register JsonReaderCapsule (feature = "format-json")
- [ ] Register CsvReaderCapsule (feature = "format-csv")
- [ ] Verify auto-detection works for all formats
- [ ] Verify list_formats() returns ["CSV", "JSON", "JSONL"] (sorted)

#### Week 1 Testing (T28 Tier 2, Property Tests)
- [ ] Q8: Document count preserved (JSONL, JSON, CSV)
- [ ] Q9: Document content preserved (JSONL, JSON, CSV)
- [ ] Q10: Progress matches count (all formats)
- [ ] Q11: Streaming consumes O(1) memory (100K docs, <10MB increase)
- [ ] Q12: CSV schema mapping handles all column orders
- [ ] Q13: Registry handles all extensions (jsonl, json, csv, tsv)
- [ ] Q14: Trait dispatch overhead <5ns (benchmark)

**Expected Output**: 7 property tests (Q8-Q14), all pass

**Day 6-7 Deliverable**: 540 lines, CSV complete

**Week 1 TOTAL**: 1,750 lines, 3 formats (JSONL/JSON/CSV), 18 hours

---

## Week 2: Plain Text + Integration (10 hours)

### Day 1: Plain Text Reader (2 hours)

#### File: `src/format/plaintext.rs`
- [ ] Define `PlainTextReaderCapsule` struct (buffer_size: usize)
- [ ] Implement `new()` (default 64KB buffer)
- [ ] Implement `with_buffer_size(size)` (custom buffer)
- [ ] Implement `Default` for `PlainTextReaderCapsule`
- [ ] Implement `FormatReaderCapsule` for `PlainTextReaderCapsule`:
  - [ ] `stream_documents()`: BufReader → lines → skip empty → auto-increment ID → increment progress
  - [ ] `format_name()`: "Plain Text"
  - [ ] `extensions()`: &["txt"]
- [ ] Add comprehensive documentation

**Expected Output**: ~100 lines, plain text reader compiles

#### File: `tests/format_plaintext.rs` (Integration Tests)
- [ ] Test valid plain text (3 lines)
- [ ] Test empty lines (should skip)
- [ ] Test Unicode/UTF-8
- [ ] Test progress tracking (matches line count)
- [ ] Test large file (10K lines)

**Expected Output**: ~120 lines, integration tests pass

#### File: `benches/format_plaintext.rs` (B32 Benchmarking)
- [ ] Benchmark BufReader (baseline)
- [ ] Benchmark PlainTextReaderCapsule (wrapper overhead)
- [ ] Validate < 1% overhead (near zero)
- [ ] Document results (I/O bound, 10-50 MB/s)

**Expected Output**: ~60 lines, benchmark validates no regression

**Day 1 Deliverable**: 280 lines, plain text complete

---

### Day 2: Integration Helpers (2 hours)

#### File: `src/format/integration.rs`
- [ ] Define `load_corpus()` function (auto-detect format, stream documents, add to pipeline)
- [ ] Define `load_corpus_with_format()` function (explicit format, stream documents, add to pipeline)
- [ ] Handle stdin (path = "-")
- [ ] Add comprehensive documentation (examples, error handling)

**Expected Output**: ~150 lines, integration helpers compile

#### Update `src/lib.rs`
- [ ] Export `load_corpus` function
- [ ] Export `load_corpus_with_format` function
- [ ] Export all format-related types (feature-gated)

#### Examples
- [ ] Create `examples/load_jsonl.rs` (load JSONL corpus)
- [ ] Create `examples/load_csv.rs` (load CSV corpus with schema mapping)
- [ ] Create `examples/load_auto.rs` (auto-detect format)
- [ ] Create `examples/load_stdin.rs` (load from stdin)
- [ ] Create `examples/load_compressed.rs` (load .jsonl.gz)

**Expected Output**: 5 examples (~150 lines total), all run successfully

**Day 2 Deliverable**: 300 lines, integration helpers complete

---

### Day 3: Deprecation + Documentation (2 hours)

#### Deprecate Old API
- [ ] Mark `custom_data::load_custom_corpus()` as deprecated
- [ ] Mark `custom_data::load_jsonl()` as deprecated
- [ ] Mark `custom_data::load_json()` as deprecated
- [ ] Mark `custom_data::load_plaintext()` as deprecated
- [ ] Add deprecation warnings (use `load_corpus()` instead)
- [ ] Update all internal uses to new API

#### Update Documentation
- [ ] Update `README.md` (new format support section)
- [ ] Update module-level docs (`src/format/mod.rs`)
- [ ] Update `CHANGELOG.md` (version 1.14.0 or similar)
- [ ] Add migration guide (old API → new API)

**Expected Output**: Documentation complete, deprecation warnings compile

---

### Day 4: Testing Strategy (T28 Tier 3, Integration) (2 hours)

#### File: `tests/format_integration.rs`
- [ ] Q15: DedupPipeline integration (load → dedup → clusters)
- [ ] Q16: Large file (10M docs, streaming memory)
- [ ] Q17: Compressed file (.jsonl.gz, transparent decompression)
- [ ] Q18: Network stream (stdin, HTTP simulation)
- [ ] Q19: Concurrent readers (10 threads, same file)
- [ ] Q20: Malformed recovery (skip bad lines, continue)
- [ ] Q21: Document order preserved (JSONL, CSV, TXT)

**Expected Output**: 7 integration tests (Q15-Q21), all pass

---

### Day 5: Testing Strategy (T28 Tier 4, Production) + Wrap-Up (2 hours)

#### File: `tests/format_production.rs`
- [ ] Q22: Edge cases (empty file, single doc, 10M docs)
- [ ] Q23: Unicode/UTF-8 (emoji, Chinese, right-to-left)
- [ ] Q24: Memory pressure (low RAM simulation)
- [ ] Q25: I/O errors (disk full, network timeout)
- [ ] Q26: Security (path traversal, symlinks)
- [ ] Q27: Performance targets (60K docs/sec for JSONL)
- [ ] Q28: Backward compatibility (old API still works)

**Expected Output**: 7 production tests (Q22-Q28), all pass

#### Week 2 Final Validation
- [ ] All 28 tests pass (T28 complete)
- [ ] All benchmarks pass (B32 validated)
- [ ] All examples run (5 examples)
- [ ] Documentation complete (README, CHANGELOG, migration guide)
- [ ] Deprecation warnings compile (no errors)

**Day 3-5 Deliverable**: T28 complete (28 tests), documentation complete

**Week 2 TOTAL**: 580 lines, 4 formats (TXT added), 10 hours

---

## Week 3: Parquet (Optional, 15 hours)

### Status: DEFERRED (add based on user demand)

**Reason**: Parquet adds 3MB dependencies (parquet + arrow), complex schema mapping, and is not critical for v1.0.

**Add When**:
- User requests Parquet support
- Need 10-20× loading speedup (columnar format)
- Have large-scale deployment (>10M docs)

**Estimated Effort**:
- 1 week (15 hours)
- 550 lines (ParquetReaderCapsule, tests, benchmarks)

---

## Post-Implementation

### Cleanup
- [ ] Remove old `custom_data.rs` (backup preserved)
- [ ] Remove deprecated warnings (version N+1)
- [ ] Update `CLAUDE.md` (new format architecture section)
- [ ] Tag release: `git tag v1.14.0`

### Announcement
- [ ] Update README.md (format support announcement)
- [ ] Write blog post (optional, technical deep-dive)
- [ ] Update docs.rs documentation

### Validation
- [ ] All 28 tests pass (T28)
- [ ] All benchmarks pass (B32, 2.31× validated)
- [ ] All examples run (5 examples)
- [ ] Zero clippy warnings
- [ ] Zero unsafe code in `src/format/` (ASSUM 99.99% safe)

---

## Framework Compliance Checklist

### UCE34 (Q1-Q34) ✅
- [ ] Q1-Q9: Problem understanding (extensibility, Chaos, performance)
- [ ] Q10: Tier selection (T5 Streaming + T2 SIMD + T1 Atomic)
- [ ] Q11-Q12: Rust transform (100% safe, nightly simd-json)
- [ ] Q13-Q29: Implementation (trait, readers, integration)
- [ ] Q30-Q32: Performance validation (B32, 2.31× proven)
- [ ] Q33: Verification (#[derive(ComputationalCapsule)] where applicable)
- [ ] Q34: Auditability (progress tracking, error logging)

### Chaos (Computational Capsule Architecture) ✅
- [ ] 100% lockfree (AtomicU64 progress tracking, no mutex)
- [ ] T5 Streaming (O(1) memory, Iterator-based)
- [ ] T2 SIMD (simd-json, 2.31× speedup)
- [ ] T1 Atomic (progress tracking, <5ns)
- [ ] Cache alignment (ProgressTrackerCapsule fits in single cache line)

### B32 (Fair Benchmarking) ✅
- [ ] Fair baselines (serde_json, csv, parquet crates)
- [ ] 95% CI (1000+ iterations)
- [ ] Same hardware (AMD 6900HX)
- [ ] Same compiler (rustc 1.82 nightly)
- [ ] Reproducibility (seed RNG, warm cache)
- [ ] Speedup classification (2.31× = EXCEPTIONAL)

### T28 (Comprehensive Testing) ✅
- [ ] Tier 1: Unit tests (Q1-Q7)
- [ ] Tier 2: Property tests (Q8-Q14)
- [ ] Tier 3: Integration tests (Q15-Q21)
- [ ] Tier 4: Production tests (Q22-Q28)
- [ ] Total: 28 tests, all pass

### ASSUM (Safety Assumptions) ✅
- [ ] 99.99% safe (zero unsafe in capsule wrappers)
- [ ] All assumptions documented (#ASSUME + #VERIFY)
- [ ] simd-json unsafe audited (community-verified)

### I20 (Integration Validation) ✅
- [ ] 20/20 integration questions answered
- [ ] DedupPipeline unchanged (zero breaking changes)
- [ ] Backward compatibility maintained

---

## Success Criteria

### Functional ✅
- [ ] Load JSONL/JSON/CSV/TXT with 100% correctness
- [ ] DedupPipeline unchanged (zero integration effort)
- [ ] Add new format in <200 lines

### Performance ✅
- [ ] JSON: 2.31× speedup (proven, B32 validated)
- [ ] CSV: Match csv crate (5-10 MB/s)
- [ ] TXT: Near I/O bound (10-50 MB/s)
- [ ] Trait dispatch: <5ns overhead (negligible)

### Quality ✅
- [ ] T28: 28 comprehensive tests (all pass)
- [ ] ASSUM: 99.99% safe (zero unsafe in wrappers)
- [ ] B32: Fair baselines, 95% CI
- [ ] Chaos: 100% lockfree

### Strategic ✅
- [ ] Extensible architecture (easy to add formats)
- [ ] Novel IP (capsule abstraction pattern)
- [ ] Long-term value (reusable foundation)

---

## Completion

**Estimated Timeline**: 2 weeks (28 hours)
**Estimated Lines**: 2,330 lines (Week 1 + Week 2, excluding Parquet)
**Formats Supported**: 4 (JSONL, JSON, CSV, Plain Text)

**Deliverables**:
- [ ] Core architecture (690 lines)
- [ ] JSONL/JSON readers (520 lines)
- [ ] CSV reader (540 lines)
- [ ] Plain text reader (280 lines)
- [ ] Integration helpers (300 lines)
- [ ] 28 comprehensive tests (T28)
- [ ] Benchmarks (B32, 2.31× validated)
- [ ] Documentation (README, CHANGELOG, migration guide, 5 examples)

**Sign-Off**:
- [ ] All checklists complete
- [ ] All tests pass
- [ ] All benchmarks validate
- [ ] Zero clippy warnings
- [ ] Documentation complete
- [ ] Ready for merge to main

---

**END OF CHECKLIST**
