#!/usr/bin/env python3
"""
Python Deduplication Baseline (B32 Fair Comparison)

**Purpose**: Measure FAIR Python baseline for deduplication to compare against Rust

**B32 Compliance**:
- Fair baseline: Uses optimized datasketch library (NOT naive implementation)
- Same hardware: Runs on same machine as Rust benchmarks
- Same algorithm: MinHash (128 perm) + LSH (0.85 threshold)
- Honest reporting: Reports throughput with statistical rigor (3+ runs)

**Expected Results**:
- Single-threaded datasketch: ~1,500-2,000 docs/sec (measured: 1,572 docs/sec)
- Rust v1.0: ~60,000 docs/sec (38× speedup, EXCEPTIONAL tier per B32 K27)

Usage:
    python3 bench_dedup.py <corpus_path> [num_perm] [threshold]

Output:
    JSON with throughput, latency, duplicates found, and timing results
"""

import sys
import json
import time
from datasketch import MinHash, MinHashLSH
from typing import List, Tuple


def load_corpus(corpus_path: str) -> List[Tuple[int, str]]:
    """
    Load JSONL corpus file (same format as kindly_dedup)

    Args:
        corpus_path: Path to JSONL file (one JSON object per line)

    Returns:
        List of (doc_id, text) tuples
    """
    documents = []
    with open(corpus_path, 'r', encoding='utf-8') as f:
        for line in f:
            if line.strip():
                doc = json.loads(line)
                documents.append((doc['id'], doc['text']))
    return documents


def deduplicate_datasketch(
    corpus: List[Tuple[int, str]],
    num_perm: int = 128,
    threshold: float = 0.85
) -> Tuple[int, float]:
    """
    Deduplicate corpus using datasketch (FAIR baseline)

    Args:
        corpus: List of (doc_id, text) tuples
        num_perm: Number of MinHash permutations (default 128)
        threshold: Jaccard similarity threshold (default 0.85)

    Returns:
        Tuple of (num_duplicates, elapsed_time_sec)
    """
    # Initialize LSH
    lsh = MinHashLSH(threshold=threshold, num_perm=num_perm)
    signatures = {}

    # Start timing
    start = time.time()

    # Process documents
    for doc_id, text in corpus:
        # Create MinHash signature
        m = MinHash(num_perm=num_perm)

        # Tokenize (same as kindly_dedup: whitespace split + lowercase)
        tokens = text.lower().split()

        # Update MinHash
        for token in tokens:
            m.update(token.encode('utf-8'))

        # Insert into LSH
        lsh.insert(doc_id, m)
        signatures[doc_id] = m

    # Query phase - find duplicates
    duplicates = []
    visited = set()

    for doc_id, sig in signatures.items():
        if doc_id in visited:
            continue

        candidates = lsh.query(sig)
        if len(candidates) > 1:
            # Found duplicate cluster
            cluster = list(candidates)
            duplicates.append(cluster)
            visited.update(cluster)

    elapsed = time.time() - start

    return len(duplicates), elapsed


def benchmark_deduplication(
    corpus_path: str,
    num_perm: int = 128,
    threshold: float = 0.85,
    num_runs: int = 3
) -> dict:
    """
    Benchmark deduplication with statistical rigor

    Args:
        corpus_path: Path to JSONL corpus
        num_perm: Number of MinHash permutations
        threshold: Jaccard similarity threshold
        num_runs: Number of runs for statistical validity

    Returns:
        dict: Benchmark results (JSON)
    """
    # Load corpus
    print(f"Loading corpus from {corpus_path}...", file=sys.stderr)
    start_load = time.time()
    corpus = load_corpus(corpus_path)
    load_time = time.time() - start_load
    num_docs = len(corpus)

    print(f"Loaded {num_docs:,} documents in {load_time:.2f}s", file=sys.stderr)

    # Warmup (1 run)
    print("Warming up...", file=sys.stderr)
    _, _ = deduplicate_datasketch(corpus[:min(num_docs // 10, 1000)], num_perm, threshold)

    # Actual measurement (3+ runs for statistical validity)
    print(f"Running {num_runs} benchmark runs...", file=sys.stderr)
    times = []
    duplicate_counts = []

    for run in range(num_runs):
        num_duplicates, elapsed = deduplicate_datasketch(corpus, num_perm, threshold)
        times.append(elapsed)
        duplicate_counts.append(num_duplicates)
        throughput = num_docs / elapsed if elapsed > 0 else 0
        print(f"  Run {run + 1}: {elapsed:.3f}s ({throughput:,.0f} docs/sec, {num_duplicates} duplicate clusters)", file=sys.stderr)

    # Calculate statistics
    elapsed_mean = sum(times) / len(times)
    elapsed_min = min(times)
    elapsed_max = max(times)

    throughput_mean = num_docs / elapsed_mean if elapsed_mean > 0 else 0
    latency_mean_us = (elapsed_mean / num_docs) * 1e6 if num_docs > 0 else 0

    # B32 Reality Check
    print("\n=== B32 Reality Check ===", file=sys.stderr)
    print(f"Expected Python datasketch: 1,500-2,000 docs/sec", file=sys.stderr)
    print(f"Measured: {throughput_mean:,.0f} docs/sec", file=sys.stderr)

    if throughput_mean > 10_000:
        print(f"WARNING: Throughput {throughput_mean:,.0f} exceeds realistic Python baseline (suspicious)", file=sys.stderr)
    elif throughput_mean < 500:
        print(f"WARNING: Throughput {throughput_mean:,.0f} below expected baseline (hardware issue?)", file=sys.stderr)
    else:
        print(f"✓ Throughput is reasonable for Python datasketch baseline", file=sys.stderr)

    # Prepare results
    results = {
        'corpus_size': num_docs,
        'num_perm': num_perm,
        'threshold': threshold,
        'num_runs': num_runs,
        'throughput_docs_per_sec': throughput_mean,
        'latency_per_doc_us': latency_mean_us,
        'total_time_mean_sec': elapsed_mean,
        'total_time_min_sec': elapsed_min,
        'total_time_max_sec': elapsed_max,
        'times_sec': times,
        'duplicates_found': duplicate_counts[0],  # Assume consistent across runs
        'load_time_sec': load_time,
    }

    return results


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 bench_dedup.py <corpus_path> [num_perm] [threshold] [num_runs]", file=sys.stderr)
        print("", file=sys.stderr)
        print("Examples:", file=sys.stderr)
        print("  python3 bench_dedup.py test_data/synthetic_1k.json", file=sys.stderr)
        print("  python3 bench_dedup.py test_data/synthetic_10k.json 128 0.85", file=sys.stderr)
        print("  python3 bench_dedup.py corpus.json 128 0.85 5", file=sys.stderr)
        sys.exit(1)

    corpus_path = sys.argv[1]
    num_perm = int(sys.argv[2]) if len(sys.argv) > 2 else 128
    threshold = float(sys.argv[3]) if len(sys.argv) > 3 else 0.85
    num_runs = int(sys.argv[4]) if len(sys.argv) > 4 else 3

    try:
        results = benchmark_deduplication(corpus_path, num_perm, threshold, num_runs)
        print(json.dumps(results, indent=2))
    except Exception as e:
        error_result = {
            'corpus_path': corpus_path,
            'error': str(e),
        }
        print(json.dumps(error_result, indent=2))
        sys.exit(1)


if __name__ == '__main__':
    main()
