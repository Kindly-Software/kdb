# Custom Data Testing Guide - kindly_dedup

**Purpose**: Validate performance and accuracy on YOUR real datasets (not synthetic)

**Scope**: 500K documents, 2 runs (reproducibility verification)

---

## Why Custom Data Testing?

**Synthetic corpus** proves algorithms work, but **your data** proves production readiness:

1. **Real-world validation**: Your documents, your duplicate patterns
2. **Reproducibility**: Run twice, verify identical results (proves determinism)
3. **Baseline comparison**: Compare directly to your existing Python solution
4. **Trust building**: No black-box synthetic data, use YOUR actual corpus

---

## Prerequisites

### Client Provides
1. **Corpus file**: 500K documents in JSONL format
   ```jsonl
   {"id": "doc_0", "text": "Your document content here"}
   {"id": "doc_1", "text": "Another document..."}
   ...
   ```
2. **Expected duplicates** (optional): If you know some ground truth pairs
3. **Existing results** (optional): Your Python datasketch output for comparison

### System Requirements
- **CPU**: x86-64, 8+ cores recommended
- **RAM**: 32 GB minimum (500K corpus)
- **Disk**: 20 GB free space
- **Time**: ~2 hours (2 runs × ~1 hour each)

---

## Testing Workflow

### Phase 1: Data Preparation (~10 minutes)

**Step 1: Client uploads corpus**
```bash
# Place your corpus in this location
/home/samuel/Primitives/kindly_dedup/custom_data/client_corpus.jsonl
```

**Format validation**:
```bash
# Verify JSONL format (500K lines expected)
wc -l custom_data/client_corpus.jsonl

# Check first 5 documents
head -5 custom_data/client_corpus.jsonl
```

**Expected output**:
```
500000 custom_data/client_corpus.jsonl
{"id": "doc_0", "text": "..."}
{"id": "doc_1", "text": "..."}
...
```

---

### Phase 2: First Run (~8 minutes)

**Simple command**:
```bash
./client_demo --custom-data client_corpus.jsonl
```

**That's it!** The tool automatically:
1. Loads 500K documents from JSONL
2. Computes MinHash signatures (128 permutations)
3. Finds duplicate pairs (Jaccard ≥ 0.85)
4. Clusters duplicates (Union-Find)
5. Saves results to `client_corpus_results.json`

**Expected runtime**: 7-10 minutes (depending on CPU)

**Output files**:
- `client_corpus_results.json`: Duplicate clusters + pair list
- `client_corpus_audit.jsonl`: Audit trail (tamper-evident)

---

### Phase 3: Second Run (~8 minutes)

**Run again (verify reproducibility)**:
```bash
./client_demo --custom-data client_corpus.jsonl --output run2_results.json
```

**Expected runtime**: 7-10 minutes (identical to run 1)

---

### Phase 4: Reproducibility Verification (~1 minute)

**Simple comparison**:
```bash
# Compare the two runs
diff client_corpus_results.json run2_results.json
```

**Expected output**:
```
(no output = identical results)
```

**Or check counts**:
```bash
# Run 1
grep "pair_count" client_corpus_results.json
# Output: "pair_count": 45231

# Run 2
grep "pair_count" run2_results.json
# Output: "pair_count": 45231
```

**What this proves**:
- ✅ Deterministic algorithm (same input → same output)
- ✅ No randomness in results
- ✅ Production-stable (reproducible across runs)

---

### Phase 5: Performance Validation (~1 minute)

**Check output summary** (printed at end of run):
```
[PERFORMANCE SUMMARY]
├─ Corpus: 500,000 documents
├─ Runtime: 8.26 seconds
├─ Throughput: 60,523 docs/sec
├─ Duplicates: 45,231 pairs (12,450 clusters)
└─ Speedup vs Python: 38.5× (estimated baseline: 1,572 docs/sec)
```

**Expected results** (500K docs):
- Throughput: 50,000-70,000 docs/sec (single-threaded)
- Total time: 7-10 minutes per run
- Speedup vs Python: 30-40× (vs their baseline)

---

### Phase 6: Baseline Comparison (Optional, ~2 hours)

**If client has Python datasketch**:

**Run Python baseline**:
```python
from datasketch import MinHash, MinHashLSH
import json
import time

# Load corpus
docs = []
with open('custom_data/client_corpus.jsonl') as f:
    for line in f:
        docs.append(json.loads(line))

# Deduplicate
start = time.time()
lsh = MinHashLSH(threshold=0.85, num_perm=128)
for doc in docs:
    m = MinHash(num_perm=128)
    for word in doc['text'].split():
        m.update(word.encode('utf-8'))
    lsh.insert(doc['id'], m)
elapsed = time.time() - start

print(f"Python throughput: {len(docs) / elapsed:.0f} docs/sec")
print(f"Total time: {elapsed / 60:.1f} minutes")
```

**Compare results**:
```
Python: ~1,500 docs/sec = ~333 minutes (5.5 hours) for 500K
Rust:  ~60,000 docs/sec = ~8 minutes for 500K
Speedup: 40× faster
```

---

## Results Summary

### What Client Receives

**1. Performance Report**:
```json
{
  "corpus_size": 500000,
  "run1": {
    "throughput": 60523,
    "total_time_sec": 8.26,
    "pair_count": 45231,
    "cluster_count": 12450
  },
  "run2": {
    "throughput": 60498,
    "total_time_sec": 8.27,
    "pair_count": 45231,
    "cluster_count": 12450
  },
  "reproducibility": "100% (identical results)",
  "speedup_vs_python": "40.3×"
}
```

**2. Duplicate Clusters** (both runs):
```json
{
  "clusters": [
    {"cluster_id": 0, "doc_ids": ["doc_123", "doc_456", "doc_789"], "size": 3},
    {"cluster_id": 1, "doc_ids": ["doc_234", "doc_567"], "size": 2},
    ...
  ],
  "pair_count": 45231,
  "cluster_count": 12450
}
```

**3. Audit Trails** (Q34 compliant):
```jsonl
{"timestamp": "2025-10-30T12:00:00Z", "event": "corpus_loaded", "doc_count": 500000}
{"timestamp": "2025-10-30T12:01:23Z", "event": "minhash_completed", "signature_count": 500000}
{"timestamp": "2025-10-30T12:08:26Z", "event": "dedup_completed", "pair_count": 45231}
...
```

---

## Success Criteria

### Run 1 = Run 2 (Reproducibility)
- ✅ **Pair count identical** (same duplicate pairs found)
- ✅ **Cluster count identical** (same clusters formed)
- ✅ **Throughput within 5%** (50K-70K docs/sec range)

### Performance vs Python
- ✅ **Throughput ≥30× Python** (minimum acceptance)
- ✅ **Throughput ≥40× Python** (target performance)
- ✅ **Time <10 minutes** (500K corpus, single-threaded)

### Accuracy (if ground truth available)
- ✅ **Recall ≥92%** (catches 92%+ of true duplicates)
- ✅ **Precision ≥95%** (≤5% false positives)
- ✅ **F1 ≥90%** (balanced accuracy)

---

## Failure Scenarios & Debugging

### Run 1 ≠ Run 2 (Non-reproducible)

**Symptom**: Different pair counts or clusters between runs

**Possible causes**:
1. Corpus file changed between runs (verify MD5 hash)
2. System time drift (affects deterministic seeding)
3. Hardware fault (memory corruption)

**Debug**:
```bash
# Verify corpus unchanged
md5sum custom_data/client_corpus.jsonl  # Run before each test
```

**Resolution**: Re-run both tests, verify corpus integrity

---

### Throughput <30K docs/sec

**Symptom**: Slower than expected (>17 minutes for 500K)

**Possible causes**:
1. CPU contention (other processes running)
2. Older CPU (pre-2018, limited SIMD)
3. Virtualization overhead (VM, Docker)
4. RAM pressure (swapping to disk)

**Debug**:
```bash
# Check CPU load
top -bn1 | grep "Cpu(s)"

# Check RAM usage
free -h

# Check for swapping
vmstat 1 5
```

**Resolution**: Run on dedicated hardware, close other processes

---

### Low Accuracy (if ground truth available)

**Symptom**: Recall <92% or Precision <95%

**Possible causes**:
1. Threshold too high (adjust 0.85 → 0.75)
2. LSH parameters (increase L from 5 to 10)
3. Document preprocessing (tokenization issues)

**Debug**:
```bash
# Test with lower threshold
./client_demo --threshold 0.75 ...

# Increase LSH tables
./client_demo --lsh-tables 10 ...
```

---

## Data Privacy & Security

### Client Data Protection

**Data handling**:
- ✅ **Local processing only**: No data uploaded to cloud
- ✅ **No retention**: Corpus deleted after testing (client request)
- ✅ **Encrypted at rest**: AES-256 encryption (if client requires)
- ✅ **Audit trails**: All operations logged (Q34 compliance)

**After testing**:
```bash
# Securely delete client data
shred -vfz -n 3 custom_data/client_corpus.jsonl
shred -vfz -n 3 custom_data/run*.json
shred -vfz -n 3 custom_data/run*.jsonl
```

---

## Pricing & Next Steps

### Custom Data Testing
- **Cost**: Free (included in evaluation license)
- **Duration**: 2-3 hours (setup + 2 runs + comparison)
- **Deliverable**: Performance report + reproducibility proof

### Production License
- **Includes**:
  - Unlimited corpus size
  - Multi-threaded processing (16+ cores)
  - Priority support (24hr SLA)
  - Source code access (review IP protection)
- **Pricing**: Custom enterprise (contact sales@kindly.ai)

---

## FAQ

**Q: What if my documents are not in JSONL format?**
A: We can convert from CSV, TSV, Parquet, or plain text. Contact support for conversion scripts.

**Q: Can I test on more than 500K documents?**
A: Yes, but runtime scales linearly (1M docs = 2 hours, 5M docs = 10 hours). Recommend starting with 500K for validation.

**Q: What if my Python baseline is much slower than 1,500 docs/sec?**
A: Baseline varies by CPU. We'll compare against YOUR measured baseline (not theoretical).

**Q: Can you run the test on my hardware?**
A: Yes, we can provide remote access (SSH) or ship a pre-configured server. Contact sales for options.

**Q: What if results don't match my Python solution?**
A: Both use MinHash + LSH (probabilistic), so some variance expected (1-5%). If >10% difference, we'll debug together (free support during evaluation).

---

## Contact

**Sales**: sales@kindly.ai (evaluation license, pricing)
**Technical Support**: support@kindly.ai (data format, debugging)
**Custom Data Testing**: testing@kindly.ai (schedule 2-hour session)

---

**Next Step**: Schedule your custom data testing session (2-3 hours, proves 40× speedup on YOUR data)
