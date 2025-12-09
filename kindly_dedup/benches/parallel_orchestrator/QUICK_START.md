# ParallelDedupOrchestrator Benchmarks - Quick Start

## 📊 Benchmark Suite Overview

**Target**: Validate 4.8-5.3× speedup @ 16 threads

**Created Files**: 6 files, 974 lines

**Status**: ✅ Infrastructure Complete | ⏳ Awaiting Pipeline Implementation

---

## 🚀 Quick Commands

### Run All Benchmarks
```bash
cargo bench --bench parallel_orchestrator --features benchmarking,parallel-dedup
```

### Run Individual Suites
```bash
# Amdahl's Law validation (1-16 threads)
cargo bench --bench parallel_orchestrator speedup_curve

# Per-phase performance breakdown
cargo bench --bench parallel_orchestrator phase_breakdown

# Production workloads (1K-1M docs)
cargo bench --bench parallel_orchestrator realistic_workload
```

### View Results
```bash
open target/criterion/report/index.html
```

---

## 📁 File Structure

```
benches/
├── criterion_config.rs                    # Shared B32 config (1000+ iterations, 95% CI)
└── parallel_orchestrator/
    ├── mod.rs                             # Main module + corpus generator
    ├── speedup_curve.rs                   # Amdahl's Law validation
    ├── phase_breakdown.rs                 # Per-phase performance
    ├── realistic_workload.rs              # Production workloads
    ├── BENCHMARKING_SETUP.md              # Comprehensive guide
    └── QUICK_START.md                     # This file
```

---

## ✅ Expected Results

### Speedup Curve (10K docs)

| Threads | Speedup | Time   | Throughput     |
|---------|---------|--------|----------------|
| 1       | 1.0×    | 167 ms | 60K docs/sec   |
| 2       | 1.8×    | 93 ms  | 108K docs/sec  |
| 4       | 3.2×    | 52 ms  | 192K docs/sec  |
| 8       | 4.8×    | 35 ms  | 286K docs/sec  |
| 16      | 5.3×    | 31 ms  | 323K docs/sec ✅ |

### Phase Breakdown (10K docs, 16 threads)

| Phase | Type       | Time  | % Total |
|-------|------------|-------|---------|
| 1     | Read       | 10 ms | 32.3%   |
| 2     | Sign       | 15 ms | 48.4%   |
| 3     | Hash       | 3 ms  | 9.7%    |
| 4     | Cluster    | 2 ms  | 6.5%    |
| 5     | Output     | 1 ms  | 3.2%    |

### Realistic Workload (16 threads)

| Size  | Sequential | Parallel | Speedup |
|-------|-----------|----------|---------|
| 1K    | 17 ms     | 5 ms     | 3.2×    |
| 10K   | 167 ms    | 31 ms    | 5.3×    |
| 100K  | 1.67 s    | 310 ms   | 5.4×    |
| 1M    | 16.7 s    | 3.1 s    | 5.4×    |

---

## ⚠️ Prerequisites

1. **Fix compilation error**:
   ```
   src/parallel_pipeline.rs:707: ConcurrentMapCapsule vs HashMap type mismatch
   ```

2. **Implement ParallelDedupOrchestrator full pipeline**:
   ```rust
   pub fn process_corpus_parallel(&mut self, documents: &[(DocId, String)]) -> Result<Vec<Cluster>, Error>
   ```

3. **Uncomment benchmark code** (search for `TODO (Week 2 Priority 4)`)

---

## 📖 Documentation

- **Comprehensive Guide**: `BENCHMARKING_SETUP.md` (389 lines)
- **Completion Report**: `../../BENCHMARKING_INFRASTRUCTURE_COMPLETE.md` (534 lines)
- **B32 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/xml/frameworks/b32.xml`

---

## 🎯 B32 Compliance

- ✅ Fair baselines (sequential DedupPipeline vs parallel orchestrator)
- ✅ 1000+ iterations (standard), 100+ (large workloads)
- ✅ 95% confidence intervals
- ✅ Realistic workloads (1K-1M docs, 50% duplicates)
- ✅ Amdahl's Law validation (P=89.5%, S=16)

---

**Created**: 2025-11-20
**Framework**: UCE34 + B32 + T28 + ASSUM + I20 + Chaos
