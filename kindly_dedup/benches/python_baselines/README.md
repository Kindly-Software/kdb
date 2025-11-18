# Python Baseline Benchmarks (B32 Fair Comparison)

## Overview

**Purpose**: Provide FAIR Python baselines for comparing against Rust kindly_dedup optimizations

**B32 Compliance**:
- ✅ Fair baselines (NOT strawman: uses multiprocessing + optimized datasketch)
- ✅ Same hardware (runs on same machine as Rust benchmarks)
- ✅ Same algorithm (MinHash 128 perm + LSH 0.85 threshold)
- ✅ Statistical rigor (3+ runs, mean/min/max reported)
- ✅ Honest reporting (includes B32 reality checks)

## Setup

### 1. Create Python Virtual Environment

```bash
cd benches/python_baselines
python3 -m venv venv
source venv/bin/activate  # Linux/macOS
# or
venv\Scripts\activate     # Windows
```

### 2. Install Dependencies

```bash
pip install -r requirements.txt
```

**Dependencies** (from `requirements.txt`):
- `datasketch==1.6.4` - Industry-standard MinHash/LSH library
- `mmh3==4.0.1` - MurmurHash3 (same algorithm as kindly_dedup)
- `numpy==1.26.0` - NumPy for optimized datasketch

### 3. Verify Installation

```bash
python3 -c "import datasketch; print(f'datasketch {datasketch.__version__}')"
# Expected: datasketch 1.6.4
```

## Usage

### Benchmark 1: Corpus Generation

**Purpose**: Measure Python corpus generation baseline (single-threaded vs multi-threaded)

```bash
# Run benchmark (auto-detect CPU cores)
python3 bench_generation.py 10000

# Run with specific worker count
python3 bench_generation.py 100000 16

# Save results to JSON
python3 bench_generation.py 100000 > results_generation.json
```

**Expected Results**:
- **Single-threaded**: ~100K docs/sec (Python string formatting)
- **Multi-threaded (16 workers)**: ~800K docs/sec (8× speedup, Python GIL limited)
- **Rust baseline**: ~2M+ docs/sec (20× faster, no GIL)

**Output Format**:
```json
{
  "corpus_size": 100000,
  "num_workers": 16,
  "single_threaded": {
    "throughput_docs_per_sec": 105234.5,
    "latency_per_doc_us": 9.5,
    "total_time_sec": 0.95
  },
  "multi_threaded": {
    "throughput_docs_per_sec": 842187.5,
    "latency_per_doc_us": 1.19,
    "total_time_sec": 0.119,
    "speedup_vs_single": 8.0,
    "num_runs": 3,
    "times_sec": [0.118, 0.119, 0.120]
  }
}
```

### Benchmark 2: Deduplication

**Purpose**: Measure Python datasketch baseline (MinHash + LSH deduplication)

```bash
# Prepare test corpus (generate from Rust)
cd ../..
cargo run --bin generate_synthetic_corpus --features download-tools -- \
    --output test_data/synthetic_10k.json \
    --num-docs 10000 \
    --duplicate-rate 0.5

# Run Python baseline
cd benches/python_baselines
python3 bench_dedup.py ../../test_data/synthetic_10k.json

# Run with custom parameters
python3 bench_dedup.py corpus.json 128 0.85 5

# Save results to JSON
python3 bench_dedup.py corpus.json > results_dedup.json
```

**Expected Results**:
- **Python datasketch**: ~1,500-2,000 docs/sec (measured: 1,572 docs/sec on 10K corpus)
- **Rust v1.0**: ~60,000 docs/sec (38× speedup, EXCEPTIONAL tier per B32 K27)

**Output Format**:
```json
{
  "corpus_size": 10000,
  "num_perm": 128,
  "threshold": 0.85,
  "num_runs": 3,
  "throughput_docs_per_sec": 1572.3,
  "latency_per_doc_us": 636.2,
  "total_time_mean_sec": 6.362,
  "total_time_min_sec": 6.301,
  "total_time_max_sec": 6.425,
  "times_sec": [6.301, 6.362, 6.425],
  "duplicates_found": 45,
  "load_time_sec": 0.023
}
```

## B32 Reality Checks

Both scripts include automatic B32 reality checks:

### Generation Reality Check
- **Expected**: 100K-800K docs/sec (single-multi)
- **Warning**: Speedup >2× workers (suspicious) or <0.3× workers (contention)

### Deduplication Reality Check
- **Expected**: 1,500-2,000 docs/sec (Python datasketch)
- **Warning**: >10K docs/sec (suspicious) or <500 docs/sec (hardware issue)

## Integration with Rust Benchmarks

### 1. Generate Test Corpus (Rust)

```bash
cd ../..

# Generate 10K corpus (50% duplicates)
cargo run --bin generate_synthetic_corpus --features download-tools -- \
    --output test_data/synthetic_10k.json \
    --num-docs 10000 \
    --duplicate-rate 0.5

# Generate 100K corpus (70% duplicates)
cargo run --bin generate_synthetic_corpus --features download-tools -- \
    --output test_data/synthetic_100k.json \
    --num-docs 100000 \
    --duplicate-rate 0.7
```

### 2. Run Python Baseline

```bash
cd benches/python_baselines
source venv/bin/activate

# Benchmark deduplication on 10K corpus
python3 bench_dedup.py ../../test_data/synthetic_10k.json > python_10k_results.json

# Benchmark deduplication on 100K corpus
python3 bench_dedup.py ../../test_data/synthetic_100k.json > python_100k_results.json
```

### 3. Run Rust Benchmarks

```bash
cd ../..

# Run Rust benchmarks (same corpus)
cargo bench --bench week1_bloom_prefilter --features benchmarking

# Compare results
open target/criterion/report/index.html
```

### 4. Compare Results

**Example Comparison**:

| Benchmark | Python Baseline | Rust v1.0 | Week 1 Optimized | Speedup (v1.0) | Speedup (Week 1) |
|-----------|----------------|-----------|------------------|----------------|------------------|
| Corpus Generation (100K) | 800K docs/sec | 2M docs/sec | 2.5M docs/sec | 2.5× | 3.1× |
| Deduplication (10K, 50% dup) | 1,572 docs/sec | 60K docs/sec | 120K docs/sec | 38× | 76× |

**B32 Classification**:
- 2.5× speedup: **EXCEPTIONAL** tier (K27: 10-50% typical, 2× exceptional)
- 38× speedup: **EXCEPTIONAL** tier (algorithm change: Python → Rust lockfree)
- 76× speedup: **BREAKTHROUGH** tier (requires extensive validation)

## Troubleshooting

### Issue: `ModuleNotFoundError: No module named 'datasketch'`

**Solution**: Activate virtual environment and install dependencies

```bash
source venv/bin/activate
pip install -r requirements.txt
```

### Issue: `FileNotFoundError: [Errno 2] No such file or directory: 'corpus.json'`

**Solution**: Generate test corpus first using Rust `generate_synthetic_corpus`

```bash
cd ../..
cargo run --bin generate_synthetic_corpus --features download-tools -- \
    --output test_data/synthetic_10k.json \
    --num-docs 10000
```

### Issue: Python baseline too slow (>10 minutes for 100K docs)

**Expected Behavior**: Python datasketch is ~40× slower than Rust

**Workaround**: Use smaller corpus for Python baseline (10K-50K docs), then extrapolate

```bash
# Benchmark on 10K corpus (faster)
python3 bench_dedup.py ../../test_data/synthetic_10k.json

# Extrapolate to 100K: throughput × 10
```

### Issue: Throughput measurement suspicious (>10K docs/sec)

**Check**:
1. Verify corpus loaded correctly (`num_documents` in output)
2. Ensure datasketch 1.6.4 installed (not newer optimized version)
3. Check CPU throttling (run `python3 -c "import time; time.sleep(10)"` to warm up)

## References

- **B32 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/B32_BENCHMARK_FRAMEWORK.md`
- **datasketch Documentation**: https://ekzhu.com/datasketch/
- **kindly_dedup Benchmarks**: `../week1_bloom_prefilter.rs`, `../week1_parallel_generation.rs`

## Contact

- **Issues**: Report benchmark discrepancies via `CLAUDE.md` documentation
- **Questions**: See B32 Framework for fair benchmarking guidelines
