# CapsuleSerialize Migration Checklist

**Total Files**: 27
**Total serde References**: 72
**Estimated Types to Migrate**: 30+

**Status**: ⏳ WAITING FOR serialize_helpers.rs (Agent 1)

---

## Migration Priority by Impact

### CRITICAL (Must Migrate - Core Types)

#### 1. **Benchmarking Module** (6 files)
- [ ] `src/benchmarking/ground_truth.rs` - 3 derives
- [ ] `src/benchmarking/dataset_manager.rs` - 1 derive
- [ ] `src/benchmarking/environment.rs` - 1 derive
- [ ] `src/benchmarking/audit_logger.rs` - 4 derives + custom impl
- [ ] `src/benchmarking/ground_truth_config.rs` - 1 derive
- [ ] `src/audit_events.rs` - Multiple derives

#### 2. **Audit & Protection Module** (4 files)
- [ ] `src/audit/events.rs` - Multiple derives
- [ ] `src/audit/logger.rs` - Multiple derives (Q34 critical)
- [ ] `src/protection/audit.rs` - Audit trail types
- [ ] `src/protection/tamper_detection.rs` - Tamper detection types

#### 3. **Core Pipeline** (3 files)
- [ ] `src/corpus_generation.rs` - Core generation types
- [ ] `src/document_loader.rs` - Document loading types
- [ ] `src/custom_data.rs` - Custom data types

---

### HIGH (Public API - Important for Users)

#### 4. **Server API** (1 file)
- [ ] `src/server.rs` - HTTP API request/response types (3 derives)

#### 5. **CLI & TUI** (3 files)
- [ ] `src/cli/license.rs` - License CLI types
- [ ] `src/tui/components/recent_files.rs` - TUI state types

#### 6. **Format Handlers** (3 files)
- [ ] `src/format/json.rs` - JSON format handling
- [ ] `src/format/jsonl.rs` - JSONL format handling

---

### MEDIUM (Binaries & Utilities)

#### 7. **Standalone Binaries** (6 files)
- [ ] `src/bin/validate_accuracy.rs` - Validation types
- [ ] `src/bin/stress_test_10m.rs` - Stress test types
- [ ] `src/bin/generate_synthetic_corpus.rs` - Corpus generation
- [ ] `src/bin/download_corpus.rs` - Download utilities
- [ ] `src/bin/download_hf_corpus.rs` - HuggingFace types
- [ ] `src/bin/measure_latency.rs` - Latency measurement
- [ ] `src/bin/handlers.rs` - Handler types

#### 8. **Infrastructure** (2 files)
- [ ] `src/license/trial.rs` - Trial license types
- [ ] `src/pdf_export/email_config.rs` - Email configuration

#### 9. **Streaming** (1 file)
- [ ] `src/streaming_corpus_skeleton.rs` - Streaming metadata (1 derive)

---

## Detailed Type Inventory

### A. Benchmarking Types (10+ types)

**ground_truth.rs**:
```
- GroundTruthStrategy (enum, Serialize + Deserialize + Copy)
- DuplicatePair (struct, Serialize + Deserialize)
- GroundTruthResult (struct, Serialize + Deserialize)
```

**ground_truth_config.rs**:
```
- GroundTruthConfig (struct, Serialize + Deserialize)
```

**dataset_manager.rs**:
```
- DatasetConfig (struct, Serialize + Deserialize)
```

**environment.rs**:
```
- EnvironmentSnapshot (struct, Serialize + Deserialize)
```

**audit_logger.rs**:
```
- AuditEvent (struct, Serialize + Deserialize)
- BenchmarkAuditRecord (struct, Serialize + Deserialize)
- PerformanceMetrics (struct, Serialize + Deserialize)
+ Custom serializers with (Serializer, Deserializer)
```

---

### B. Audit & Compliance Types (8+ types)

**audit_events.rs**:
```
- Multiple event types (Serialize + Deserialize)
- Q34 compliance critical
```

**audit/events.rs**:
```
- Event types with custom serialization
```

**audit/logger.rs**:
```
- Audit log entries (Serialize + Deserialize)
- Hash chain integrity types
```

---

### C. API Types (5+ types)

**server.rs**:
```
- DedupRequest (struct, Serialize)
- DedupResponse (struct, Serialize)
- DeduplicationResult (struct, Serialize)
```

---

### D. Format Handlers (6+ types)

**format/json.rs** and **format/jsonl.rs**:
```
- Format-specific request/response types
- Document wrapper types
```

---

### E. CLI/TUI Types (4+ types)

**cli/license.rs**:
```
- License configuration types
```

**tui/components/recent_files.rs**:
```
- State management types
```

---

### F. Binary-Specific Types (7+ types)

**validate_accuracy.rs**:
```
- ValidationConfig (struct, Serialize + Deserialize)
```

**stress_test_10m.rs**:
```
- StressTestConfig (struct, Serialize + Deserialize)
```

**generate_synthetic_corpus.rs**:
```
- CorpusGenerationConfig (struct, Serialize + Deserialize)
```

Others similar patterns in:
- download_corpus.rs
- download_hf_corpus.rs
- measure_latency.rs
- handlers.rs

---

## Migration Implementation Plan

### Phase 1: Setup (When serialize_helpers.rs Ready)
```bash
# 1. Verify serialize_helpers.rs exists
# 2. Run compilation check
cargo check --lib

# 3. Verify atomic_capsule::serialize module access
cargo doc --open  # Check docs
```

### Phase 2: Priority 1 (Critical Benchmarking)
```bash
# Expected: 10+ types, 6 files
# Time: ~1 hour

# 1. Migrate benchmarking/ground_truth.rs (3 derives)
# 2. Migrate benchmarking/audit_logger.rs (4 derives + custom)
# 3. Migrate remaining benchmarking files
# 4. Test: cargo test --lib benchmarking
```

### Phase 3: Priority 2 (Audit & Protection)
```bash
# Expected: 8+ types, 4 files
# Time: ~1 hour

# 1. Migrate audit/events.rs (Q34 critical)
# 2. Migrate audit/logger.rs
# 3. Migrate protection/*
# 4. Test: cargo test --lib audit
```

### Phase 4: Priority 3 (API & Core)
```bash
# Expected: 5+ types, 5 files
# Time: ~1 hour

# 1. Migrate server.rs (HTTP API)
# 2. Migrate format/*.rs (JSON/JSONL)
# 3. Migrate core pipeline types
# 4. Test: cargo test --lib
```

### Phase 5: Priority 4 (Binaries)
```bash
# Expected: 7+ types, 6 files
# Time: ~1 hour

# 1. Migrate binary configs
# 2. Test: cargo build --bins
```

### Phase 6: Verification & Commit
```bash
# Expected: 30 min

cargo test --lib
cargo check --all-targets
git add -A
git commit -m "[TRADE SECRET] refactor(serialize): Migrate 30+ types to CapsuleSerialize (27 files)"
```

---

## Risk Assessment by Category

| Category | Risk | Mitigation |
|----------|------|-----------|
| **Benchmarking** | Medium | Isolated module, roundtrip tests |
| **Audit Trail** | HIGH | Q34 compliance required, determinism tests mandatory |
| **Server API** | Medium | JSON support needed, test HTTP endpoints |
| **Binaries** | Low | Independent, can be tested separately |
| **Format Handlers** | Medium | JSON/JSONL performance maintained |

---

## Testing Checklist

### For Each Migrated Type:
- [ ] Compiles without warnings
- [ ] Roundtrip test passes: serialize → deserialize → equals
- [ ] Determinism test passes: serialize twice → equals
- [ ] (If Q34) Hash chain integrity verified
- [ ] (If public API) Documentation updated

### Full Suite:
```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test '*'

# Benchmarking suite
cargo test --lib benchmarking --release

# All binaries
cargo build --bins --release

# Q34 audit compliance
cargo test --features "audit-trail" --lib
```

---

## Notes for Agent 2

1. **Wait for serialize_helpers.rs** - Don't start until Agent 1 delivers
2. **Prefer derive macro** - Use `#[derive(CapsuleSerialize)]` if available
3. **Manual impl for enums** - May need custom match statements
4. **Maintain JSON support** - server.rs needs JSON API for HTTP
5. **Q34 critical** - audit/logger.rs must maintain hash chain integrity
6. **No breaking changes** - Internal serialization only, public API unchanged

---

## Timeline Estimate

- **Waiting for serialize_helpers.rs**: ⏳ (Agent 1)
- **Phase 1 (Setup)**: 15 minutes
- **Phase 2 (Critical)**: 60 minutes
- **Phase 3 (Audit)**: 60 minutes
- **Phase 4 (API)**: 60 minutes
- **Phase 5 (Binaries)**: 60 minutes
- **Phase 6 (Verification)**: 30 minutes

**Total**: ~4.5 hours (when helpers ready)

---

## Blocking Issues

- [ ] serialize_helpers.rs (Agent 1) - BLOCKING
- [ ] CapsuleSerialize derive macro availability - Check with atomic_capsule docs
- [ ] JSON serialization support in CapsuleSerialize - May need custom impl

---

## Related Issues

- GitHub Issue: kindly_dedup v2.0.0 migration (#XX) - Zero serde dependencies
- CLAUDE.md: line 73 mentions migration details
- Cargo.toml: lines 28, 73-74 document serde removal

---

## Sign-Off

- [ ] Analysis complete
- [ ] serialize_helpers.rs ready (Agent 1)
- [ ] Begin migration (Agent 2)
- [ ] Verify & merge (Agent 3)
