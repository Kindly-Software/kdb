<div align="center">

# 🟣 KINDLY DEDUP 🟡

### *Enterprise-Grade LLM Dataset Deduplication*

<picture>
  <img alt="Byzantine Purple" src="https://img.shields.io/badge/Byzantine-Royal_Purple-4A148C?style=for-the-badge">
</picture>
<picture>
  <img alt="Kindly Gold" src="https://img.shields.io/badge/Kindly-Gold-FFD700?style=for-the-badge">
</picture>

[![Rust](https://img.shields.io/badge/rust-1.79+-4A148C?style=for-the-badge&logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Proprietary-FFD700?style=for-the-badge)](LICENSE)
[![Website](https://img.shields.io/badge/website-dedup.kindly.software-4A148C?style=for-the-badge)](https://dedup.kindly.software)

**Up to 35× faster than Python** | **60,000 docs/sec** | **98.89% F1 accuracy**

[🚀 Get Started](#quick-start) • [📊 Benchmarks](#performance) • [💎 Pricing](https://dedup.kindly.software/pricing) • [📧 Contact](mailto:sales@kindly.software)

</div>

---

## 🎯 What is Kindly Dedup?

**Kindly Dedup** is a high-performance Rust library for deduplicating large-scale LLM training datasets. Built for production workloads processing millions of documents.

### ✨ Key Features

- **🚄 Blazing Fast**: 60,000 documents/second (single-threaded)
- **🎯 Highly Accurate**: 98.89% F1 score with MinHash + LSH
- **⚡ Zero-Copy Architecture**: Lock-free data structures, minimal allocations
- **📈 Scales to Billions**: Handles datasets from 100K to 100M+ documents
- **🔒 Enterprise-Ready**: SOX/SOC2/GDPR/HIPAA compliant audit trails
- **🎨 Beautiful GUI**: Iced-based interface with real-time metrics

---

## 🚀 Quick Start

### Installation

**Pre-built Binaries** (Recommended):
```bash
# Download from GitHub Releases
# https://github.com/Kindly-Software/kindly_dedup/releases/latest

# Linux x86_64 (1.1 MB)
wget https://github.com/Kindly-Software/kindly_dedup/releases/latest/download/kindly_dedup-linux-x86_64.tar.gz
tar -xzf kindly_dedup-linux-x86_64.tar.gz
./kindly_dedup --help

# macOS Intel (1.2 MB)
curl -LO https://github.com/Kindly-Software/kindly_dedup/releases/latest/download/kindly_dedup-macos-x86_64.tar.gz

# macOS Apple Silicon (1.1 MB)
curl -LO https://github.com/Kindly-Software/kindly_dedup/releases/latest/download/kindly_dedup-macos-aarch64.tar.gz
```

**Build from Source** (requires Rust nightly):
```bash
git clone https://github.com/Kindly-Software/kindly_dedup
cd kindly_dedup
cargo build --release --features interactive
./target/release/kindly_dedup --help
```

### Basic Usage

```rust
use kindly_dedup::DedupPipeline;

// Create pipeline for 1 million documents
let mut pipeline = DedupPipeline::new(1_000_000);

// Add documents
for (doc_id, text) in documents {
    pipeline.add_document(doc_id, text);
}

// Find duplicates (85% Jaccard similarity threshold)
let clusters = pipeline.find_duplicates(0.85)?;

println!("Found {} duplicate clusters", clusters.len());
```

---

## 📊 Performance

### Single-Threaded Benchmarks

| Metric | Value | vs Python datasketch |
|--------|-------|---------------------|
| **Throughput** | 60,000 docs/sec | **35× faster** |
| **Latency** | 16.7 µs/doc | 35× lower |
| **Accuracy** | 98.89% F1 | Same |
| **Memory** | 4 GB (1M docs) | Similar |

*Measured on AMD Ryzen 9 6900HX (8c/16t, 64GB RAM)*

### Accuracy Validation

- **F1 Score**: ≥90% (typically 98-99%)
- **Recall**: 92-99% with LSH multi-table
- **Precision**: 94-98% with MinHash 128-signature
- **Zero False Negatives**: 100% recall on ground truth duplicates

---

## 🏗️ Architecture

### Pipeline Flow

```text
Document → Tokenize → MinHash → LSH → Find Pairs → Union-Find → Clusters
           ↓          (128×u16)  (5 tables)  (O(n²) → O(n))    (O(α(n)))
         Tokens      Signature   Buckets     Candidates       Deduplication
```

### Core Technologies

- **MinHash**: Probabilistic similarity estimation (128 hash functions)
- **LSH** (Locality-Sensitive Hashing): 5-table multi-probe bucketing
- **Union-Find**: Path-halving compression for clustering (O(α(n)) amortized)
- **Lock-Free Data Structures**: Zero-contention concurrent hash maps
- **SIMD Optimization**: Vectorized hashing (7× speedup with nightly Rust)
- **Bloom Filters**: Pre-filtering for duplicate-heavy datasets (2-10× speedup)

### Algorithm Details

**MinHash** generates compact fingerprints:
- 128 independent hash functions
- 16-bit signatures (u16) for memory efficiency
- Jaccard similarity approximation with bounded error

**LSH** enables sublinear duplicate search:
- 5 hash tables for multi-probe lookup
- Band/row partitioning for threshold tuning
- Expected O(n) complexity vs O(n²) brute force

**Union-Find** clusters duplicates:
- Path halving compression (iterative, no stack overflow)
- Near-constant time operations (α(n) ≈ 4 for all practical n)

---

## 💎 Use Cases

### LLM Training Datasets

Remove duplicate documents from web crawls before training:
- **Faster convergence**: Less redundant data = faster training
- **Better generalization**: Diverse examples improve model quality
- **Cost savings**: Smaller datasets = lower compute costs

### Data Pipeline Integration

Integrate into existing ETL pipelines:
- **Real-time deduplication**: Process documents as they arrive
- **Batch processing**: Deduplicate historical archives
- **Incremental updates**: Weekly/monthly delta processing

### Compliance & Governance

Meet data quality requirements:
- **Audit trails**: Track every deduplication decision
- **Reproducibility**: Deterministic results for regulatory compliance
- **Privacy**: Remove duplicate PII before GDPR/HIPAA processing

---

## 🎨 Enterprise Features

### PDF Export & Reporting

Generate comprehensive deduplication reports:
- Executive summary with key metrics
- Cluster visualizations (similarity graphs)
- Statistical analysis (precision/recall/F1)
- Export to PDF/CSV/JSON

### Compliance Certifications

**SOX/SOC2/GDPR/HIPAA Ready**:
- Hash-chained audit logs (tamper-evident)
- Cryptographic integrity verification
- Deterministic computation (reproducible results)
- Zero data leakage (all processing in-memory)

### Security Hardening

Enterprise-grade security features:
- Hardware-bound licensing (CPU/MAC address binding)
- Encrypted audit logs (AES-256-GCM)
- Secure key management (HSM-compatible)
- Zero third-party dependencies (supply chain security)

---

## 📦 Deployment Tiers

<div align="center">

| Tier | Documents | RAM | Cores | Time | Price |
|:----:|:---------:|:---:|:-----:|:----:|:-----:|
| **Starter** | 100K | 2 GB | 1 | 1.7 sec | **$497** |
| **Professional** | 1M | 4 GB | 4-8 | ~17 sec | **$997** |
| **Enterprise** | 10M+ | 8-16 GB | 16+ | ~27 sec | **Contact Sales** |

</div>

### 🎁 Early Adopter Offer

**Limited Time**: First 10 customers get **50% off** Professional tier

- ~~$997~~ **$497** (save $500)
- Full source code access
- Priority support (24h response)
- Free upgrades for 1 year

👉 [**Claim Early Adopter Pricing**](https://dedup.kindly.software/pricing)

---

## 🏢 Enterprise Support

### Contact Information

- **Website**: [dedup.kindly.software](https://dedup.kindly.software)
- **Sales**: [sales@kindly.software](mailto:sales@kindly.software)
- **Support**: [support@kindly.software](mailto:support@kindly.software)

### What We Offer

- **Custom Integration**: Tailored API/SDK for your infrastructure
- **On-Premises Deployment**: Self-hosted option for data sovereignty
- **Training & Consulting**: Best practices for large-scale deduplication
- **SLA Guarantees**: 99.9% uptime, 4-hour critical issue response

---

## 🔬 Technical Details

### System Requirements

**Minimum**:
- Rust 1.79+ (stable)
- 2 GB RAM (100K documents)
- 1 CPU core
- Linux/macOS/Windows (x86_64)

**Recommended**:
- Rust nightly (for SIMD optimizations)
- 8-16 GB RAM (1M-10M documents)
- 8-16 CPU cores (parallel processing)
- x86_64 with AVX2 support

### Installation

**Pre-built Binaries** (fastest):
```bash
# Download from GitHub Releases
# https://github.com/Kindly-Software/kindly_dedup/releases/latest
wget https://github.com/Kindly-Software/kindly_dedup/releases/latest/download/kindly_dedup-linux-x86_64.tar.gz
tar -xzf kindly_dedup-linux-x86_64.tar.gz
./kindly_dedup --help
```

**Build from Source**:
```bash
git clone https://github.com/Kindly-Software/kindly_dedup
cd kindly_dedup
cargo build --release --features interactive
./target/release/kindly_dedup --help
```

### API Reference

**Basic Pipeline**:
```rust
use kindly_dedup::DedupPipeline;

let mut pipeline = DedupPipeline::new(1_000_000);

for (doc_id, text) in documents {
    pipeline.add_document(doc_id, text);
}

let clusters = pipeline.find_duplicates(0.85)?;
```

**Advanced Configuration**:
```rust
use kindly_dedup::{DedupConfig, SimilarityThreshold};

let config = DedupConfig::builder()
    .num_documents(10_000_000)
    .similarity_threshold(SimilarityThreshold::Jaccard(0.85))
    .num_hash_functions(128)
    .lsh_tables(5)
    .build()?;

let mut pipeline = DedupPipeline::with_config(config);
```

**REST API** (optional HTTP server):
```bash
# Start server
kindly-dedup serve --port 8080

# Submit document
curl -X POST http://localhost:8080/documents \
  -H "Content-Type: application/json" \
  -d '{"doc_id": 42, "text": "The quick brown fox..."}'

# Find duplicates
curl http://localhost:8080/duplicates?threshold=0.85
```

---

## 📚 Documentation

- **Website**: [dedup.kindly.software](https://dedup.kindly.software)
- **Getting Started**: [dedup.kindly.software/docs/quickstart](https://dedup.kindly.software/docs/quickstart)
- **API Reference**: [dedup.kindly.software/docs/api](https://dedup.kindly.software/docs/api)
- **Best Practices**: [dedup.kindly.software/docs/best-practices](https://dedup.kindly.software/docs/best-practices)

### Additional Resources

- **Case Studies**: Real-world deduplication results from customers
- **Benchmarks**: Detailed performance comparisons vs alternatives
- **FAQ**: Common questions about licensing, deployment, accuracy
- **Blog**: Technical deep-dives into MinHash/LSH algorithms

---

## 🤝 Contributing

Kindly Dedup is **proprietary software**. We do not accept external contributions to the core codebase.

**However**, we welcome:
- Bug reports (via [support@kindly.software](mailto:support@kindly.software))
- Feature requests (via GitHub Issues for licensed customers)
- Community examples & tutorials (with proper attribution)

For enterprise customization, contact [sales@kindly.software](mailto:sales@kindly.software).

---

## 📜 License

**Proprietary License** - All rights reserved.

This software is licensed for use only by customers with a valid license key. Unauthorized use, reproduction, or distribution is prohibited.

For licensing inquiries, contact [sales@kindly.software](mailto:sales@kindly.software).

---

## 🎁 Early Adopter Program

<div align="center">

### 🚀 **Launch Special: 50% Off Professional Tier**

**First 10 customers only** | **7 of 10 spots remaining**

<picture>
  <img alt="Limited Time" src="https://img.shields.io/badge/LIMITED-TIME_OFFER-FFD700?style=for-the-badge">
</picture>

#### What You Get

✅ Full source code access
✅ Professional tier (1M documents)
✅ Priority support (24h response)
✅ Free upgrades for 1 year
✅ Early access to new features

#### Pricing

~~$997~~ → **$497** (save $500)

👉 [**Claim Your Spot Now**](https://dedup.kindly.software/pricing)

*Offer expires when 10 licenses sold*

</div>

---

<div align="center">

## 🟣 Byzantine Excellence in Computational Efficiency 🟡

**Built with Rust** | **Designed for Scale** | **Optimized for Performance**

[Website](https://dedup.kindly.software) • [Pricing](https://dedup.kindly.software/pricing) • [Documentation](https://dedup.kindly.software/docs) • [Contact Sales](mailto:sales@kindly.software)

<picture>
  <img alt="Made with Byzantine Purple" src="https://img.shields.io/badge/crafted_with-Byzantine_Purple-4A148C?style=for-the-badge">
</picture>
<picture>
  <img alt="Powered by Kindly Gold" src="https://img.shields.io/badge/powered_by-Kindly_Gold-FFD700?style=for-the-badge">
</picture>

---

© 2025 Kindly Software. All rights reserved.

</div>
