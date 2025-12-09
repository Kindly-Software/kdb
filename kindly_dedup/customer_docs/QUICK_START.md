# Quick Start Guide

Get started with Kindly Dedup in 5 minutes.

## Prerequisites

- **Operating System**: Linux, macOS, or Windows
- **RAM**: Minimum 2 GB, recommended 8 GB+
- **CPU**: Minimum 2 cores, recommended 8+ cores
- **Disk Space**: 1 GB for installation, plus storage for your dataset

## Installation

### Option 1: Download Pre-built Binary (Recommended)

Download the latest release for your platform:

- **Linux (x86_64)**: `kindly-dedup-linux-x64.tar.gz`
- **macOS (Intel)**: `kindly-dedup-macos-intel.tar.gz`
- **macOS (Apple Silicon)**: `kindly-dedup-macos-arm64.tar.gz`
- **Windows (x86_64)**: `kindly-dedup-windows-x64.zip`

Extract and run:

```bash
# Linux/macOS
tar -xzf kindly-dedup-*.tar.gz
cd kindly-dedup
./kindly-dedup --version

# Windows
# Extract ZIP file and run kindly-dedup.exe in Command Prompt
kindly-dedup.exe --version
```

### Option 2: Build from Source (Advanced)

Requires Rust toolchain (1.70+):

```bash
cargo install kindly-dedup
kindly-dedup --version
```

## Basic Usage

### Step 1: Prepare Your Data

Create a JSONL file with your documents (one document per line):

```jsonl
{"id": 1, "text": "The quick brown fox jumps over the lazy dog."}
{"id": 2, "text": "A fast brown fox leaps over a sleepy dog."}
{"id": 3, "text": "The quick brown fox jumps over the lazy dog."}
```

Save as `documents.jsonl`.

### Step 2: Run Deduplication

```bash
kindly-dedup deduplicate \
  --input documents.jsonl \
  --output clusters.json \
  --threshold 0.85
```

**Parameters**:
- `--input`: Path to input file (JSONL, CSV, or plain text)
- `--output`: Path to output file (JSON format)
- `--threshold`: Similarity threshold (0.0-1.0, higher = stricter matching)

### Step 3: Review Results

The output file contains clusters of duplicate documents:

```json
{
  "clusters": [
    {
      "representative_id": 1,
      "duplicate_ids": [3],
      "similarity": 1.0
    },
    {
      "representative_id": 2,
      "duplicate_ids": [],
      "similarity": 0.92
    }
  ],
  "stats": {
    "total_documents": 3,
    "unique_documents": 2,
    "duplicate_documents": 1,
    "deduplication_ratio": 0.33
  }
}
```

## Expected Performance

On a typical modern laptop (8 cores, 16 GB RAM):

- **100K documents**: ~2 seconds
- **1M documents**: ~17 seconds
- **10M documents**: ~30 seconds (multi-threaded)

Memory usage scales efficiently:
- **100K documents**: ~500 MB
- **1M documents**: ~1.5 GB
- **10M documents**: ~3.5 GB

## Common Use Cases

### Exact Duplicates Only

Use a high threshold (0.99-1.0):

```bash
kindly-dedup deduplicate --input data.jsonl --output results.json --threshold 0.99
```

### Near Duplicates (Fuzzy Matching)

Use a moderate threshold (0.80-0.90):

```bash
kindly-dedup deduplicate --input data.jsonl --output results.json --threshold 0.85
```

### Large Datasets (Persistent Mode)

For datasets larger than available RAM:

```bash
kindly-dedup deduplicate \
  --input large_dataset.jsonl \
  --output results.json \
  --threshold 0.85 \
  --persistent \
  --storage-path ./dedup_storage
```

## Next Steps

- Read the [User Guide](USER_GUIDE.md) for comprehensive CLI reference
- See [API Reference](API_REFERENCE.md) to use the HTTP API
- Check [Deployment Guide](DEPLOYMENT.md) for production setup
- Review [FAQ](FAQ.md) for best practices and tuning tips

## Troubleshooting

**Error: "Out of memory"**
- Use `--persistent` mode for large datasets
- Reduce batch size with `--batch-size 1000`
- See [TROUBLESHOOTING.md](TROUBLESHOOTING.md)

**Slow performance**
- Increase parallelism with `--threads <num_cores>`
- Enable GPU acceleration if available (see User Guide)
- Verify input file format is correct

For more help, see [TROUBLESHOOTING.md](TROUBLESHOOTING.md) or contact support@kindly.ai.
