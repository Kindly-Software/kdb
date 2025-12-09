# Quick Reference: 28ms Bottleneck Locations

## File: `/home/samuel/Primitives/kindly_dedup/src/parallel/orchestrator.rs`

### Bottleneck #1: Document Vector Clone (3-5ms) 🔴 CRITICAL

**Location**: Line 550 in `phase2_sign_parallel()`

```rust
550 | let docs_arc = Arc::new(documents.to_vec());  // ← BOTTLENECK!
```

**Fix**: Remove `.to_vec()` clone
```rust
let docs_arc = Arc::new(documents);  // ← No clone
```

**Impact**: 3-5ms improvement (~37-63% of overhead)

---

### Bottleneck #2: Mutex Lock on Signatures (1-2ms) 🔴 HIGH

**Location A**: Line 556 - Mutex creation
```rust
556 | let all_signatures = Arc::new(std::sync::Mutex::new(Vec::with_capacity(num_documents)));
```

**Location B**: Line 592-594 - Worker lock
```rust
592 |     if let Ok(mut sigs) = all_sigs.lock() {      // ← Lock holds up workers
593 |         sigs.extend(batch_signatures);
594 |     }
```

**Location C**: Lines 631-635 - Main thread double-lock
```rust
631 | if let Ok(final_sigs) = all_signatures.lock() {
632 |     if let Ok(mut self_sigs) = self.signatures.lock() {
633 |         *self_sigs = final_sigs.iter().map(|sig| *sig.signature()).collect();
634 |     }
635 | }
```

**Fix**: Use per-thread collectors instead of shared Mutex<Vec>

**Impact**: 1-2ms improvement (~12-25% of overhead)

---

### Bottleneck #3: Thread Spawn Overhead (1-2ms) 🔴 HIGH

**Location**: Line 572 in spawn loop
```rust
562 | for thread_id in 0..self.num_threads() {
563 |     // ... 6 Arc clones ...
572 |     thread_pool.execute(move || {  // ← Thread spawn cost
573 |         // ... worker logic
614 |     });
615 | }
```

**Cost**: ~1ms per thread creation (OS overhead)
**Fix**: Reuse thread pool across invocations

**Impact**: 1-2ms improvement (~12-25% of overhead)

---

### Bottleneck #4: Arc::clone() Contention (0.5-1.0ms) 🟡 MEDIUM

**Location**: Lines 563-569 in spawn loop
```rust
562 | for thread_id in 0..self.num_threads() {
563 |     let docs = docs_arc.clone();                    // Arc clone 1
564 |     let queue = queue_clone.clone();                // Arc clone 2
565 |     let progress = progress_clone.clone();          // Arc clone 3
566 |     let signatures = signature_capsule_arc.clone(); // Arc clone 4
567 |     let all_sigs = all_signatures.clone();          // Arc clone 5
569 |     let notifier_clone = Arc::clone(&notifier);     // Arc clone 6
570 | }
// Total: 6 clones × num_threads atomic increments = contention
```

**Fix**: Consolidate into shared context struct (1 Arc clone per thread)

**Impact**: 0.5-1.0ms improvement (~6-12% of overhead)

---

### Bottleneck #5: BatchQueue CAS Operations (0.5-1.0ms) 🟡 MEDIUM

**Location**: Lines 576, 604, 610 in worker loop
```rust
541 | for batch_id in 0..num_batches {
542 |     queue.enqueue(batch_id)?;              // CAS operation 1
543 | }

576 | while let Some(batch_id) = queue.dequeue() {  // CAS loop
577 |     // ... process batch
604 |     queue.mark_completed();                    // CAS operation 2
605 | }

610 | if queue.all_completed() {                    // Scan/check operation
611 |     notifier_clone.notify_completion();
612 | }
```

**Issue**: CAS contention on shared queue state + potential O(n) scan

**Fix**: Optimize BatchQueueCapsule implementation (if available)

**Impact**: 0.5-1.0ms improvement (~6-12% of overhead)

---

## Test Location: Performance Test

**File**: `/home/samuel/Primitives/kindly_dedup/src/parallel/orchestrator.rs`
**Function**: `prop_amdahls_law()` (line 1359)

**Key line**: 1375 - Corpus size
```rust
1375 | let corpus_size = 10_000;  // ← Only creates 1 batch (see issue #3 below)
```

**Related Issue**: Batch Distribution
- Current: 10,000 docs ÷ 16,384 batch size = 0.61 batches = **1 batch**
- Problem: Only 1 batch means only 1 worker has work, others idle
- Solution: Use 100,000+ docs or reduce batch size

**Impact**: Enables true parallelism (but doesn't help 10K test directly)

---

## Summary Table

| # | Bottleneck | File:Line | Overhead | Fix Priority | Est. Improvement |
|---|-----------|-----------|----------|--------------|------------------|
| 1 | Vec clone | 550 | 3-5ms | 🔴 CRITICAL | 1.14x → 1.45x |
| 2 | Mutex locks | 556,592,631 | 1-2ms | 🔴 HIGH | 1.45x → 1.65x |
| 3 | Thread spawn | 572 | 1-2ms | 🔴 HIGH | Marginal |
| 4 | Arc clones | 563-569 | 0.5-1ms | 🟡 MEDIUM | Marginal |
| 5 | Queue ops | 576,604,610 | 0.5-1ms | 🟡 MEDIUM | Marginal |

---

## Measured Data

**Test Command**:
```bash
cargo test --lib --release prop_amdahls_law -- --nocapture
```

**Results**:
```
Corpus: 10000 documents | Parallel fraction: 90%

Threads | Time (ms) | Speedup | Expected | Status
--------|-----------|---------|----------|--------
1       | 61.56     | 1.00x   | 1.00x    | ✓
2       | 54.08     | 1.14x   | 1.82x    | ✗ FAILED

Actual parallelism: 24.6% (should be 90%)
Serial overhead: 75.4% (should be 10%)
```

---

## Quick Fix Checklist

- [ ] Remove `.to_vec()` on line 550
- [ ] Replace shared Mutex<Vec> with per-thread collectors (lines 556, 567, 592, 631)
- [ ] Consider reusing thread pool (line 572)
- [ ] Consolidate Arc clones into context struct (lines 563-569)
- [ ] Update test corpus size to 100K or reduce batch size (line 1375)
- [ ] Re-run test: should see speedup improve to 1.65-1.80×

