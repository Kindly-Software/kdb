# Dead Serde Code Cleanup Checklist

## Files with Serde Code (Not Compiled in Production)

Total files: 26

### Category 1: Format Readers (Feature-Gated)
These files are behind `format-json` feature which is NOT enabled by default:
- [ ] `src/format/jsonl.rs` - Replace with atomic_capsule::serialize::JsonReaderCapsule
- [ ] `src/format/json.rs` - Replace with atomic_capsule::serialize::JsonReaderCapsule

**Action**: Migrate to atomic_capsule JSON parsing OR delete if unused.

### Category 2: Optional Binaries (Not Built by Default)
These binaries require specific features:
- [ ] `src/bin/download_hf_corpus.rs` - Requires `hf-datasets` feature
- [ ] `src/bin/download_corpus.rs` - Requires `download-tools` feature
- [ ] `src/bin/generate_synthetic_corpus.rs` - Requires `download-tools` feature
- [ ] `src/bin/measure_latency.rs` - Requires `download-tools` feature
- [ ] `src/bin/stress_test_10m.rs` - Requires `download-tools` feature
- [ ] `src/bin/validate_accuracy.rs` - Requires `download-tools` feature
- [ ] `src/bin/handlers.rs` - Uses serde_json::json! macro

**Action**: Migrate to atomic_capsule serialization OR document as legacy tools.

### Category 3: Disabled Binaries
These files are in `src/bin_disabled/`:
- [ ] `src/bin_disabled/handlers_new.rs` - Uses serde

**Action**: DELETE (already disabled, safe to remove).

### Category 4: Benchmarking Infrastructure
These files support benchmarking (dev-only):
- [ ] `src/benchmarking/audit_logger.rs`
- [ ] `src/benchmarking/dataset_manager.rs`
- [ ] `src/benchmarking/environment.rs`
- [ ] `src/benchmarking/ground_truth.rs`
- [ ] `src/benchmarking/ground_truth_config.rs`

**Action**: Migrate to atomic_capsule OR document as benchmarking support (acceptable).

### Category 5: Support Modules
These modules support optional features:
- [ ] `src/corpus_generation.rs` - Synthetic corpus generation
- [ ] `src/custom_data.rs` - Custom data structures
- [ ] `src/document_loader.rs` - Document loading utilities
- [ ] `src/streaming_corpus_skeleton.rs` - Streaming corpus scaffolding
- [ ] `src/server.rs` - HTTP server (optional)
- [ ] `src/license/trial.rs` - Trial license management

**Action**: Migrate to atomic_capsule serialization.

### Category 6: Audit/Protection Modules
These modules use serde for configuration/events:
- [ ] `src/audit/events.rs` - Audit event serialization
- [ ] `src/audit_events.rs` - Audit events
- [ ] `src/protection/tamper_detection.rs` - Tamper detection logs
- [ ] `src/pdf_export/email_config.rs` - Email configuration

**Action**: Migrate to atomic_capsule serialization for Q34 compliance.

### Category 7: TUI Components
- [ ] `src/tui/components/recent_files.rs` - Recent files tracking

**Action**: Migrate to atomic_capsule serialization.

## Migration Strategy

### Phase 1: Delete Dead Code (Safe)
Delete files in `src/bin_disabled/`:
```bash
rm -rf src/bin_disabled/
```

### Phase 2: Migrate Core Infrastructure
Priority order:
1. `src/audit_events.rs` - Q34 compliance critical
2. `src/audit/events.rs` - Q34 compliance critical
3. `src/benchmarking/*.rs` - Benchmarking infrastructure (5 files)
4. `src/format/*.rs` - Format readers (2 files)

### Phase 3: Migrate Optional Features
Lower priority (not used in production):
1. `src/bin/*.rs` - Optional binaries (7 files)
2. `src/license/trial.rs` - Trial management
3. `src/server.rs` - HTTP server
4. Support modules (6 files)

### Phase 4: Migrate TUI Components
Lowest priority:
1. `src/tui/components/recent_files.rs` - TUI state

## Verification After Cleanup

After migrating/deleting files:
```bash
# Should return ZERO
grep -r "use serde" src/ --include="*.rs" | wc -l

# Should return ZERO
grep -r "serde_json::" src/ --include="*.rs" | wc -l

# Should return ZERO
grep -r "bincode::" src/ --include="*.rs" | wc -l
```

## Timeline Estimate

- Phase 1 (Delete): 5 minutes ✅ Can do immediately
- Phase 2 (Core): 2-4 hours (7 files, critical Q34 audit)
- Phase 3 (Optional): 4-6 hours (13 files, lower priority)
- Phase 4 (TUI): 1 hour (1 file)

**Total**: 7-11 hours to achieve 100% zero-serde codebase

## Trade-Off Analysis

### Option A: Delete All Non-Essential Code
**Pros**:
- 100% serde-free codebase immediately
- Zero maintenance burden
- Clear separation: production (atomic_capsule) vs legacy (deleted)

**Cons**:
- Lose optional binaries (download_corpus, validate_accuracy)
- Lose benchmarking infrastructure (may need to rebuild)

### Option B: Migrate Everything to atomic_capsule
**Pros**:
- Keep all functionality
- 100% atomic_capsule ecosystem
- Better performance (10-50× speedup)

**Cons**:
- 7-11 hours development time
- Risk of introducing bugs during migration

### Recommendation: Hybrid Approach
1. **DELETE**: `src/bin_disabled/` (already disabled)
2. **MIGRATE**: Core infrastructure (audit, benchmarking) - 2-4 hours
3. **DEFER**: Optional binaries (download_corpus, etc.) - migrate on-demand or delete

This gives us:
- ✅ 100% production code serde-free
- ✅ Q34 audit compliance maintained
- ✅ Benchmarking infrastructure working
- ⏳ Optional tools deferred (migrate when needed)

**Timeline**: 2-5 hours to production-ready state
