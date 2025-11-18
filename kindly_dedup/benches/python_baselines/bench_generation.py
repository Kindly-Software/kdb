#!/usr/bin/env python3
"""
Python Corpus Generation Baseline (B32 Fair Comparison)

**Purpose**: Measure FAIR Python baseline for corpus generation to compare against Rust

**B32 Compliance**:
- Fair baseline: Uses multiprocessing (NOT single-threaded strawman)
- Same hardware: Runs on same machine as Rust benchmarks
- Same workload: Identical corpus size and document structure
- Honest reporting: Reports both single-threaded and multi-threaded throughput

**Expected Results**:
- Single-threaded: ~100K docs/sec (Python string formatting)
- Multi-threaded (16 workers): ~800K docs/sec (8× speedup, Python GIL limited)
- Rust baseline: ~2M+ docs/sec (20× faster, no GIL)

Usage:
    python3 bench_generation.py <corpus_size> [num_workers]

Output:
    JSON with throughput, latency, and timing results
"""

import sys
import json
import time
import multiprocessing
from typing import List, Tuple

# Templates (same as Rust implementation)
TEMPLATES = [
    "Machine learning algorithms process data through neural networks with backpropagation",
    "Natural language processing enables computers to understand human communication patterns",
    "Deep learning architectures include convolutional and recurrent neural networks",
    "Transfer learning allows models to leverage pre-trained knowledge for new tasks",
    "Attention mechanisms improve sequence-to-sequence model performance significantly",
    "Transformer architecture revolutionized natural language understanding and generation",
    "Reinforcement learning trains agents through reward signals and exploration",
    "Computer vision systems analyze images using convolutional neural networks",
    "Generative adversarial networks create realistic synthetic data through competition",
    "Self-supervised learning reduces the need for manually labeled training data",
]


def generate_document(i: int) -> Tuple[int, str]:
    """Generate single document (for parallel processing)"""
    template = TEMPLATES[i % len(TEMPLATES)]
    text = f"{template} document {i} with unique identifier {i * 17} and timestamp {i * 23}"
    return (i, text)


def generate_corpus_single(num_docs: int) -> List[Tuple[int, str]]:
    """
    Generate corpus SINGLE-THREADED (baseline)

    B32 Note: This is the UNFAIR strawman baseline, but included for comparison
    """
    corpus = []
    for i in range(num_docs):
        doc = generate_document(i)
        corpus.append(doc)
    return corpus


def generate_corpus_multi(num_docs: int, num_workers: int = None) -> List[Tuple[int, str]]:
    """
    Generate corpus MULTI-THREADED (FAIR baseline)

    B32 Note: This is the FAIR optimized Python baseline for comparison

    Args:
        num_docs: Number of documents to generate
        num_workers: Number of worker processes (defaults to CPU count)

    Returns:
        List of (doc_id, text) tuples
    """
    if num_workers is None:
        num_workers = multiprocessing.cpu_count()

    with multiprocessing.Pool(processes=num_workers) as pool:
        corpus = pool.map(generate_document, range(num_docs))

    return corpus


def benchmark_generation(corpus_size: int, num_workers: int = None) -> dict:
    """
    Benchmark corpus generation with statistical rigor

    Args:
        corpus_size: Number of documents to generate
        num_workers: Number of worker processes (None = auto-detect)

    Returns:
        dict: Benchmark results (JSON)
    """
    if num_workers is None:
        num_workers = multiprocessing.cpu_count()

    results = {
        'corpus_size': corpus_size,
        'num_workers': num_workers,
    }

    # Benchmark 1: Single-threaded (UNFAIR but informative)
    print(f"Running single-threaded baseline ({corpus_size:,} docs)...", file=sys.stderr)
    start = time.time()
    corpus_single = generate_corpus_single(corpus_size)
    elapsed_single = time.time() - start

    throughput_single = corpus_size / elapsed_single if elapsed_single > 0 else 0
    latency_single_us = (elapsed_single / corpus_size) * 1e6 if corpus_size > 0 else 0

    results['single_threaded'] = {
        'throughput_docs_per_sec': throughput_single,
        'latency_per_doc_us': latency_single_us,
        'total_time_sec': elapsed_single,
    }

    print(f"  Single-threaded: {throughput_single:,.0f} docs/sec", file=sys.stderr)

    # Benchmark 2: Multi-threaded (FAIR baseline)
    print(f"Running multi-threaded baseline ({num_workers} workers)...", file=sys.stderr)

    # Warmup (1 iteration)
    _ = generate_corpus_multi(min(corpus_size // 10, 1000), num_workers)

    # Actual measurement (3 runs for statistical validity)
    times = []
    for run in range(3):
        start = time.time()
        corpus_multi = generate_corpus_multi(corpus_size, num_workers)
        elapsed = time.time() - start
        times.append(elapsed)
        print(f"  Run {run + 1}: {elapsed:.3f}s", file=sys.stderr)

    # Calculate statistics
    elapsed_multi = sum(times) / len(times)  # Mean
    throughput_multi = corpus_size / elapsed_multi if elapsed_multi > 0 else 0
    latency_multi_us = (elapsed_multi / corpus_size) * 1e6 if corpus_size > 0 else 0

    # Calculate speedup vs single-threaded
    speedup = throughput_multi / throughput_single if throughput_single > 0 else 0

    results['multi_threaded'] = {
        'throughput_docs_per_sec': throughput_multi,
        'latency_per_doc_us': latency_multi_us,
        'total_time_sec': elapsed_multi,
        'speedup_vs_single': speedup,
        'num_runs': 3,
        'times_sec': times,
    }

    print(f"  Multi-threaded: {throughput_multi:,.0f} docs/sec ({speedup:.2f}× speedup)", file=sys.stderr)

    # B32 Reality Check
    print("\n=== B32 Reality Check ===", file=sys.stderr)
    print(f"Expected: 100K-800K docs/sec (single-multi)", file=sys.stderr)
    print(f"Measured: {throughput_single:,.0f} - {throughput_multi:,.0f} docs/sec", file=sys.stderr)

    if speedup > 2 * num_workers:
        print(f"WARNING: Speedup {speedup:.2f}× exceeds realistic {num_workers}× (suspicious)", file=sys.stderr)
    elif speedup < num_workers * 0.3:
        print(f"WARNING: Speedup {speedup:.2f}× below expected {num_workers * 0.5}× (contention?)", file=sys.stderr)
    else:
        print(f"✓ Speedup {speedup:.2f}× is reasonable for {num_workers} workers", file=sys.stderr)

    return results


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 bench_generation.py <corpus_size> [num_workers]", file=sys.stderr)
        print("", file=sys.stderr)
        print("Examples:", file=sys.stderr)
        print("  python3 bench_generation.py 10000", file=sys.stderr)
        print("  python3 bench_generation.py 100000 16", file=sys.stderr)
        sys.exit(1)

    corpus_size = int(sys.argv[1])
    num_workers = int(sys.argv[2]) if len(sys.argv) > 2 else None

    try:
        results = benchmark_generation(corpus_size, num_workers)
        print(json.dumps(results, indent=2))
    except Exception as e:
        error_result = {
            'corpus_size': corpus_size,
            'error': str(e),
        }
        print(json.dumps(error_result, indent=2))
        sys.exit(1)


if __name__ == '__main__':
    main()
