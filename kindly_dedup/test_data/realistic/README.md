# Realistic LLM Training Datasets

**B32 Compliance**: Real datasets with documented provenance (NO synthetic data for sales claims)

## Purpose

These datasets enable realistic benchmarking of `kindly_dedup` against industry-standard LLM training data. All datasets have:
- **Real source data** (NOT synthetic)
- **Documented provenance** (source, date, version, URL)
- **SHA-256 integrity verification**
- **Manifest tracking** (JSON metadata)

## Available Datasets

### 1. Common Crawl (CC-MAIN-2024-33)

**Status**: ✅ Implemented

**Details**:
- **Source**: https://commoncrawl.org/
- **Format**: WET (Web Extracted Text) JSONL
- **Size**: 100K-10M documents (~0.5-50GB)
- **Purpose**: Main validation dataset, high duplication rate (90-95%)
- **Download**:
  ```bash
  cargo run --bin download_corpus --features download-tools -- \
    --source commoncrawl --limit 100000 \
    --output test_data/realistic/commoncrawl_100k.json \
    --generate-manifest
  ```

**Why Common Crawl**:
- Industry standard for LLM pre-training (GPT-3, LLaMA, PaLM)
- High natural duplication (90-95% near-duplicates)
- Large scale (petabytes available)
- Publicly accessible and well-documented

### 2. The Pile (EleutherAI)

**Status**: 🔧 Planned

**Details**:
- **Source**: https://pile.eleuther.ai/
- **Format**: JSONL (one document per line)
- **Size**: 825GB total, subsets available
- **Purpose**: Diverse training data (22 sources: books, code, science papers, etc.)
- **Duplication**: Lower than Common Crawl (~30-50%)

**Implementation**:
```bash
# TODO: Implement Pile downloader
cargo run --bin download_corpus --features download-tools -- \
  --source pile --limit 1000000 \
  --output test_data/realistic/pile_1m.json \
  --generate-manifest
```

**Why The Pile**:
- Diverse high-quality sources
- Used in GPT-Neo, GPT-J training
- Well-documented composition
- Lower duplication rate (accuracy testing)

### 3. C4 (Colossal Clean Crawled Corpus)

**Status**: 🔧 Planned

**Details**:
- **Source**: https://huggingface.co/datasets/allenai/c4
- **Format**: Parquet/JSONL (Hugging Face datasets)
- **Size**: 305GB (cleaned Common Crawl)
- **Purpose**: Clean web text, moderate duplication (~70-80%)

**Implementation**:
```bash
# Requires Hugging Face API
cargo run --bin download_corpus --features download-tools -- \
  --source c4 --limit 1000000 \
  --output test_data/realistic/c4_1m.json \
  --generate-manifest
```

**Why C4**:
- T5, Flan-T5, and many others trained on C4
- Cleaned and filtered (higher quality than raw Common Crawl)
- Moderate duplication (between Pile and Common Crawl)

### 4. RedPajama (LLaMA Training Data Replica)

**Status**: 🔧 Planned

**Details**:
- **Source**: https://huggingface.co/datasets/togethercomputer/RedPajama-Data-1T
- **Format**: Parquet/JSONL (Hugging Face datasets)
- **Size**: 1.2TB total
- **Purpose**: Replica of LLaMA training data, mixed duplication

**Implementation**:
```bash
# Requires Hugging Face API
cargo run --bin download_corpus --features download-tools -- \
  --source redpajama --limit 1000000 \
  --output test_data/realistic/redpajama_1m.json \
  --generate-manifest
```

**Why RedPajama**:
- Exact replica of LLaMA training data mix
- Production-grade composition
- Real-world duplication patterns

## Dataset Manifests

Each dataset has an accompanying `.manifest.json` file with:
- **source**: Dataset name and version
- **url**: Original source URL
- **downloaded**: ISO 8601 timestamp
- **document_count**: Number of documents
- **size_bytes**: Total file size
- **sha256**: Integrity hash
- **provenance**: Version, subset, and modification notes

Example manifest:
```json
{
  "source": "Common Crawl (CC-MAIN-2024-33)",
  "url": "https://data.commoncrawl.org/crawl-data/CC-MAIN-2024-33/wet.paths.gz",
  "downloaded": "2025-10-29T18:00:00Z",
  "document_count": 100000,
  "size_bytes": 536870912,
  "sha256": "abc123def456...",
  "provenance": "CC-MAIN-2024-33 crawl, 100000 documents, unmodified"
}
```

## Usage

### Download All Datasets

```bash
# Run automated download script (when implemented)
./scripts/download_all_datasets.sh
```

### Verify Integrity

```bash
# Verify SHA-256 hashes
cargo test --test dataset_manager_tests -- --nocapture
```

### Use in Benchmarks

```bash
# Benchmark with realistic data
cargo bench --bench dedup_bench -- --features download-tools
```

## B32 Compliance

All datasets meet B32 framework requirements:

✅ **Real Data**: Industry-standard LLM training datasets (NOT synthetic)
✅ **Documented Provenance**: Source, URL, version, timestamp tracked
✅ **Integrity Verification**: SHA-256 hashes for all datasets
✅ **Reproducibility**: Exact dataset versions documented
✅ **Fair Baselines**: Compare against published benchmarks (Python datasketch, GPU FED)

## Benchmarking Strategy

### Phase 1: Common Crawl Validation (Current)
- **100K documents**: Quick validation (<5 minutes)
- **1M documents**: Standard benchmark (30-60 minutes)
- **10M documents**: Stress test (6-12 hours)

### Phase 2: Diverse Dataset Testing (Planned)
- **The Pile**: Low duplication accuracy test
- **C4**: Moderate duplication test
- **RedPajama**: Production mix test

### Phase 3: Comparative Benchmarking (Planned)
- **Python datasketch**: Baseline comparison
- **GPU FED**: State-of-art comparison
- **kindly_dedup v1.1**: Our implementation

## Storage Requirements

| Dataset | Size | Documents | Storage |
|---------|------|-----------|---------|
| Common Crawl 100K | ~0.5GB | 100,000 | Minimal |
| Common Crawl 1M | ~5GB | 1,000,000 | Standard |
| Common Crawl 10M | ~50GB | 10,000,000 | Large |
| The Pile 1M | ~10GB | 1,000,000 | Standard |
| C4 1M | ~8GB | 1,000,000 | Standard |
| RedPajama 1M | ~10GB | 1,000,000 | Standard |

**Total**: ~83GB for full benchmark suite

## References

1. **Common Crawl**: https://commoncrawl.org/
2. **The Pile**: https://pile.eleuther.ai/ (Gao et al., 2020)
3. **C4**: https://www.tensorflow.org/datasets/catalog/c4 (Raffel et al., 2020)
4. **RedPajama**: https://www.together.ai/blog/redpajama (Together Computer, 2023)
5. **B32 Framework**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/B32_BENCHMARK_FRAMEWORK.md`

## License & Attribution

All datasets retain their original licenses:
- **Common Crawl**: Public domain (CC0)
- **The Pile**: MIT License
- **C4**: Apache 2.0
- **RedPajama**: Apache 2.0

Please cite original sources when publishing benchmarks using these datasets.
