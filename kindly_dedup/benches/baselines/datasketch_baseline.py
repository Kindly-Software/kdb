#!/usr/bin/env python3
"""
Python datasketch baseline for B32 fair comparison

B32 Compliance:
- Industry-standard datasketch library
- Same dataset as Rust benchmarks
- Same parameters (128 permutations, 0.85 threshold)
- NO optimization tricks (fair baseline)

Usage:
    python3 datasketch_baseline.py <corpus_path> [num_perm] [threshold]

Output:
    JSON with throughput, latency, total_time, duplicates_found
"""

import sys
import json
import time
from datasketch import MinHash, MinHashLSH


def load_corpus(corpus_path):
    """Load JSONL corpus file (same format as kindly_dedup)"""
    documents = []
    with open(corpus_path, 'r', encoding='utf-8') as f:
        for line in f:
            if line.strip():
                doc = json.loads(line)
                documents.append((doc['id'], doc['text']))
    return documents


def run_dedup(corpus_path, num_perm=128, threshold=0.85):
    """
    Run datasketch deduplication benchmark

    Args:
        corpus_path: Path to JSONL corpus
        num_perm: Number of MinHash permutations
        threshold: Jaccard similarity threshold

    Returns:
        dict: Benchmark results (JSON)
    """
    # Load corpus
    start_load = time.time()
    documents = load_corpus(corpus_path)
    load_time = time.time() - start_load

    num_docs = len(documents)
    print(f"Loaded {num_docs} documents in {load_time:.2f}s", file=sys.stderr)

    # Initialize LSH
    lsh = MinHashLSH(threshold=threshold, num_perm=num_perm)
    signatures = {}

    # Start timing
    start = time.time()

    # Process documents
    for doc_id, text in documents:
        # Create MinHash signature
        m = MinHash(num_perm=num_perm)

        # Tokenize (same as kindly_dedup: whitespace split)
        tokens = text.split()

        # Update MinHash
        for token in tokens:
            m.update(token.encode('utf-8'))

        # Insert into LSH
        lsh.insert(doc_id, m)
        signatures[doc_id] = m

    # Query phase - find duplicates
    duplicates = []
    for doc_id, sig in signatures.items():
        candidates = lsh.query(sig)
        if len(candidates) > 1:
            # Found duplicate cluster
            duplicates.append((doc_id, list(candidates)))

    elapsed = time.time() - start

    # Calculate metrics
    throughput = num_docs / elapsed if elapsed > 0 else 0
    latency_us = (elapsed / num_docs) * 1e6 if num_docs > 0 else 0

    # Return JSON results
    results = {
        'throughput_docs_per_sec': throughput,
        'latency_per_doc_us': latency_us,
        'total_time_sec': elapsed,
        'duplicates_found': len(duplicates),
        'num_documents': num_docs,
        'num_perm': num_perm,
        'threshold': threshold,
    }

    return results


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 datasketch_baseline.py <corpus_path> [num_perm] [threshold]",
              file=sys.stderr)
        sys.exit(1)

    corpus_path = sys.argv[1]
    num_perm = int(sys.argv[2]) if len(sys.argv) > 2 else 128
    threshold = float(sys.argv[3]) if len(sys.argv) > 3 else 0.85

    try:
        results = run_dedup(corpus_path, num_perm, threshold)
        print(json.dumps(results, indent=2))
    except Exception as e:
        error_result = {
            'throughput_docs_per_sec': 0.0,
            'latency_per_doc_us': 0.0,
            'total_time_sec': 0.0,
            'duplicates_found': 0,
            'error': str(e),
        }
        print(json.dumps(error_result, indent=2))
        sys.exit(1)


if __name__ == '__main__':
    main()
