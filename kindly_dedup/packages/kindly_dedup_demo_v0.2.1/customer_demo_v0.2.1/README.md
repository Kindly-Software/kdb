# kindly_dedup Demo - Fast LLM Dataset Deduplication

Thank you for evaluating kindly_dedup! 🎉

## What is this?

kindly_dedup is a high-performance tool for removing duplicate documents from LLM training datasets. It's typically **30-40× faster** than Python alternatives while maintaining near-perfect accuracy.

## Quick Start

```bash
# Make the demo executable
chmod +x client_demo

# Run the demo (takes about 45 minutes)
./client_demo
```

The demo will:
1. **Validate accuracy** on 100K documents (~17 min)
2. **Demonstrate speed** on 1M documents (~17 sec)
3. **Show scale** on 10M documents (~3 min)

## Test Your Own Data

Want to validate on your real dataset?

```bash
./client_demo --custom-data your_corpus.jsonl
```

**Supported formats**: JSONL, JSON, plain text
**Optimal size**: 500K documents (~3-10 minutes)
**Demo limit**: 5 million documents maximum

## System Requirements

**Minimum**:
- 16 GB RAM
- 4+ CPU cores
- 10 GB disk space
- Linux, macOS, or Windows

**Recommended** (for full demo):
- 64 GB RAM
- 8+ CPU cores
- 50 GB disk space
- Linux (fastest performance)

## What You'll See

**Accuracy**: Near-perfect results (95-100% precision, 95-98% recall)
**Speed**: Up to 60K documents per second (single-threaded)
**Scale**: Multi-threaded processing handles 300-500K docs/sec

## Need Help?

- **Questions?** sales@kindly.software
- **Technical support?** support@kindly.software
- **More details?** See SALES_SHEET.md

## Evaluation License

This demo is provided under an evaluation license:
- **Duration**: 30 days
- **Limitations**: 5 million documents maximum
- **Purpose**: Production performance validation

For production use beyond the demo limit, contact sales@kindly.software

---

**Dedup from Kindly 💜**

We're here to help you build better AI models.
