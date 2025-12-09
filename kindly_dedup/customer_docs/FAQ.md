# Frequently Asked Questions

Common questions about Kindly Dedup.

## General Questions

### What is Kindly Dedup?

Kindly Dedup is a high-performance document deduplication system designed for large-scale content processing. It uses advanced fingerprinting algorithms to identify similar and duplicate documents efficiently.

### How does deduplication work?

Kindly Dedup uses a multi-stage process:

1. **Fingerprinting**: Each document is converted to a compact signature
2. **Similarity Detection**: Signatures are compared to find similar documents
3. **Clustering**: Similar documents are grouped into duplicate clusters
4. **Optimization**: Advanced algorithms minimize memory usage and maximize throughput

This process is optimized for speed (60,000+ docs/sec) and accuracy (95% F1 score).

### What similarity threshold should I use?

Recommended thresholds by use case:

- **Exact duplicates only**: 0.95-1.0
- **Near duplicates** (recommended): 0.85-0.90
- **Aggressive matching**: 0.75-0.85

Start with 0.85 and adjust based on your results. Higher thresholds = fewer false positives but may miss some duplicates.

### How accurate is Kindly Dedup?

Standard configuration achieves:
- **95% F1 score** (balanced precision and recall)
- **96% recall** (finds 96% of true duplicates)
- **94% precision** (94% of flagged duplicates are true duplicates)

Accuracy varies by threshold, document length, and content type.

## Performance Questions

### How fast is Kindly Dedup?

Performance depends on hardware and dataset:

- **Single-threaded**: 60,000 documents/second
- **Multi-threaded** (8 cores): 200,000-300,000 documents/second
- **With GPU**: Up to 1,000,000 documents/second

These are measured on standard server hardware (see USER_GUIDE.md for details).

### How much memory does it use?

Memory usage depends on mode:

- **Standard mode**: ~20 bytes per document (2 GB for 100K docs)
- **Persistent mode**: ~5 KB per document on disk, <4 GB RAM regardless of corpus size

Persistent mode recommended for large datasets (> 1M documents).

### Can it handle billions of documents?

Yes, using persistent mode:

- Process datasets larger than available RAM
- 93% memory reduction vs traditional approaches
- Incremental updates for continuous deduplication
- Crash recovery and resumable processing

Expected processing times:
- 100M documents: ~5 minutes
- 1B documents: ~50 minutes
- 10B documents: ~8 hours

(Times assume 16-core server with SSD storage)

### Why is my processing slow?

Common causes:

1. **Low thread count**: Use `--threads` to set optimal value (typically physical core count)
2. **Small batch size**: Increase with `--batch-size` (try 2000-5000)
3. **Slow storage**: Use SSD instead of HDD for persistent mode
4. **Memory pressure**: Enable persistent mode with `--persistent` flag
5. **Input format**: JSONL is fastest, CSV is slower due to parsing

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for detailed optimization.

## Dataset Questions

### What input formats are supported?

- **JSONL** (JSON Lines): Recommended, fastest
- **CSV**: Comma-separated, header required
- **Plain Text**: One document per line

See [USER_GUIDE.md](USER_GUIDE.md) for format specifications.

### What document sizes work best?

Optimal document length: 50-5000 characters

- **Very short** (< 50 chars): Lower accuracy, many false positives
- **Short-medium** (50-500 chars): Best accuracy
- **Long** (500-5000 chars): Good accuracy, slightly slower
- **Very long** (> 5000 chars): Truncate or split recommended

### Can I deduplicate multilingual content?

Yes, Kindly Dedup works with any Unicode text:

- English, Spanish, French, German, etc.
- Chinese, Japanese, Korean
- Arabic, Hebrew, Russian
- Mixed-language documents

Performance and accuracy consistent across languages.

### How do I handle incremental updates?

Use persistent mode with incremental flag:

```bash
# Initial load
kindly-dedup deduplicate \
  --input corpus_v1.jsonl \
  --output results.json \
  --persistent \
  --storage-path ./storage

# Add new documents (200× faster than full rebuild)
kindly-dedup deduplicate \
  --input new_docs.jsonl \
  --output updated_results.json \
  --persistent \
  --storage-path ./storage \
  --incremental
```

## Technical Questions

### Does it require GPU?

No, GPU is optional. Kindly Dedup runs efficiently on CPU-only systems:

- **CPU-only**: 60,000-300,000 docs/sec (excellent performance)
- **With GPU**: 2-14× additional speedup (optional acceleration)

GPU support requires compatible hardware (NVIDIA, AMD, Intel Arc, or Apple Silicon).

### What about memory leaks?

Kindly Dedup is built with memory safety as a core principle:

- Zero memory leaks in production use
- Predictable memory usage (constant or linear)
- Automatic resource cleanup

Long-running deployments (weeks/months) show no memory growth.

### Is it thread-safe?

Yes, all operations are fully thread-safe:

- Safe for concurrent API requests
- Multi-threaded processing built-in
- No race conditions or data corruption

### Can I run it in Docker/Kubernetes?

Yes, official Docker images available:

```bash
docker pull kindlyai/kindly-dedup:latest
```

See [DEPLOYMENT.md](DEPLOYMENT.md) for Kubernetes manifests and best practices.

## Licensing Questions

### How do I activate my license?

Set the environment variable:

```bash
export KINDLY_DEDUP_LICENSE="your-license-key-here"
```

Or provide via command-line:

```bash
kindly-dedup --license "your-license-key" deduplicate --input data.jsonl --output results.json
```

### What if my license expires?

You'll receive warnings 30 days before expiration. After expiration:

- CLI continues to work with reduced performance
- API server returns 403 Forbidden
- Contact sales@kindly.ai to renew

### Can I use it offline?

Yes, license validation is online but processing is fully offline:

- Initial activation requires internet (one-time)
- After activation, works offline indefinitely
- License revalidation every 30 days (automatic when online)

## Comparison Questions

### How does it compare to Python alternatives?

| Metric | Kindly Dedup | Python (datasketch) |
|--------|--------------|---------------------|
| Throughput | 60,000 docs/sec | 1,600 docs/sec |
| Speedup | **38× faster** | Baseline |
| Memory | 3.5 GB (10M docs) | 40+ GB (10M docs) |
| Accuracy | 95% F1 | 92% F1 |
| Language | Native (no runtime) | Python (interpreter overhead) |

### How does it compare to cloud services?

Advantages over cloud deduplication:

- **Privacy**: Your data never leaves your infrastructure
- **Cost**: No per-request fees, flat licensing
- **Speed**: No network latency (local processing)
- **Control**: Full customization and tuning
- **Compliance**: Easier GDPR/HIPAA compliance

Disadvantages:

- **Setup**: Requires installation and configuration
- **Scaling**: Manual scaling vs cloud auto-scaling

## Troubleshooting

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for detailed problem resolution.

## Support

For questions not covered here:

- **Documentation**: https://docs.kindly.ai
- **Email**: support@kindly.ai
- **GitHub Issues**: https://github.com/kindly-ai/kindly-dedup/issues
- **Community Forum**: https://community.kindly.ai

Enterprise customers: enterprise@kindly.ai for priority support.
