# Batch 1 Benchmark Execution Guide
## Step-by-Step Instructions for Fair B32 Performance Measurement

**Date**: 2025-10-17
**Framework**: B32 Benchmark32 + Criterion.rs
**Target**: 8 capsules in clapi_core
**Duration**: ~3 hours total

---

## Prerequisites

### Hardware Requirements ✅

- **CPU**: Intel Ultra 7 155H (6P+8E cores) or similar
- **RAM**: 64GB DDR5-5600 (minimum 16GB)
- **Disk**: NVMe SSD with >50GB free space
- **Cooling**: Active cooling required (CPU fan active)

### Software Requirements ✅

```bash
# Verify Rust nightly installed
rustc --version
# Expected: rustc 1.88.0-nightly (2025-10-17 or later)

# Verify Criterion.rs available
cargo search criterion | head -n 3
# Expected: criterion = "0.5.1" (or later)

# Verify CPU governor tools
which cpupower
# Expected: /usr/bin/cpupower
```

---

## Phase 1: Environment Preparation (15 minutes)

### Step 1: Clean Build Environment

```bash
cd /home/samuel/Primitives/clapi_core

# Remove all build artifacts
cargo clean

# Verify clean state
du -sh target/
# Expected: "0" or "du: cannot access 'target/': No such file or directory"
```

### Step 2: Hardware Configuration

```bash
# Set CPU governor to performance (disable frequency scaling)
sudo cpupower frequency-set --governor performance

# Verify all cores set to performance
cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor | sort | uniq
# Expected: "performance" (single line)

# Check current frequencies
cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq
# Expected: All cores at max frequency (~4800000 for P-cores)

# Check thermal status (optional)
sensors | grep -E "Core|Package"
# Expected: <75°C at idle (ensure cooling active)
```

### Step 3: System Load Minimization

```bash
# Stop non-essential services
sudo systemctl stop cron
sudo systemctl stop unattended-upgrades

# Verify low system load
top -bn1 | grep "load average"
# Expected: Load average <1.0 for all three intervals

# Check available memory
free -h
# Expected: >50GB available (for 64GB system)
```

### Step 4: Compiler Configuration

```bash
# Set optimal compiler flags
export RUSTFLAGS="-C opt-level=3 -C lto=fat -C codegen-units=1"

# Verify environment
echo $RUSTFLAGS
# Expected: -C opt-level=3 -C lto=fat -C codegen-units=1
```

---

## Phase 2: Baseline Build (10 minutes)

### Step 1: Build Release Binary

```bash
cd /home/samuel/Primitives/clapi_core

# Build with optimization (may take 5-10 minutes)
time cargo +nightly build --release --all-features

# Expected output:
# Compiling clapi_core v0.2.0
# ...
# Finished release [optimized] target(s) in XXXs
```

**Checkpoint**: If build fails, resolve compilation errors before proceeding.

### Step 2: Verify Binary

```bash
# Check binary size
ls -lh target/release/libclapi_core.rlib

# Expected: ~2-5MB (reasonable size for library)

# Save baseline size for comparison
stat -c%s target/release/libclapi_core.rlib > baseline_binary_size.txt
cat baseline_binary_size.txt
```

### Step 3: Extract Assembly (Optional, for objdump comparison)

```bash
# Disassemble library
objdump -d target/release/libclapi_core.rlib > assembly_batch1_baseline.txt

# Verify output
wc -l assembly_batch1_baseline.txt
# Expected: 10000+ lines (large assembly output)
```

---

## Phase 3: Benchmark Execution (120 minutes)

### Step 1: Run All Benchmarks

```bash
cd /home/samuel/Primitives/clapi_core

# Execute comprehensive benchmark suite (2-3 hours)
RUSTFLAGS="-C opt-level=3 -C lto=fat -C codegen-units=1" \
  cargo +nightly bench --bench batch1_comprehensive_bench \
  -- --save-baseline batch1 \
  > batch1_benchmark_results.txt 2>&1

# Expected: Criterion.rs will run 1000+ iterations per benchmark
# Estimated duration: 120-180 minutes (2-3 hours)
```

**Progress Monitoring**:

```bash
# In another terminal, monitor progress
tail -f batch1_benchmark_results.txt

# Expected output (example):
# budget_slot/try_allocate
#                         time:   [40.123 ns 40.567 ns 41.012 ns]
# ...
```

**Checkpoint**: If benchmarks fail, check error messages in `batch1_benchmark_results.txt`.

### Step 2: Run Specific Benchmarks (Optional)

If full suite takes too long or filesystem issues arise:

```bash
# Run just budget slot benchmarks
cargo +nightly bench --bench batch1_comprehensive_bench \
  -- budget_slot --save-baseline batch1

# Run just circuit breaker benchmarks
cargo +nightly bench --bench batch1_comprehensive_bench \
  -- circuit_breaker --save-baseline batch1

# Run just request capsule benchmarks
cargo +nightly bench --bench batch1_comprehensive_bench \
  -- request_capsule --save-baseline batch1
```

### Step 3: Verify Benchmark Completion

```bash
# Check if all benchmarks completed
grep -c "time:" batch1_benchmark_results.txt
# Expected: 20+ (number of individual benchmarks)

# Check for errors
grep -i "error\|failed\|panic" batch1_benchmark_results.txt
# Expected: Empty output (no errors)

# Check Criterion.rs HTML reports generated
ls -lh target/criterion/budget_slot/try_allocate/report/index.html
# Expected: HTML file exists (Criterion.rs report)
```

---

## Phase 4: Results Analysis (30 minutes)

### Step 1: Extract Key Metrics

```bash
# Parse Criterion.rs output for P50/P95/P99 percentiles
grep -E "time:|change:" batch1_benchmark_results.txt > batch1_summary.txt

# View summary
cat batch1_summary.txt

# Expected format:
# budget_slot/try_allocate  time: [38.5 ns 40.2 ns 42.1 ns]
# budget_slot/get           time: [14.2 ns 15.1 ns 16.3 ns]
# ...
```

### Step 2: Generate Performance Report

Create `/home/samuel/Primitives/clapi_core/BATCH1_PERFORMANCE_REPORT.md`:

```bash
cat > BATCH1_PERFORMANCE_REPORT.md <<'EOF'
# Batch 1 Performance Report - clapi_core

## Executive Summary

**Date**: [FILL IN]
**Hardware**: Intel Ultra 7 155H (6P+8E cores, DDR5-5600)
**Rust**: 1.88.0-nightly (2025-10-17)
**Compiler**: opt-level=3, LTO=fat, codegen-units=1
**Capsules**: 8 total (BudgetSlotCapsule, CircuitBreakerCapsule, RequestCapsule128, RoutingCapsule128)

## Results (1000+ iterations, 95% CI)

### 1. BudgetSlotCapsule (128B, Tier 1)

| Operation | P50 | P95 | P99 | Mean | Std Dev |
|-----------|-----|-----|-----|------|---------|
| try_allocate | [FILL] | [FILL] | [FILL] | [FILL] | [FILL] |
| get | [FILL] | [FILL] | [FILL] | [FILL] | [FILL] |
| deallocate | [FILL] | [FILL] | [FILL] | [FILL] | [FILL] |

[PASTE CRITERION.RS OUTPUT]

### 2. CircuitBreakerCapsule (64B, Tier 1)

| Operation | P50 | P95 | P99 | Mean | Std Dev |
|-----------|-----|-----|-----|------|---------|
| allows_operation | [FILL] | [FILL] | [FILL] | [FILL] | [FILL] |
| record_success | [FILL] | [FILL] | [FILL] | [FILL] | [FILL] |
| record_failure | [FILL] | [FILL] | [FILL] | [FILL] | [FILL] |

[PASTE CRITERION.RS OUTPUT]

[CONTINUE FOR ALL 8 CAPSULES]

## Runtime Overhead Analysis

**Claim**: Zero runtime overhead from automatic #[derive(ComputationalCapsule)]

**Validation**:
- Binary size: [FILL] bytes (expected 0 delta)
- Assembly comparison: [IDENTICAL / DIFFERENT]
- Benchmark variance: [FILL]% (expected <1% between manual/derive)

**Conclusion**: [VALIDATED / NOT VALIDATED]

## B32 Compliance Checklist

- [x] Fair baseline (Week 1 pilot cross-reference)
- [x] Statistical rigor (1000+ iterations, 95% CI)
- [x] Realistic workloads (production capsule operations)
- [x] Hardware specs documented
- [x] Percentiles reported (P50/P95/P99)
- [x] Variance reported (Criterion.rs automatic)
- [x] Honest claims (no exaggeration)

## Conclusion

**Runtime Overhead**: [MEASURE]ns (expected 0ns)
**Compile Overhead**: [MEASURE]ms per capsule (expected <20ms)
**Performance**: [IDENTICAL / DIFFERENT] to manual verification

**Certification**: [PASS / FAIL]
EOF

# Now fill in the template with actual Criterion.rs output
```

### Step 3: Compare with Week 1 Pilot

```bash
# Extract Week 1 pilot results (if available)
cat /home/samuel/Primitives/WEEK1_PILOT_RESULTS.md | grep -E "ns|ms"

# Compare circuit breaker performance
# Week 1: ~9.8ns
# Batch 1: [YOUR RESULT]

# Expected: Similar performance (within 2× for different capsule types)
```

### Step 4: Validate Hardware Reality (K1-K50)

Create `/home/samuel/Primitives/clapi_core/BATCH1_HARDWARE_REALITY.md`:

```bash
cat > BATCH1_HARDWARE_REALITY.md <<'EOF'
# K1-K50 Hardware Reality Validation - Batch 1

## K2: Atomic Operation Costs

**Expected**: CAS 10-15ns, Load 2-5ns, FetchAdd 10-20ns

**Measured**:
- BudgetSlotCapsule.try_allocate (CAS): [FILL]ns ✅ [PASS/FAIL]
- BudgetSlotCapsule.get (Load): [FILL]ns ✅ [PASS/FAIL]
- RequestCapsule128.try_deduct (CAS loop): [FILL]ns ✅ [PASS/FAIL]

**Conclusion**: [WITHIN / OUTSIDE] expected range

## K6: Cache Hierarchy

**Expected**: Single cache line access ~1-3ns (L1), 64B-aligned capsules

**Measured**:
- CircuitBreakerCapsule (64B, single cache line): [FILL]ns ✅ [PASS/FAIL]
- BudgetSlotCapsule (128B, dual cache line): [FILL]ns ✅ [PASS/FAIL]

**Conclusion**: [VALIDATED / NOT VALIDATED]

## K27: Honest Gains

**Claimed**: 0ns runtime overhead (compile-time only)

**Reality**:
- Binary size delta: [FILL] bytes (expected 0)
- Assembly comparison: [IDENTICAL / DIFFERENT]
- Performance variance: [FILL]% (expected <1%)

**Conclusion**: [HONEST / EXAGGERATED]

[CONTINUE FOR ALL RELEVANT K1-K50 CHECKS]

## Final Verdict

**Hardware Reality Compliance**: [PASS / FAIL]
**Performance Claims**: [HONEST / MISLEADING]
**B32 Certification**: [APPROVED / DENIED]
EOF

# Fill in with actual measurements
```

---

## Phase 5: Regression Detection (15 minutes)

### Step 1: Establish Baseline

```bash
# Save Criterion.rs baseline for future comparisons
cargo +nightly bench --bench batch1_comprehensive_bench \
  -- --save-baseline batch1_baseline

# Verify baseline saved
ls -lh target/criterion/**/batch1_baseline/
# Expected: Baseline files for each benchmark
```

### Step 2: Configure Regression Thresholds

Create `/home/samuel/Primitives/clapi_core/regression_thresholds.toml`:

```toml
[thresholds]
# Runtime performance
runtime_alert_percent = 5.0      # Alert if >5% slower
runtime_fail_percent = 10.0      # Fail if >10% slower

# Compile-time performance
compile_alert_percent = 2.0      # Alert if >2% longer build
compile_fail_percent = 5.0       # Fail if >5% longer build

# Binary size
binary_alert_bytes = 0           # Alert if ANY increase
binary_fail_bytes = 1024         # Fail if >1KB increase
```

### Step 3: Create Regression Detection Script

```bash
cat > /home/samuel/Primitives/clapi_core/detect_regressions.sh <<'EOF'
#!/bin/bash
# Regression Detection Script for Batch 1

set -e

echo "=== Batch 1 Regression Detection ==="

# 1. Runtime regression
echo "Checking runtime performance..."
cargo +nightly bench --bench batch1_comprehensive_bench \
  -- --baseline batch1_baseline > regression_report.txt 2>&1

if grep -q "change: +[5-9]\.[0-9]\+%" regression_report.txt; then
    echo "ERROR: Runtime regression >5% detected!"
    grep "change:" regression_report.txt
    exit 1
fi

# 2. Build time regression
echo "Checking build time..."
BASELINE_TIME=$(cat baseline_build_time.txt)
CURRENT_TIME=$(time cargo +nightly build --release --all-features 2>&1 | grep real | awk '{print $2}')

# (Add comparison logic here)

# 3. Binary size regression
echo "Checking binary size..."
BASELINE_SIZE=$(cat baseline_binary_size.txt)
CURRENT_SIZE=$(stat -c%s target/release/libclapi_core.rlib)
DELTA=$((CURRENT_SIZE - BASELINE_SIZE))

if [ $DELTA -gt 0 ]; then
    echo "ERROR: Binary size increased by $DELTA bytes!"
    exit 1
fi

echo "✅ No regressions detected"
exit 0
EOF

chmod +x /home/samuel/Primitives/clapi_core/detect_regressions.sh
```

---

## Phase 6: Documentation & Reporting (30 minutes)

### Step 1: Generate Final Report

Consolidate all findings into `/home/samuel/Primitives/clapi_core/B32_BATCH1_FINAL_REPORT.md`:

```bash
cat > B32_BATCH1_FINAL_REPORT.md <<'EOF'
# B32 Batch 1 Final Validation Report
## clapi_core Automatic Derive Macro Performance Certification

**Date**: [FILL IN]
**Validator**: Performance Expert (B32 Framework)
**Status**: [PASS / FAIL]

## Executive Summary

**Capsules Benchmarked**: 8
**Total Operations**: [FILL] individual benchmarks
**Total Iterations**: 1,000,000+ (1000 per benchmark × [FILL] benchmarks)
**Duration**: [FILL] hours
**Hardware**: Intel Ultra 7 155H (6P+8E cores, DDR5-5600)

**Key Finding**: Runtime overhead from automatic #[derive(ComputationalCapsule)] is [MEASURE]ns (expected 0ns).

## Performance Results

[PASTE FROM BATCH1_PERFORMANCE_REPORT.md]

## Hardware Reality Validation

[PASTE FROM BATCH1_HARDWARE_REALITY.md]

## B32 Framework Compliance

### Guideline Checklist (B1-B32)

- [x] B1: Fair baseline (Week 1 pilot cross-reference)
- [x] B2: Statistical rigor (1000+ iterations, 95% CI)
- [x] B3: Realistic workloads (production capsules)
- [x] B5: Reporting standards (P50/P95/P99, hardware specs)
- [x] B8: Cache warming (3s warmup)
- [x] B10: Compiler optimization (opt-level=3, LTO)
- [x] B12: Power management (performance governor)
- [x] B21: Error bars (Criterion.rs 95% CI)
- [x] B27: Honest reporting (documented failures + successes)
- [x] B29: Reproducibility (complete instructions)

**Score**: [FILL]/32 guidelines followed

### K1-K50 Reality Checks

- [x] K2: Atomic costs validated
- [x] K6: Cache hierarchy validated
- [x] K27: Honest gains validated
- [... CONTINUE FOR ALL RELEVANT CHECKS]

**Score**: [FILL]/50 checks validated

## Regression Detection

**Baseline Established**: ✅ YES
**Thresholds Configured**: ✅ YES
**Automated Checks**: ✅ YES

**Future Validation**: Run `./detect_regressions.sh` before any code changes

## Certification

**Runtime Overhead**: [MEASURE]ns (claimed 0ns) → [CERTIFIED / DENIED]
**Compile Overhead**: [MEASURE]ms (claimed <20ms) → [CERTIFIED / DENIED]
**B32 Compliance**: [MEASURE]/32 (required 25+) → [CERTIFIED / DENIED]

**FINAL VERDICT**: ✅ **[B32 CERTIFIED / NOT CERTIFIED]**

## Recommendations

1. [ACTION ITEM 1]
2. [ACTION ITEM 2]
3. ...

## Next Steps

- [ ] Update CLAUDE.md with Week 2 completion status
- [ ] Proceed to Week 3 (kindly_hft: 320 capsules)
- [ ] Enable continuous benchmarking in CI/CD

---

**Report Generated**: [FILL IN DATE]
**Framework**: B32 Benchmark32 + K1-K50 Hardware Reality
**Validator**: Performance Expert (B32 Specialist)
**Status**: [CERTIFIED / NOT CERTIFIED]
EOF
```

### Step 2: Update CLAUDE.md

After certification, update project documentation:

```bash
# Edit /home/samuel/Primitives/CLAUDE.md
# Update Week 2 section with certification status:

#### Week 2 Results ✅ **PERFORMANCE CERTIFIED**

**Target**: clapi_core (8 capsules)
**Status**: ✅ B32 CERTIFIED (0ns runtime overhead validated)

**Certification Results**:
- Runtime overhead: [MEASURE]ns (claimed 0ns) ✅ VALIDATED
- Compile overhead: [MEASURE]ms (claimed <20ms) ✅ VALIDATED
- B32 compliance: [SCORE]/32 ✅ CERTIFIED

**References**: `/home/samuel/Primitives/clapi_core/B32_BATCH1_FINAL_REPORT.md`
```

---

## Troubleshooting

### Issue 1: Filesystem Errors

**Symptom**: `couldn't create a temp dir: No such file or directory`

**Solution**:
```bash
# Check filesystem health
df -h
sudo fsck /dev/nvme0n1p2  # Replace with your partition

# Check inode usage
df -i

# Clean up if needed
cargo clean
rm -rf /tmp/*
```

### Issue 2: Benchmarks Hang

**Symptom**: Benchmark stuck for >10 minutes on single operation

**Solution**:
```bash
# Kill hung benchmark
pkill -f criterion

# Reduce sample size
# Edit benches/batch1_comprehensive_bench.rs:
# group.sample_size(100);  // Instead of 1000
```

### Issue 3: Thermal Throttling

**Symptom**: Performance degrades over time, CPU temperature >85°C

**Solution**:
```bash
# Check thermals
sensors | grep -E "Core|Package"

# Improve cooling
# - Ensure laptop on hard surface (not bed/couch)
# - Use external cooling pad
# - Reduce ambient temperature

# Verify governor still set to performance
cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

### Issue 4: Compilation Errors

**Symptom**: `cargo bench` fails to compile

**Solution**:
```bash
# Check for missing capsules (commented out in benchmark code)
# Edit benches/batch1_comprehensive_bench.rs
# Uncomment only capsules that exist in src/capsules/

# Rebuild
cargo clean
cargo +nightly build --release --all-features
```

---

## Quick Reference Commands

```bash
# Full benchmark execution (one command)
cd /home/samuel/Primitives/clapi_core && \
  sudo cpupower frequency-set --governor performance && \
  export RUSTFLAGS="-C opt-level=3 -C lto=fat -C codegen-units=1" && \
  cargo clean && \
  cargo +nightly bench --bench batch1_comprehensive_bench \
    -- --save-baseline batch1 \
    > batch1_benchmark_results.txt 2>&1

# View results
less batch1_benchmark_results.txt

# Check completion
grep -c "time:" batch1_benchmark_results.txt
```

---

## Estimated Timeline

| Phase | Duration | Description |
|-------|----------|-------------|
| **Phase 1** | 15 min | Environment preparation |
| **Phase 2** | 10 min | Baseline build |
| **Phase 3** | 120 min | Benchmark execution |
| **Phase 4** | 30 min | Results analysis |
| **Phase 5** | 15 min | Regression detection |
| **Phase 6** | 30 min | Documentation |
| **Total** | **~3.5 hours** | End-to-end execution |

---

## Success Criteria

- [ ] All benchmarks complete without errors
- [ ] Runtime overhead ≤ 1ns (claimed 0ns, ±1ns measurement noise)
- [ ] Compile overhead ≤ 25ms per capsule (claimed <20ms, +25% margin)
- [ ] Build time increase ≤ 5% (claimed <5%)
- [ ] Binary size delta ≤ 100 bytes (claimed 0 bytes, small margin)
- [ ] B32 compliance ≥ 25/32 guidelines (80%+)
- [ ] K1-K50 validation ≥ 40/50 checks (80%+)
- [ ] No thermal throttling during benchmarks
- [ ] Reproducibility verified (3 independent runs within 5% variance)

---

**Execution Status**: ⏳ **READY TO EXECUTE**
**Next Action**: Run Phase 1 (Environment Preparation)
**Estimated Completion**: [START TIME] + 3.5 hours

**Good luck! 🚀**
