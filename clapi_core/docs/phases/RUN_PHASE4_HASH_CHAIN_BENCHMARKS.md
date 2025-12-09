# Quick Start: Phase 4 Hash Chain Validation Benchmarks

**File**: `benches/capsule_hash64_chain_bench.rs`
**Benchmarks**: 14 comprehensive benchmarks
**Runtime**: ~4-5 minutes total
**Framework**: B32 (32 guidelines + 50 hardware reality checks)

---

## Run All Benchmarks

```bash
cargo bench --bench capsule_hash64_chain_bench
```

**Output**: Criterion HTML report in `target/criterion/`

---

## Run Specific Benchmark Groups

### Chain Verification (4 benchmarks)
```bash
cargo bench --bench capsule_hash64_chain_bench verify_chain
```

**Measures**: O(n) vs O(n²) chain verification (10/100/1000 entries + broken link)

### State Lookup (3 benchmarks)
```bash
cargo bench --bench capsule_hash64_chain_bench find_state
```

**Measures**: Linear search for hash in chain (found/not found/1000 entries)

### Audit Trail Export (3 benchmarks)
```bash
cargo bench --bench capsule_hash64_chain_bench export_audit
```

**Measures**: Streaming export to metadata (10/100/1000 entries)

### Chain Walk (2 benchmarks)
```bash
cargo bench --bench capsule_hash64_chain_bench walk_chain
```

**Measures**: Backward traversal with verification (10/1000 entries)

### Scaling Analysis (2 benchmarks)
```bash
cargo bench --bench capsule_hash64_chain_bench variable_sizes
cargo bench --bench capsule_hash64_chain_bench hash_lookup_production
```

**Measures**: O(n) vs O(n²) scaling + production scenarios

---

## Run Single Benchmark

```bash
# Example: Verify 10-entry chain
cargo bench --bench capsule_hash64_chain_bench verify_chain_10_entries

# Example: Export 100 entries
cargo bench --bench capsule_hash64_chain_bench export_audit_trail_100
```

---

## Performance Targets

| Benchmark | Target | Hardware Reality Check |
|-----------|--------|------------------------|
| `verify_chain_10_entries/optimized_on` | <1μs | K10: O(n) single pass |
| `verify_chain_100_entries/optimized_on` | <10μs | K10: Linear scaling |
| `verify_chain_1000_entries/optimized_on` | <100μs | K10: O(n) maintains advantage |
| `find_state_at_hash_found` | <5μs | K10: Average case O(n/2) |
| `find_state_at_hash_1000` | <50μs | K10: Linear search O(n) |
| `export_audit_trail_10` | <2μs | K13: Zero allocation |
| `export_audit_trail_100` | <20μs | K13: Cache-friendly sequential |
| `export_audit_trail_1000` | <200μs | K13: L2 cache-friendly |
| `walk_chain_backward_10` | <1μs | K2+K6: Pointer chase + cache |
| `walk_chain_backward_1000` | <100μs | K2+K6: Sequential access |

---

## Expected Speedups (B32 K27 Reality Check)

| Chain Size | Optimized (O(n)) | Baseline (O(n²)) | Speedup |
|------------|------------------|------------------|---------|
| 10 entries | ~1μs | ~10μs | **10×** |
| 100 entries | ~10μs | ~1ms | **100×** |
| 1000 entries | ~100μs | ~100ms | **1000×** |

**Why realistic?**: Algorithm change (O(n) vs O(n²)), not micro-optimization. K10 hardware reality check confirms Big-O constants matter.

---

## Interpreting Results

### Sample Output

```
verify_chain_10_entries/optimized_on
                        time:   [897.32 ns 905.18 ns 913.94 ns]
                        thrpt:  [10.944 Melem/s 11.049 Melem/s 11.146 Melem/s]
                                             ↑ median (use this)

verify_chain_10_entries/baseline_naive_on2
                        time:   [8.9432 μs 9.0184 μs 9.0986 μs]
                        thrpt:  [1.0990 Melem/s 1.1088 Melem/s 1.1182 Melem/s]

Speedup: 9.96× (9018.4ns / 905.18ns)
```

### What to Look For

- **Median time**: Most reliable metric (middle value, ~905ns above)
- **95% CI**: `[897ns - 914ns]` (tight range = good)
- **Throughput**: Elements/second (higher is better)
- **Speedup**: Baseline / Optimized (expect 10-1000× for O(n) vs O(n²))

---

## Regression Detection

### CI/CD Integration

```yaml
# .github/workflows/ci.yml
- name: Run hash chain benchmarks
  run: cargo bench --bench capsule_hash64_chain_bench -- --baseline main

- name: Check for regressions
  run: |
    # Compare against main baseline
    cargo bench --bench capsule_hash64_chain_bench -- \
      --save-baseline pr-${{ github.event.pull_request.number }}
```

### Manual Comparison

```bash
# Baseline (main branch)
git checkout main
cargo bench --bench capsule_hash64_chain_bench -- --save-baseline main

# Test (feature branch)
git checkout feature/optimization
cargo bench --bench capsule_hash64_chain_bench -- --baseline main
```

**Criterion will show**: `change: -5.2%` (5.2% faster) or `change: +3.1%` (3.1% slower)

---

## Troubleshooting

### Benchmark Takes Too Long

**Issue**: `verify_chain_1000_entries/baseline_naive_on2` takes ~100ms per sample
**Solution**: Skip large baseline benchmarks
```bash
# Only run optimized (skip baseline O(n²))
cargo bench --bench capsule_hash64_chain_bench optimized_on
```

### High Variance

**Issue**: CI > 10% (e.g., `[850ns - 1050ns]`)
**Causes**:
- Background processes (close Chrome/Slack)
- Thermal throttling (ensure cooling)
- Turbo boost instability (disable in BIOS)

**Fix**:
```bash
# Increase sample size
cargo bench --bench capsule_hash64_chain_bench -- --sample-size 2000
```

### Unexpected Speedup

**Issue**: Speedup >1000× (suspicious per K27)
**Debug**:
1. Check compiler optimizations (`--release` flag)
2. Verify `black_box()` prevents dead code elimination
3. Compare assembly (`cargo asm`)

---

## Hardware Specifications

**For reproducibility**, document hardware:

```bash
# CPU info
lscpu | grep "Model name"
# Example: Intel(R) Core(TM) Ultra 7 155H

# RAM info
free -h
# Example: 64GB DDR5-5600

# Thermal state
sensors
# Example: CPU temp <70°C (good), >85°C (throttling)
```

**Include in report**: CPU model, RAM size/speed, cooling state

---

## Benchmark Summary

| Category | Benchmarks | Total Runtime |
|----------|-----------|---------------|
| Chain Verification | 4 | ~2 min |
| State Lookup | 3 | ~45s |
| Audit Trail Export | 3 | ~40s |
| Chain Walk | 2 | ~30s |
| Scaling Analysis | 2 | ~1 min |
| **TOTAL** | **14** | **~5 min** |

---

## Quick Validation

```bash
# Verify benchmark compiles
cargo check --bench capsule_hash64_chain_bench

# Run single fast benchmark
cargo bench --bench capsule_hash64_chain_bench verify_chain_10_entries/optimized_on

# Expected: ~1μs (< 1000ns)
```

---

## HTML Report

After running benchmarks:

```bash
# Open HTML report
firefox target/criterion/verify_chain_10_entries/report/index.html

# Or via HTTP server
cd target/criterion
python3 -m http.server 8000
# Visit: http://localhost:8000
```

**Report includes**:
- Box plots (distribution)
- Line charts (history)
- Violin plots (density)
- Statistical analysis (confidence intervals)

---

## Next Steps

1. **Run benchmarks**: `cargo bench --bench capsule_hash64_chain_bench`
2. **Collect data**: Save output + HTML report
3. **Compare targets**: Verify results match performance targets
4. **Document findings**: Actual speedups vs expected (10×/100×/1000×)
5. **Commit baseline**: Save for future regression detection

---

**Status**: ✅ Ready to Run
**Framework**: B32 (32 guidelines + 50 hardware reality checks)
**Deliverables**: 14 benchmarks, fair baselines, statistical rigor, realistic workloads
**Runtime**: ~5 minutes total

**Run now**: `cargo bench --bench capsule_hash64_chain_bench`
