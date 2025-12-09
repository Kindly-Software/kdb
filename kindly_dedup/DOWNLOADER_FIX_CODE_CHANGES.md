# HuggingFace Downloader: Code Fix Implementation

## Overview
This document provides exact code changes to fix the premature stop issue by adding timeout protection to task collection loops.

---

## Change 1: Add Import Statement

**File**: `src/bin/download_hf_corpus.rs`
**Line**: ~84 (with other use statements)

**Current** (lines 82-86):
```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
```

**Fixed**:
```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio::time::timeout;  // <-- ADD THIS LINE
```

---

## Change 2: Fix Batch Task Collection Loop (CRITICAL)

**File**: `src/bin/download_hf_corpus.rs`
**Lines**: 695-713

**Current**:
```rust
        if tasks.len() >= concurrency * 2 || idx == files.len() - 1 {
            // Collect results from completed tasks (docs already sent to channel)
            let mut batch_doc_count = 0;
            for task in tasks.drain(..) {
                match task.await {
                    Ok(Ok((_shard_idx, doc_count))) => {
                        if doc_count > 0 {
                            // Docs already sent to channel inside download_hf_file (TRUE streaming)
                            batch_doc_count += doc_count;
                            shards_downloaded += 1;
                        }
                    }
                    Ok(Err(e)) => {
                        eprintln!("Task error: {}", e);
                    }
                    Err(e) => {
                        eprintln!("Join error: {}", e);
                    }
                }
            }
            shard_indices.clear();
```

**Fixed**:
```rust
        if tasks.len() >= concurrency * 2 || idx == files.len() - 1 {
            // Save checkpoint before waiting (in case task collection hangs)
            if let Err(e) = save_checkpoint(idx, resumed_docs + batch_doc_count, output_path) {
                eprintln!("{}Warning: Failed to save checkpoint: {}{}", PURPLE, e, RESET);
            }

            // Collect results from completed tasks (docs already sent to channel)
            let mut batch_doc_count = 0;
            for task in tasks.drain(..) {
                // Add timeout wrapper: max 120 seconds per task
                match timeout(Duration::from_secs(120), task).await {
                    Ok(Ok(Ok((_shard_idx, doc_count)))) => {
                        if doc_count > 0 {
                            // Docs already sent to channel inside download_hf_file (TRUE streaming)
                            batch_doc_count += doc_count;
                            shards_downloaded += 1;
                        }
                    }
                    Ok(Ok(Err(e))) => {
                        eprintln!("{}TASK ERROR: {}{}", PURPLE, e, RESET);
                        let _ = std::io::stderr().flush();
                    }
                    Ok(Err(e)) => {
                        eprintln!("{}JOIN ERROR: {}{}", PURPLE, e, RESET);
                        let _ = std::io::stderr().flush();
                    }
                    Err(_) => {
                        eprintln!("{}TASK TIMEOUT: Hung task detected (taking >120s){}", PURPLE, RESET);
                        let _ = std::io::stderr().flush();
                    }
                }
            }
            shard_indices.clear();
```

**Why**:
- `timeout()` wraps the task.await() to catch hung tasks
- `Ok(Ok(...))` = task completed successfully (nested Result from timeout + JoinHandle)
- `Ok(Ok(Err(...)))` = task failed with error
- `Ok(Err(...))` = task panicked
- `Err(...)` = task timeout (took >120s)
- `stderr().flush()` ensures error message appears in logs before crash
- Checkpoint saved BEFORE waiting (preserves progress even if task hangs)

---

## Change 3: Remove Old Checkpoint Save (to avoid duplicate)

**File**: `src/bin/download_hf_corpus.rs`
**Lines**: 728-731 (DELETE THESE)

**Current**:
```rust
            // Update progress atomically
            progress.update((resumed_docs + batch_doc_count) as u64, limit as u64);

            // Save checkpoint every batch (every concurrency * 2 shards)
            if let Err(e) = save_checkpoint(idx, resumed_docs + batch_doc_count, output_path) {
                eprintln!("{}Warning: Failed to save checkpoint: {}{}", PURPLE, e, RESET);
            }
```

**Fixed** (keep only progress update):
```rust
            // Update progress atomically
            progress.update((resumed_docs + batch_doc_count) as u64, limit as u64);
            // Note: Checkpoint now saved before task.await() to preserve progress if task hangs
```

---

## Change 4: Fix Final Task Collection Loop

**File**: `src/bin/download_hf_corpus.rs`
**Lines**: 735-753

**Current**:
```rust
    // Await any remaining tasks (docs already sent to channel)
    let mut final_doc_count = 0;
    for task in tasks.drain(..) {
        match task.await {
            Ok(Ok((_shard_idx, doc_count))) => {
                if doc_count > 0 {
                    // Docs already sent to channel inside download_hf_file (TRUE streaming)
                    final_doc_count += doc_count;
                    shards_downloaded += 1;
                }
            }
            Ok(Err(e)) => {
                eprintln!("Task error: {}", e);
            }
            Err(e) => {
                eprintln!("Join error: {}", e);
            }
        }
    }
```

**Fixed**:
```rust
    // Await any remaining tasks (docs already sent to channel)
    let mut final_doc_count = 0;
    for task in tasks.drain(..) {
        // Add timeout wrapper: max 120 seconds per task
        match timeout(Duration::from_secs(120), task).await {
            Ok(Ok(Ok((_shard_idx, doc_count)))) => {
                if doc_count > 0 {
                    // Docs already sent to channel inside download_hf_file (TRUE streaming)
                    final_doc_count += doc_count;
                    shards_downloaded += 1;
                }
            }
            Ok(Ok(Err(e))) => {
                eprintln!("{}TASK ERROR: {}{}", PURPLE, e, RESET);
                let _ = std::io::stderr().flush();
            }
            Ok(Err(e)) => {
                eprintln!("{}JOIN ERROR: {}{}", PURPLE, e, RESET);
                let _ = std::io::stderr().flush();
            }
            Err(_) => {
                eprintln!("{}TASK TIMEOUT: Hung task detected (taking >120s){}", PURPLE, RESET);
                let _ = std::io::stderr().flush();
            }
        }
    }
```

---

## Applying the Fixes

### Option 1: Manual Edit
1. Open `src/bin/download_hf_corpus.rs`
2. Add import: `use tokio::time::timeout;` after line 85
3. Update lines 695-713 with Change 2
4. Update lines 728-731 with Change 3
5. Update lines 735-753 with Change 4
6. Test build: `cargo build --bin download_hf_corpus --features hf-datasets`

### Option 2: Search/Replace Script
```bash
cd /home/samuel/Primitives/kindly_dedup

# Add import
sed -i '/use tokio::task::JoinHandle;/a use tokio::time::timeout;' \
    src/bin/download_hf_corpus.rs

# Replace task.await with timeout wrapper in batch loop
# (More complex, recommend manual edit for safety)
```

---

## Validation

### Build Check
```bash
cargo build --bin download_hf_corpus --release --features hf-datasets
# Should compile without warnings
```

### Syntax Check
```bash
cargo check --bin download_hf_corpus --features hf-datasets
# Should report no errors
```

### Quick Test
```bash
# Test with small dataset to verify timeout doesn't trigger
cargo run --bin download_hf_corpus --release --features hf-datasets -- \
  --dataset allenai/c4 --subset en --limit 100000 \
  --output test_data/c4_test_100k.jsonl --concurrency 4
# Should complete without timeout messages
```

### Stress Test
```bash
# Test with larger dataset to verify robustness
timeout 600 cargo run --bin download_hf_corpus --release --features hf-datasets -- \
  --dataset allenai/c4 --subset en --limit 1000000 \
  --output test_data/c4_test_1m.jsonl --concurrency 4

# Monitor for:
# 1. "Download complete!" message
# 2. No timeout messages (unless expected)
# 3. Final statistics printed
# 4. Checkpoint saved and deleted
```

---

## Monitoring the Fix

### Expected Output (with fix)
```
HuggingFace Dataset Downloader
Dataset: allenai/c4
...
Discovered dataset shards...
Found 1000 shard files...

Starting multi-shard download...
T8 Network: 4 concurrent shard downloads

[Shard 1] Downloading...
[Shard 2] Downloading...
...
[Shard 25] ✓ 354266 documents
...
[Download timeout: Hung task detected] <- Only if task stalls
...

Download complete!
  Shards downloaded: 28/1000
  Documents sent to writer: 10,000,000/1,000,000,000
  Overall download throughput: 60000 docs/sec

Closing download channel and waiting for writer thread to complete...
Writer thread completed writing 10000000 new documents
```

### Detecting Improvement
Compare old vs new behavior:
- **Old**: Stops at random doc count, no error message
- **New**: Either completes successfully OR prints "TASK TIMEOUT" message and continues

---

## Risks and Mitigations

| Risk | Likelihood | Mitigation |
|------|------------|-----------|
| 120s timeout too short for very slow network | Low | Increase to 180s if needed, monitor actual task times |
| Timeout mask real bugs in download_hf_file() | Low | Task errors are still reported, timeout is last resort |
| Multiple timeout messages spam logs | Low | Batch multiple errors into one message if needed |
| Checkpoint before timeout causes duplication | Medium | Accept small duplication risk, resume-from-checkpoint handles it |

---

## Testing the Fix

### Test 1: Verify No Regression
```bash
# Run existing test suite
cargo test --bin download_hf_corpus --features hf-datasets
```

### Test 2: Inject Artificial Slowness
Modify `download_hf_file()` to add:
```rust
// Add after line 350 (before request):
if file_path.contains("00070") {  // Only shard 70
    tokio::time::sleep(Duration::from_secs(130)).await;  // Exceeds timeout
}
```

**Expected result**: "TASK TIMEOUT" message, program continues

### Test 3: Network Interruption Test
```bash
# Simulate bad network while running
cargo run --bin download_hf_corpus --release -- ... &
sleep 5
sudo iptables -A OUTPUT -d 185.199.111.133 -j DROP  # Block HF CDN
sleep 10
sudo iptables -D OUTPUT -d 185.199.111.133 -j DROP  # Restore

# Should see timeout message, then recovery
```

---

## Summary

**Lines Changed**:
- 1 import added (line ~85)
- 2 nested loops modified (lines 697-713, 737-753)
- 1 checkpoint deletion (lines 728-731)
- **Total**: ~40 lines of code changes

**Complexity**: LOW
**Risk**: VERY LOW (timeout is standard Rust pattern)
**Benefit**: CRITICAL (prevents silent failures on 1B doc downloads)
**Time to fix**: ~5 minutes manual edit + testing
