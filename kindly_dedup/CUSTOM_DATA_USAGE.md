# Custom Data Usage Guide

**Quick Start**: Run deduplication on your own datasets

---

## Installation

```bash
# Build with benchmarking features
cd /home/samuel/Primitives/kindly_dedup
cargo build --release --bin client_demo --features benchmarking

# Or with META_CAPSULE protection
CUSTOMER_ID=demo-$(uuidgen) cargo build --release --bin client_demo --features "meta-capsule,benchmarking"
```

Binary location: `target/release/client_demo`

---

## Usage

### Standard Demo (No Arguments)
```bash
./target/release/client_demo
```

Runs original 3-tier validation:
- Tier 1: 100K docs (accuracy validation, ~17 min)
- Tier 2: 1M docs (scale demonstration, ~17 sec)
- Tier 3: 10M docs (massive scale, ~3 min, optional)

### Custom Data
```bash
# Basic usage
./target/release/client_demo --custom-data my_corpus.txt

# With custom threshold
./target/release/client_demo --custom-data corpus.txt --threshold 0.90

# Save results to JSON
./target/release/client_demo --custom-data corpus.txt --output results.json

# Full example
./target/release/client_demo \
  --custom-data /path/to/train_data.txt \
  --threshold 0.85 \
  --output /path/to/dedup_results.json
```

### Help
```bash
./target/release/client_demo --help
```

---

## Command-Line Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--custom-data <FILE>` | `-d` | Path to custom data file | None (runs standard demo) |
| `--threshold <FLOAT>` | `-t` | Jaccard similarity threshold (0.0-1.0) | 0.85 |
| `--output <FILE>` | `-o` | Save results to JSON file | None (console only) |
| `--help` | `-h` | Show help message | - |

---

## File Formats

### Plain Text
One document per line:
```
First document text here
Second document text here
Third document text here
```

**Rules**:
- UTF-8 encoding
- One document per line
- Empty lines skipped
- Line breaks within documents not supported

### JSONL (JSON Lines)
One JSON object per line:
```json
{"id": 0, "text": "First document"}
{"id": 1, "text": "Second document"}
{"id": 2, "text": "Third document"}
```

**Rules**:
- UTF-8 encoding
- Simple format: `{"id": N, "text": "..."}`
- One JSON object per line
- Malformed lines skipped with warning

---

## Examples

### Example 1: Basic Deduplication
```bash
# Create test file
cat > my_docs.txt << 'EOF'
Machine learning is amazing
Artificial intelligence rocks
Machine learning is amazing
Deep learning is powerful
Artificial intelligence rocks
Neural networks are cool
EOF

# Run deduplication
./target/release/client_demo --custom-data my_docs.txt

# Expected output:
# - 6 documents loaded
# - 2 duplicate pairs found
# - 2 clusters
```

### Example 2: Custom Threshold
```bash
# Lower threshold = more aggressive deduplication
./target/release/client_demo --custom-data my_docs.txt --threshold 0.70

# Higher threshold = more conservative
./target/release/client_demo --custom-data my_docs.txt --threshold 0.95
```

### Example 3: Save Results
```bash
./target/release/client_demo \
  --custom-data large_corpus.txt \
  --threshold 0.85 \
  --output results.json

# Check results
cat results.json
```

### Example 4: JSONL Format
```bash
# Create JSONL file
cat > my_docs.jsonl << 'EOF'
{"id": 1, "text": "First document"}
{"id": 2, "text": "Second document"}
{"id": 3, "text": "First document"}
EOF

# Run deduplication
./target/release/client_demo --custom-data my_docs.jsonl
```

---

## Output

### Console Output
```
═══════════════════════════════════════════════════════════
  CUSTOM DATA DEDUPLICATION
═══════════════════════════════════════════════════════════

Loading custom data from: my_docs.txt
├─ Loaded 1000 documents in 0.05 seconds
└─ Format: Plain text

Running deduplication pipeline...
├─ Threshold: 0.85
└─ Documents: 1000

├─ Pipeline time: 0.02 seconds
├─ Throughput: 50000 docs/sec
└─ Clusters found: 87

Result: ✓ DEDUPLICATION COMPLETE
├─ Unique documents: 913
└─ Duplicate documents: 87

═══════════════════════════════════════════════════════════
  CUSTOM DATA SUMMARY
═══════════════════════════════════════════════════════════

FILE INFORMATION:
  Path: my_docs.txt
  Documents: 1000

PERFORMANCE:
  Load time: 0.05 seconds
  Pipeline time: 0.02 seconds
  Throughput: 50000 docs/sec

DEDUPLICATION RESULTS:
  Threshold: 0.85
  Clusters found: 87

Total time: 0.07 seconds

BASELINE COMPARISON:
  Python datasketch: ~1,572 docs/sec (measured)
  kindly_dedup: 50000 docs/sec
  Speedup: 31.8×
```

### JSON Output (`--output` flag)
```json
{
  "file_path": "my_docs.txt",
  "timestamp": 1730332800,
  "doc_count": 1000,
  "load_time_secs": 0.050,
  "pipeline_time_secs": 0.020,
  "throughput_docs_per_sec": 50000,
  "cluster_count": 87,
  "threshold": 0.85
}
```

---

## Performance

### Expected Throughput
- **Small files** (<10K docs): 40-60K docs/sec
- **Medium files** (10K-100K docs): 50-70K docs/sec
- **Large files** (100K-1M docs): 55-65K docs/sec
- **Very large files** (1M+ docs): 50-60K docs/sec

### Speedup vs Python datasketch
- **Baseline**: Python datasketch ~1,572 docs/sec
- **kindly_dedup**: 50-70K docs/sec
- **Speedup**: **30-45× faster**

### Timing Breakdown
| Operation | Time (1M docs) | Percentage |
|-----------|----------------|------------|
| File loading | ~2-5 sec | 10-20% |
| Pipeline | ~15-20 sec | 80-90% |
| **Total** | **~17-25 sec** | **100%** |

---

## Troubleshooting

### File Not Found
```
Error: Failed to read file 'my_docs.txt': No such file or directory (os error 2)
```

**Solution**: Check file path is correct, use absolute path if needed.

### Empty File
```
Error: File is empty
```

**Solution**: Ensure file contains at least one document.

### No Valid Documents
```
Error: No valid documents found in file
```

**Solution**: Check file format (plain text or JSONL), ensure UTF-8 encoding.

### Out of Memory
```
Error: Cannot allocate memory
```

**Solution**:
- Split file into smaller chunks
- Use machine with more RAM
- Limit to <10M documents per run

### Malformed JSONL
```
├─ Skipped 5 malformed lines
```

**Info**: Non-JSON lines are skipped automatically, check file format if many skipped.

---

## Best Practices

### File Preparation
1. **UTF-8 encoding**: Ensure files are UTF-8
2. **One doc per line**: Plain text or JSONL
3. **No nested structures**: Keep JSONL simple
4. **Remove empty lines**: Speeds up loading

### Threshold Selection
- **0.80-0.85**: Balanced (recommended)
- **0.70-0.80**: Aggressive deduplication
- **0.85-0.95**: Conservative deduplication
- **0.95-1.00**: Very conservative (near-exact only)

### Performance Tips
1. **Use SSD**: File loading is I/O bound
2. **More RAM**: Helps with large files (>1M docs)
3. **Fewer cores**: Single-threaded, doesn't benefit from many cores
4. **Larger files**: Better throughput (amortizes startup cost)

---

## Limitations

### Current Limitations
- **Memory**: Entire file loaded into memory (no streaming)
- **Single-threaded**: File loading is sequential
- **No cluster output**: Clusters not saved to file yet
- **Simple JSONL**: Nested JSON not supported

### Future Enhancements
See `CUSTOM_DATA_INTEGRATION.md` for planned features.

---

## FAQ

**Q: Can I run on multiple files?**
A: Not yet. Run once per file, or concatenate files first.

**Q: Does it modify my original file?**
A: No, read-only operation.

**Q: Can I get the list of duplicate documents?**
A: Not yet, only cluster count. Future enhancement.

**Q: What encoding is supported?**
A: UTF-8 only.

**Q: How much RAM do I need?**
A: ~10× file size. 1GB file → 10GB RAM recommended.

**Q: Is there a file size limit?**
A: Practical limit: ~10M documents (depends on available RAM).

**Q: Can I use it in production?**
A: Yes, with META_CAPSULE protection and evaluation license.

---

## Contact

- **Sales**: sales@kindly.ai (production license)
- **Support**: support@kindly.ai (technical issues)
- **Documentation**: See `DEMO_README.md` for standard demo

---

**Framework**: I20 Integration Framework
**Version**: 1.0
**Status**: Production Ready
