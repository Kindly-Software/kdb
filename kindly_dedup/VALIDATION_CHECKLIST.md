# Validation Checklist - Week 1 Hardware Validation

**Date**: 2025-10-29
**Hardware**: AMD Ryzen 9 6900HX (16 cores, 64GB RAM) @ 192.168.0.38
**Goal**: Validate 500K+ docs/sec multi-threaded throughput

---

## Day 1: Deploy & Benchmark (Monday)

### Step 1: SSH to Training Server

```bash
ssh samuel@192.168.0.38
```

**Verify**:
- [ ] Connection successful
- [ ] CPU: AMD Ryzen 9 6900HX (check with `lscpu`)
- [ ] Cores: 16 cores (check with `nproc`)
- [ ] Memory: 64GB (check with `free -h`)

### Step 2: Navigate to Project

```bash
cd ~/Primitives/kindly_dedup
```

**Verify**:
- [ ] Directory exists
- [ ] Code synced (lsyncd auto-sync from 192.168.0.103)
- [ ] Latest commit matches local (check with `git log -1`)

### Step 3: Build & Run Benchmarks

```bash
# Clean build
cargo clean

# Run parallel benchmarks (takes 10-15 minutes)
cargo bench --features parallel-dedup --bench parallel_bench
```

**Expected Output**:
```
parallel_scaling/1_thread_1000_docs    time:   [50-55 ms ...]
parallel_scaling/4_threads_1000_docs   time:   [15-20 ms ...]
parallel_scaling/8_threads_1000_docs   time:   [8-12 ms ...]
parallel_scaling/16_threads_1000_docs  time:   [1.5-2.5 ms ...]
```

**Verify**:
- [ ] Benchmarks complete (no errors)
- [ ] HTML reports generated (`target/criterion/index.html`)
- [ ] Throughput estimates visible in output

### Step 4: Extract Results

```bash
# Open HTML reports (copy to local if needed)
# OR: grep benchmark output

grep "time:" target/criterion/parallel_scaling/*/new/estimates.txt

# Calculate throughput
# 1000 docs / (latency in ms) × 1000 = docs/sec
```

**Verify**:
- [ ] 16-thread latency extracted (target: <2ms for 1000 docs)
- [ ] Throughput calculated (target: >500K docs/sec)
- [ ] Parallel efficiency calculated (target: 60-80%)

---

## Day 2: Analyze Results (Tuesday)

### Step 1: Calculate Throughput

**Formula**:
```
Throughput = 1000 docs / (latency_ms / 1000) = docs/sec
```

**Example**:
- 16 threads, 1000 docs: 1.74 ms latency
- Throughput = 1000 / (1.74 / 1000) = 574,713 docs/sec ✅

**Checklist**:
- [ ] Single-threaded: ~60K docs/sec (baseline)
- [ ] 4 threads: ~200K docs/sec (3.4× speedup)
- [ ] 8 threads: ~350K docs/sec (6× speedup)
- [ ] 16 threads: **≥500K docs/sec** (TARGET)

### Step 2: Calculate Parallel Efficiency

**Formula**:
```
Efficiency = (Actual Throughput) / (Threads × Single-threaded Throughput)
```

**Example**:
- Single-threaded: 60,000 docs/sec
- 16 threads actual: 576,000 docs/sec
- Efficiency = 576,000 / (16 × 60,000) = 60% ✅

**Checklist**:
- [ ] 4 threads: 80-90% efficiency (expected)
- [ ] 8 threads: 70-80% efficiency (expected)
- [ ] 16 threads: **60-80% efficiency** (TARGET)

### Step 3: Validate Speedup vs Baseline

**Baseline**: Python datasketch = 1,572 docs/sec

**Formula**:
```
Speedup = (Actual Throughput) / (Baseline Throughput)
```

**Example**:
- 16 threads: 576,000 docs/sec
- Speedup = 576,000 / 1,572 = 366× ✅

**Checklist**:
- [ ] Single-threaded: 38× speedup (validated)
- [ ] 16 threads: **≥116× speedup** (TARGET)
- [ ] 16 threads: **≥174× stretch** (STRETCH GOAL)
- [ ] 16 threads: **366× projected** (EXPECTED)

### Step 4: Decision Point

**PASS Criteria** (ALL must be met):
- ✅ Throughput ≥500K docs/sec (16 cores)
- ✅ Parallel efficiency ≥50%
- ✅ Speedup ≥116× vs baseline
- ✅ No crashes, errors, or anomalies

**Decision**:
- [ ] **PASS**: Proceed to Day 3 (stress test)
- [ ] **MARGINAL** (400-500K): Profile with perf, optimize
- [ ] **FAIL** (<400K): Investigate bottlenecks, 1-week delay

---

## Day 3: Stress Test (Wednesday, if Day 2 passes)

### Step 1: Build Stress Test Binary

```bash
# Navigate to project
cd ~/Primitives/kindly_dedup

# Build stress test (takes 2-3 minutes)
cargo build --release --features parallel-dedup --bin stress_test_10m
```

**Verify**:
- [ ] Build successful (no errors)
- [ ] Binary exists: `ls -lh target/release/stress_test_10m`

### Step 2: Run 10M Document Stress Test

```bash
# Run stress test (target: <60 seconds)
time ./target/release/stress_test_10m

# Monitor in separate terminal (optional)
ssh samuel@192.168.0.38
htop  # Watch CPU/memory usage
```

**Expected Output**:
```
Generating 10M documents... done (30s)
Deduplicating... done (17s)
Total: 10,000,000 documents
Duplicates: 2,000,000 (20%)
Unique: 8,000,000 (80%)
Time: 47 seconds
Throughput: 212,766 docs/sec
```

**Verify**:
- [ ] Total time <60 seconds (TARGET)
- [ ] Memory peak <10GB (check with `htop` during run)
- [ ] No crashes, errors, or warnings
- [ ] Throughput ≥166K docs/sec (10M / 60s)

### Step 3: Sustained Load Test (Optional)

```bash
# Run 1-hour sustained stress test
for i in {1..10}; do
  echo "Run $i of 10..."
  time ./target/release/stress_test_10m
done
```

**Verify**:
- [ ] Consistent performance (±5% variance)
- [ ] No memory leaks (memory returns to baseline)
- [ ] No crashes after 10 runs

---

## Success Criteria Summary

### Minimum Viable Performance (PASS)

| Metric | Target | Status |
|--------|--------|--------|
| **16-core Throughput** | ≥500K docs/sec | [ ] Pass / [ ] Fail |
| **Parallel Efficiency** | ≥50% | [ ] Pass / [ ] Fail |
| **Speedup vs Baseline** | ≥116× | [ ] Pass / [ ] Fail |
| **10M Docs Time** | <60 seconds | [ ] Pass / [ ] Fail |
| **Memory Peak** | <10GB | [ ] Pass / [ ] Fail |
| **Zero Crashes** | 100% uptime | [ ] Pass / [ ] Fail |

**Overall Status**: [ ] PASS / [ ] MARGINAL / [ ] FAIL

---

## Next Steps After Validation

### If PASS (All criteria met)

**Week 2: Production Deployment**
- [ ] Day 8: HTTP API server (Axum + Tokio)
- [ ] Day 9: Production stress test (Common Crawl)
- [ ] Day 10: Monitoring (Prometheus + Grafana)
- [ ] Day 11-14: Launch week (HN, Twitter, Product Hunt)

**Target**: 100+ signups Week 1, 10+ paying customers Month 1

### If MARGINAL (400-500K docs/sec)

**Week 1-2: Optimization**
- [ ] Profile with perf/flamegraph
- [ ] Optimize hot paths (LSH bucketing, Jaccard verification)
- [ ] Re-run benchmarks
- [ ] Retry validation

**Timeline**: +1 week optimization buffer

### If FAIL (<400K docs/sec)

**Week 1-2: Investigation**
- [ ] Investigate algorithmic bottlenecks
- [ ] Review parallel algorithm (Rayon overhead?)
- [ ] Consider GPU acceleration (fallback)
- [ ] Re-architect if needed

**Timeline**: +2 weeks investigation + optimization

---

## Troubleshooting

### Benchmark Output Missing/Incomplete

**Symptom**: No HTML reports or incomplete output

**Fix**:
```bash
# Clean and retry
cargo clean
rm -rf target/criterion
cargo bench --features parallel-dedup --bench parallel_bench
```

### Throughput Below Expectations

**Symptom**: 16-core throughput <500K docs/sec

**Debug**:
```bash
# Profile with perf
cargo build --release --features parallel-dedup
perf record -g ./target/release/benchmark_binary
perf report

# Generate flamegraph
cargo flamegraph --bench parallel_bench
```

### Memory Issues

**Symptom**: OOM errors or high memory usage

**Debug**:
```bash
# Check memory during run
htop  # Monitor in separate terminal

# Reduce document size if needed
# Edit stress_test_10m.rs: decrease document count
```

### SSH Connection Issues

**Symptom**: Can't connect to 192.168.0.38

**Debug**:
```bash
# Check network
ping 192.168.0.38

# Check SSH service
ssh -v samuel@192.168.0.38

# Check lsyncd status (local)
cat ~/.local/share/lsyncd/kindly_hft.status
```

---

## Report Template

After validation, update `FINAL_GO_NO_GO_DECISION.md`:

```markdown
## Measured Results (16-Core Server)

| Metric | Target | Measured | Status |
|--------|--------|----------|--------|
| Throughput (16 cores) | 576K docs/sec | XXX K docs/sec | ✓/✗ |
| Parallel efficiency | 60% | XX% | ✓/✗ |
| Speedup vs baseline | 116-174× | XXX× | ✓/✗ |
| 10M docs total time | <60s | XXs | ✓/✗ |
| Memory usage | <10GB | X.XGB | ✓/✗ |

**FINAL DECISION**: [GO / NO-GO / CONDITIONAL]
```

---

**Checklist Version**: 1.0
**Created**: 2025-10-29
**Owner**: Claude Code (Performance + Decision Expert)
**Target Completion**: Day 3 (Wednesday, Week 1)
