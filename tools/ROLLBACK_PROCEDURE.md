# Phase 4 Migration Rollback Procedure

**Purpose**: Emergency rollback procedure if derive macro migration causes production issues.

**Scope**: All 7 projects (atomic_capsule, clapi_core, kindly_hft, kindly-db, kiang, others)

**Rollback Time**: <5 minutes (git restore) to <1 hour (full validation)

---

## When to Rollback

Rollback IMMEDIATELY if any of the following occur:

### Critical Failures (Immediate Rollback Required)

- [ ] **Tests fail** after migration (any test failure)
- [ ] **Compilation errors** after migration
- [ ] **Production crashes** (clapi_core, kindly_hft)
- [ ] **Data corruption** (kindly-db, audit trails)
- [ ] **Performance regression** >10% (B32 benchmarks)
- [ ] **Brain correctness failure** (kindly_hft historical replay)
- [ ] **ACID violation** (kindly-db transaction tests)

### Warning Conditions (Investigate, Possible Rollback)

- [ ] Performance regression 5-10% (investigate before rollback)
- [ ] Clippy warnings introduced (investigate, may be false positive)
- [ ] Compilation time increase >20ms/capsule
- [ ] Binary size increase >5%

---

## Rollback Decision Matrix

| Condition | Severity | Action | Timeframe |
|-----------|----------|--------|-----------|
| **Test failures** | CRITICAL | Immediate rollback | <5 min |
| **Compilation errors** | CRITICAL | Immediate rollback | <5 min |
| **Production crash** | CRITICAL | Immediate rollback + incident report | <5 min |
| **Data corruption** | CRITICAL | Immediate rollback + forensic analysis | <5 min |
| **Perf regression >10%** | HIGH | Rollback + investigate | <15 min |
| **Brain incorrectness** | CRITICAL | Immediate rollback + replay analysis | <5 min |
| **ACID violation** | CRITICAL | Immediate rollback + transaction log | <5 min |
| **Perf regression 5-10%** | MEDIUM | Investigate first, rollback if needed | <1 hour |
| **Clippy warnings** | LOW | Investigate, fix warnings if valid | No rollback |
| **Compile time >20ms** | LOW | Investigate, optimize derive macro | No rollback |

---

## Rollback Procedure

### Step 1: Checkpoint Verification (1 minute)

Before rolling back, verify checkpoint exists:

```bash
# List all Phase 4 migration tags
git tag | grep "phase4"

# Expected output:
# atomic_capsule-phase4.1-complete
# clapi_core-phase4.2-complete
# kindly_hft-phase4.3-complete
# kindly-db-phase4.4-complete
# phase4-migration-complete

# If tag doesn't exist, create emergency checkpoint NOW
git tag -a "pre-rollback-emergency" -m "Emergency checkpoint before rollback"
```

### Step 2: Identify Rollback Scope (1 minute)

Determine which project(s) need rollback:

```bash
# Check which project failed
PROJECT_FAILED="atomic_capsule"  # Example: replace with actual failed project

# Verify project state
cd "${PROJECT_FAILED}"
git status
git log --oneline -5
```

### Step 3: Execute Rollback (2 minutes)

#### Option A: Single Project Rollback (Preferred)

```bash
# Rollback single project to pre-migration state
cd "${PROJECT_FAILED}"

# Restore all Rust files to pre-migration state
git restore --source=HEAD~1 **/*.rs

# Clean build artifacts
cargo clean

# Verify rollback
cargo test --all-features
cargo clippy --all-features -- -D warnings
```

#### Option B: Full Repository Rollback (Nuclear Option)

```bash
# Rollback entire repository to last known-good state
cd /home/samuel/Primitives

# Find last good checkpoint
git log --oneline --decorate | grep "phase4"

# Rollback to checkpoint (replace CHECKPOINT with actual tag/commit)
CHECKPOINT="atomic_capsule-phase4.1-complete"
git reset --hard "${CHECKPOINT}"

# Clean all build artifacts
for project in atomic_capsule clapi_core kindly_hft kindly-db kiang; do
    if [ -d "${project}" ]; then
        cd "${project}"
        cargo clean
        cd ..
    fi
done
```

### Step 4: Validation (5-15 minutes)

After rollback, validate all projects compile and tests pass:

```bash
#!/bin/bash
# validate_rollback.sh - Verify rollback successful

PROJECTS=("atomic_capsule" "clapi_core" "kindly_hft" "kindly-db" "kiang")

for project in "${PROJECTS[@]}"; do
    if [ -d "${project}" ]; then
        echo "Validating ${project}..."
        cd "${project}"

        # Compilation check
        cargo build --all-features 2>&1 | tee "../rollback_${project}_build.log"
        if [ $? -ne 0 ]; then
            echo "ERROR: ${project} failed to compile after rollback!"
            exit 1
        fi

        # Test check
        cargo test --all-features 2>&1 | tee "../rollback_${project}_tests.log"
        if [ $? -ne 0 ]; then
            echo "ERROR: ${project} tests failed after rollback!"
            exit 1
        fi

        # Clippy check
        cargo clippy --all-features -- -D warnings 2>&1 | tee "../rollback_${project}_clippy.log"
        if [ $? -ne 0 ]; then
            echo "WARNING: ${project} has clippy warnings after rollback"
        fi

        cd ..
        echo "✓ ${project} validated successfully"
    fi
done

echo "✓ All projects validated after rollback"
```

### Step 5: Document Rollback (10 minutes)

```bash
# Create rollback incident report
cat > "ROLLBACK_INCIDENT_$(date +%Y%m%d_%H%M%S).md" << EOF
# Phase 4 Migration Rollback Incident

**Date**: $(date)
**Project**: ${PROJECT_FAILED}
**Reason**: [DESCRIBE FAILURE]

## Failure Details

- **Test failures**: [LIST FAILED TESTS]
- **Compilation errors**: [LIST ERRORS]
- **Performance regression**: [BENCHMARK DATA]
- **Other**: [DESCRIBE]

## Rollback Actions Taken

1. Restored project to: [CHECKPOINT/COMMIT]
2. Validated compilation: [PASS/FAIL]
3. Validated tests: [N/N PASS]
4. Validated benchmarks: [WITHIN 5% BASELINE]

## Root Cause Analysis

[DESCRIBE WHAT WENT WRONG]

## Next Steps

1. [FIX DERIVE MACRO BUG]
2. [RE-RUN MIGRATION WITH FIX]
3. [VALIDATE AGAIN]

## Lessons Learned

[DOCUMENT WHAT WE LEARNED]
EOF

echo "✓ Rollback incident report created"
```

---

## Per-Project Rollback Instructions

### atomic_capsule (250 macros)

```bash
cd atomic_capsule

# Rollback to pre-Phase 4.1 state
git restore --source=HEAD~1 src/**/*.rs

# Validate
cargo test --all-features
cargo bench --all-features
cargo clippy --all-features -- -D warnings

# Expected: 266 tests pass, 0 warnings
```

**Critical Validation**:
- [ ] DualAtomicU64 benchmarks within 5% baseline
- [ ] SIMD capsules aligned correctly (256B for f64x8)
- [ ] Hash module 0ns const hash maintained
- [ ] Collections lockfree (<50ns operations)

### clapi_core (94 macros)

```bash
cd clapi_core

# Rollback to pre-Phase 4.2 state
git restore --source=HEAD~1 src/**/*.rs

# Validate
cargo test --all-features
cargo bench --all-features

# Production validation (CRITICAL)
cargo test --test integration_tests -- --ignored

# Expected: 365 tests pass, <300ns hot path
```

**Critical Validation**:
- [ ] Budget registry lockfree allocation (<100ns)
- [ ] Circuit breakers multi-provider correct
- [ ] HTTP hot path <300ns (budget + routing + metrics)
- [ ] Load test 60M ops/s @ 8 threads

**Production Rollback** (if deployed to production):

```bash
# If migration was deployed to production, rollback deployment
cd clapi_core

# Feature flag disable (zero downtime)
# Edit config/rollout_config.toml:
# [features]
# derive_macros = false  # Disable derive macros, use manual macros

# Restart service
systemctl restart clapi_core

# Validate production metrics
curl http://localhost:8080/metrics | jq '.circuit_breaker'
```

### kindly_hft (200+ macros)

```bash
cd kindly_hft

# Rollback to pre-Phase 4.3 state
git restore --source=HEAD~1 src/**/*.rs

# Validate per zone
for zone in brainstem hypothalamus thalamus hippocampus prefrontal_cortex motor_cortex anterior_cingulate amygdala insular_cortex cerebellum basal_ganglia association_cortex primary_sensory; do
    echo "Validating ${zone}..."
    cargo test --lib "${zone}_tests"
done

# Critical: Historical replay (brain correctness)
cargo test --test historical_replay -- --ignored --test-threads=1

# Expected: Bit-exact outputs (no divergence)
```

**Critical Validation**:
- [ ] All 14 brain zones pass tests
- [ ] Hebbian learning 2.5ns/connection (19× speedup)
- [ ] Batch atomic updates 10μs (57× speedup)
- [ ] Historical replay produces identical outputs
- [ ] 960K neurons training validates

**Brain Correctness Critical**:

If brain produces different trading decisions after rollback, **this is a critical bug**:

```bash
# Run comprehensive brain correctness tests
cargo test --test brain_correctness_validation -- --ignored

# Compare trading decisions (historical replay)
./scripts/compare_brain_outputs.sh \
    migration_baseline_brain_outputs.json \
    rollback_brain_outputs.json

# Expected: 100% identical (bit-exact)
```

### kindly-db (40 macros)

```bash
cd kindly-db

# Rollback to pre-Phase 4.4 state
git restore --source=HEAD~1 src/**/*.rs

# Validate ACID properties
cargo test --test acid_compliance_tests -- --ignored
cargo test --test concurrent_transactions -- --ignored --test-threads=16
cargo test --test data_integrity_validation -- --ignored

# Expected: All ACID tests pass, no data corruption
```

**Critical Validation**:
- [ ] ACID properties validated (Atomicity, Consistency, Isolation, Durability)
- [ ] Concurrent transactions correct (16 threads, 10,000 txns)
- [ ] No data corruption detected
- [ ] Query performance within 5% baseline

**Data Integrity Critical**:

```bash
# Verify no data corruption after rollback
cargo test --test data_integrity_full_scan -- --ignored

# Expected: 100% data integrity (no corruption)
```

### kiang + others (34 macros)

```bash
# Rollback all remaining projects
for project in kiang atomic_hedge_capsule atomic_position_capsule atomic_risk_envelope; do
    if [ -d "${project}" ]; then
        cd "${project}"
        git restore --source=HEAD~1 src/**/*.rs
        cargo test --all-features
        cd ..
    fi
done
```

---

## Rollback Validation Checklist

After rollback, ALL of the following must be true:

### Compilation
- [ ] All projects compile successfully
- [ ] Zero compilation errors
- [ ] Zero clippy warnings (same as baseline)

### Tests
- [ ] atomic_capsule: 266/266 tests pass
- [ ] clapi_core: 365/365 tests pass
- [ ] kindly_hft: All zone tests pass + historical replay identical
- [ ] kindly-db: ACID compliance tests pass
- [ ] kiang + others: All tests pass

### Performance
- [ ] Benchmarks within 5% of pre-migration baseline
- [ ] No performance regression detected
- [ ] Memory layout unchanged (size, alignment)

### Production (if applicable)
- [ ] clapi_core production metrics normal (<300ns hot path)
- [ ] kindly_hft brain outputs identical (historical replay)
- [ ] kindly-db no data corruption

---

## Emergency Contact Procedure

If rollback fails or causes new issues:

### Tier 1: Immediate Response (0-15 minutes)

1. **STOP all migrations** across all projects
2. **Document failure** (logs, errors, metrics)
3. **Notify team** (if applicable)
4. **Isolate affected systems** (production only)

### Tier 2: Investigation (15-60 minutes)

1. **Analyze root cause**:
   - Diff manual macros vs derive macros
   - Review compilation output
   - Check test failures
   - Examine performance data

2. **Determine fix strategy**:
   - Fix derive macro bug
   - Fix manual macro compatibility
   - Fix project-specific issue

### Tier 3: Recovery (1-4 hours)

1. **Implement fix**
2. **Re-run validation** (full T28 suite)
3. **Re-attempt migration** (if fix successful)
4. **Document lessons learned**

---

## Rollback Testing Procedure

**Test rollback procedure BEFORE executing real migration**:

```bash
#!/bin/bash
# test_rollback_procedure.sh - Dry-run rollback test

# Create test branch
git checkout -b test-rollback-procedure

# Simulate migration
cargo +nightly -Zscript tools/migrate_verify_macros_to_derive.rs migrate --dry-run atomic_capsule

# Simulate rollback
git restore atomic_capsule/**/*.rs

# Validate rollback worked
cd atomic_capsule
cargo test --all-features
cd ..

# Clean up test branch
git checkout main
git branch -D test-rollback-procedure

echo "✓ Rollback procedure tested successfully"
```

---

## Rollback Success Criteria

Rollback is considered **successful** if:

- [ ] All projects compile (zero errors)
- [ ] All tests pass (100% pass rate, same as baseline)
- [ ] Benchmarks within 5% of pre-migration baseline
- [ ] Zero clippy warnings (same as baseline)
- [ ] Production systems unaffected (if deployed)
- [ ] No data corruption (kindly-db)
- [ ] Brain correctness maintained (kindly_hft)

---

## Post-Rollback Actions

After successful rollback:

1. **Incident Report**: Document failure in `ROLLBACK_INCIDENT_*.md`
2. **Root Cause Analysis**: Identify why migration failed
3. **Fix Derive Macro**: Update `atomic_capsule_derive` if bug found
4. **Re-Test Fix**: Validate fix with small test case
5. **Re-Attempt Migration**: Only after fix validated
6. **Update Documentation**: Note edge case in migration guide

---

## Lessons Learned Template

```markdown
# Phase 4 Migration Rollback - Lessons Learned

## What Went Wrong

[DESCRIBE FAILURE]

## Root Cause

[IDENTIFY ROOT CAUSE]

## Fix Applied

[DESCRIBE FIX]

## Prevention Strategy

[HOW TO PREVENT IN FUTURE]

## Updated Procedures

[WHAT PROCEDURES WERE UPDATED]
```

---

## Rollback Metrics

Track rollback frequency and success rate:

| Date | Project | Reason | Rollback Time | Success | Fix Applied |
|------|---------|--------|---------------|---------|-------------|
| 2025-10-20 | atomic_capsule | Test failure | 5 min | ✅ | Derive macro alignment bug |
| - | - | - | - | - | - |

**Target**: <5% rollback rate (95%+ migration success rate)

---

## Final Notes

- **Rollback is SAFE**: Git ensures all code is recoverable
- **Rollback is FAST**: <5 minutes for emergency rollback
- **Rollback is TESTED**: Procedure tested before real migration
- **Rollback is DOCUMENTED**: All steps clearly documented

**When in doubt, ROLLBACK immediately. Better safe than sorry.**
