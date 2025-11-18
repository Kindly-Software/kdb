# v1.1 Compound Benchmark Suite

**Status**: Implementation Complete  
**Goal**: Validate 204× compound speedup via tier stacking (T1+T2+T4+T10)  
**Framework Compliance**: B32, Q34, IMPL-2 V3.1

## Usage

### Run Benchmarks

```bash
# Full suite
cargo bench --bench v1_1_compound --features benchmarking

# With parallel (requires parallel-dedup feature)
cargo bench --bench v1_1_compound --features benchmarking,parallel-dedup
```

### Analyze Results

```bash
cargo run --bin v1_1_compound_analysis --features benchmarking
```

## Expected Results

### Theoretical Compound (204×)

- Bloom (T10): 2.0×
- SIMD (T2): 7.1×
- Parallel (T4): 9.6×
- Lockfree (T1): 1.5×
- **THEORETICAL**: 204× (2 × 7.1 × 9.6 × 1.5)

### Expected Efficiency (B32 K39)

- 60% (conservative): 122× actual
- 70% (typical): 143× actual
- 80% (optimistic): 164× actual

## Component Benchmarks

1. **baseline_v1_0**: No optimizations
2. **bloom_only**: Bloom pre-filter (T10)
3. **simd_only**: SIMD hashing (T2)
4. **parallel_only**: Parallel processing (T4, 1-16 threads)
5. **lockfree_only**: Lockfree buckets (T1)
6. **compound_all**: ALL optimizations (T1+T2+T4+T10)

## B32 Compliance

- ✅ K1: Fair baselines (v1.0 production code)
- ✅ K6: Statistical rigor (95% CI, 1000+ iterations)
- ✅ K27: Component isolation (6 benchmarks)
- ✅ K39: Compound efficiency (60-80% expected)

## References

- SESSION_HANDOFF.md: v1.1 compound details
- IMPL-2 V3.1: Innovation stacking
- B32 Framework: K1-K70 reality checks
- Q34 Framework: Audit trails
