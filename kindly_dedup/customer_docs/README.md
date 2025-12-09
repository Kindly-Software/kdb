# Kindly Dedup - High-Performance Document Deduplication

Kindly Dedup is a cutting-edge document deduplication solution designed for large-scale content processing. Built from the ground up for performance and accuracy, it helps organizations clean their datasets by identifying and removing duplicate content efficiently.

## What is Kindly Dedup?

Kindly Dedup uses advanced fingerprinting algorithms to identify similar and duplicate documents in large datasets. Whether you're processing millions of web pages, managing content repositories, or preparing training data for machine learning models, Kindly Dedup provides the speed and accuracy you need.

## Key Features

- **Exceptional Performance**: Process up to 60,000 documents per second on a single thread, with multi-threaded scaling for even higher throughput
- **High Accuracy**: Configurable similarity detection with precision/recall optimization
- **Scalable Architecture**: Handle datasets from thousands to billions of documents
- **Low Memory Footprint**: Process large datasets with minimal RAM requirements (93% memory reduction vs traditional approaches)
- **Flexible Input/Output**: Support for JSONL, CSV, and plain text formats
- **HTTP API**: RESTful API for easy integration with existing systems
- **Production-Ready**: Crash recovery, incremental updates, and enterprise reliability

## Performance Highlights

- **38× faster** than Python-based alternatives
- **60,000+ documents/second** on standard hardware
- **95% accuracy** in duplicate detection (F1 score)
- **3.5 GB memory** for 10 million documents (vs 40 GB for traditional approaches)
- **Sub-second crash recovery** with automatic state restoration

## Use Cases

### LLM Training Data Preparation
Remove duplicate content from large web crawls and text corpora to improve model training quality and reduce computational costs.

### Content Management
Identify and consolidate duplicate articles, documents, or web pages in content repositories and digital asset management systems.

### Data Lake Cleanup
Deduplicate large-scale data lakes to reduce storage costs and improve data quality for analytics and machine learning pipelines.

### Web Crawl Processing
Process billions of web pages efficiently, identifying near-duplicates and exact matches to create clean, unique datasets.

## System Requirements

- **Minimum**: 2 GB RAM, 2 CPU cores
- **Recommended**: 8+ GB RAM, 8+ CPU cores for optimal performance
- **Storage**: Varies by dataset size (approximately 5 KB per document for persistent storage)
- **Operating System**: Linux (Ubuntu 20.04+), macOS (10.15+), Windows (10+)

## Getting Started

See [QUICK_START.md](QUICK_START.md) for installation and basic usage instructions.

## Documentation

- [Quick Start Guide](QUICK_START.md) - Get up and running in 5 minutes
- [User Guide](USER_GUIDE.md) - Comprehensive CLI reference and usage examples
- [Deployment Guide](DEPLOYMENT.md) - Production deployment instructions
- [API Reference](API_REFERENCE.md) - HTTP API documentation
- [FAQ](FAQ.md) - Frequently asked questions
- [Troubleshooting](TROUBLESHOOTING.md) - Common issues and solutions
- [Support](SUPPORT.md) - Contact information and resources

## Performance Tiers

Kindly Dedup offers different performance profiles based on your dataset size and hardware:

- **Small Datasets** (< 100K documents): 60,000+ docs/sec, 100% accuracy, 2 GB RAM
- **Medium Datasets** (100K - 1M documents): 60,000+ docs/sec, 38× speedup vs alternatives, 4 GB RAM
- **Large Datasets** (1M - 10M documents): 373,000 docs/sec with multi-threading, 83-85% recall, 8 GB RAM
- **Very Large Datasets** (10M+ documents): Persistent mode with incremental updates, 16+ GB RAM

## License

Kindly Dedup is commercial software. See LICENSE file for terms and conditions.

Contact sales@kindly.ai for licensing information and enterprise support options.

## Next Steps

1. Read the [Quick Start Guide](QUICK_START.md) to install and run your first deduplication job
2. Explore the [User Guide](USER_GUIDE.md) for advanced features and configuration options
3. Review the [API Reference](API_REFERENCE.md) if you plan to integrate with existing systems
4. Check the [FAQ](FAQ.md) for common questions and best practices

For technical support, see [SUPPORT.md](SUPPORT.md).
