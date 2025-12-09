# TransactionLogCapsule Usage Guide

Quick-reference guide for using TransactionLogCapsule in kindly_dedup.

## Quick Start

### 1. Enable Feature in Cargo.toml
```toml
[dependencies]
kindly_dedup = { path = ".", features = ["batch-lsh"] }
```

### 2. Basic Usage
```rust
use kindly_dedup::lsh::{TransactionLogCapsule, LshEntry};

// Create log
let log = TransactionLogCapsule::new("dedup.txn.log")?;

// Append batch
let batch = vec![
    LshEntry::new(0, 0x1234567890abcdef, 1),
    LshEntry::new(1, 0xfedcba9876543210, 2),
];
let gen = log.append_batch(&batch)?;

// Replay
let batches = log.replay()?;
assert_eq!(batches[0][0].doc_id, 1);

// Clear after commit
log.truncate()?;
```

## Common Patterns

### Pattern 1: Single Batch Transaction
```rust
fn insert_batch(
    log: &TransactionLogCapsule,
    lsh_index: &mut LshIndex,
    batch: &[LshEntry],
) -> Result<()> {
    // Write to transaction log (crash-safe)
    let _gen = log.append_batch(batch)?;

    // Insert into index
    lsh_index.insert_batch(batch)?;

    // Commit (in production: database commit, etc.)
    Ok(())
}
```

### Pattern 2: Crash Recovery
```rust
fn recover_on_startup(
    log_path: &str,
    lsh_index: &mut LshIndex,
) -> Result<()> {
    let log = TransactionLogCapsule::new(log_path)?;

    // Replay all batches
    let batches = log.replay()?;
    for batch in batches {
        lsh_index.insert_batch(&batch)?;
    }

    // Clear log after recovery
    log.truncate()?;
    Ok(())
}
```

### Pattern 3: Batch Accumulation
```rust
fn batch_inserter(
    log: &TransactionLogCapsule,
    lsh_index: &mut LshIndex,
) -> Result<()> {
    let mut batch = Vec::new();
    const BATCH_SIZE: usize = 1000;

    for entry in entries_from_source()? {
        batch.push(entry);

        // Flush when batch reaches target size
        if batch.len() >= BATCH_SIZE {
            let _gen = log.append_batch(&batch)?;
            lsh_index.insert_batch(&batch)?;
            batch.clear();
        }
    }

    // Flush remaining
    if !batch.is_empty() {
        let _gen = log.append_batch(&batch)?;
        lsh_index.insert_batch(&batch)?;
    }

    Ok(())
}
```

### Pattern 4: Generation-Based Tracking
```rust
fn track_progress(log: &TransactionLogCapsule) -> u64 {
    // Get current generation (transaction ID)
    // Even = committed, Odd = in-flight
    let gen = log.get_generation();
    (gen / 2) as u64  // Number of committed transactions
}
```

### Pattern 5: Integrity Verification
```rust
fn verify_log_integrity(log_path: &str) -> Result<bool> {
    let log = TransactionLogCapsule::new(log_path)?;

    // Verify CRC32 checksums
    let is_valid = log.verify_checksum()?;

    if !is_valid {
        eprintln!("Log corruption detected!");
        // In production: alert monitoring system, trigger recovery
    }

    Ok(is_valid)
}
```

## Performance Tuning

### Batch Size Selection
```rust
// Small batches (100): Low latency, more frequent fsync
const BATCH_SIZE_LOW_LATENCY: usize = 100;

// Medium batches (1000): Balanced (default, recommended)
const BATCH_SIZE_BALANCED: usize = 1000;

// Large batches (5000): High throughput, higher per-batch latency
const BATCH_SIZE_HIGH_THROUGHPUT: usize = 5000;

// Select based on your needs:
let log = TransactionLogCapsule::new("dedup.log")?;
// Uses BATCH_SIZE_BALANCED internally
```

### Log Rotation
```rust
fn check_log_size(log: &TransactionLogCapsule) -> Result<()> {
    let bytes = log.get_bytes_written();

    // Default: 1 GB threshold
    const MAX_LOG_SIZE: u64 = 1_000_000_000;

    if bytes > MAX_LOG_SIZE {
        // In production: trigger rotation
        // log.rotate_log()?;  // Will be public in future versions
    }

    Ok(())
}
```

## Error Handling

### Common Errors
```rust
use std::io;

match log.append_batch(&batch) {
    Ok(gen) => println!("Batch {}: OK", gen),
    Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
        eprintln!("Permission denied: {}", e);
        // Handle permission issue
    }
    Err(e) if e.kind() == io::ErrorKind::NoSpace => {
        eprintln!("Disk full: {}", e);
        // Handle disk full
    }
    Err(e) => {
        eprintln!("I/O error: {}", e);
        // Handle other I/O errors
    }
}
```

## Testing Your Integration

### Unit Test Template
```rust
#[cfg(test)]
mod tests {
    use kindly_dedup::lsh::{TransactionLogCapsule, LshEntry};

    #[test]
    fn test_my_integration() {
        let log_path = "/tmp/test_transaction_log.log";
        let log = TransactionLogCapsule::new(log_path).unwrap();

        // Test code here
        let batch = vec![LshEntry::new(0, 0x123, 42)];
        let gen = log.append_batch(&batch).unwrap();
        assert_eq!(gen, 0);

        // Cleanup
        let _ = log.truncate();
        let _ = std::fs::remove_file(log_path);
    }
}
```

## Best Practices

### 1. Always Cleanup Logs
```rust
// Good: Log cleared after successful processing
let _ = log.truncate()?;

// Avoid: Accumulating logs over time
// (Causes disk space issues)
```

### 2. Validate Checksums Periodically
```rust
// On startup or periodic health check
if let Ok(valid) = log.verify_checksum() {
    if !valid {
        eprintln!("WARNING: Log corruption detected!");
    }
}
```

### 3. Use Batch Sizes Based on Workload
```rust
// I/O bound (network): Large batches (5000)
// CPU bound (parsing): Medium batches (1000)
// Latency sensitive (web): Small batches (100)
```

### 4. Handle Concurrent Access
```rust
// If multiple threads access same log:
// - Use Arc<TransactionLogCapsule>
// - Serialization via Mutex (internal file handle)
// - Safe but not lock-free at application level

let log = Arc::new(TransactionLogCapsule::new("dedup.log")?);
let log_clone = Arc::clone(&log);
std::thread::spawn(move || {
    // Use log_clone in thread
});
```

### 5. Monitor Performance
```rust
use std::time::Instant;

let start = Instant::now();
let _gen = log.append_batch(&batch)?;
let elapsed = start.elapsed();

if elapsed.as_millis() > 10 {
    eprintln!("Slow append: {}ms", elapsed.as_millis());
    // Investigate: disk latency, batch size, etc.
}
```

## Troubleshooting

### Issue: "Permission denied" errors
**Solution**: Ensure write permissions to log directory
```bash
chmod 755 $(dirname dedup.log)
chmod 644 dedup.log  # If file exists
```

### Issue: "Disk full" errors
**Solution**: Check disk space and implement rotation
```rust
fn check_disk_space() -> Result<u64> {
    use std::fs;
    let metadata = fs::metadata(".")?;
    Ok(metadata.st_size())  // Available space (platform-specific)
}
```

### Issue: Slow appends (>5ms)
**Solution**: Check disk performance
```bash
# Benchmark SSD
dd if=/dev/zero of=/tmp/test bs=1M count=100 oflag=fsync
# Should complete in ~100ms (1MB/sec for fsync-heavy workload)
```

### Issue: Recovery takes too long
**Solution**: Reduce batch size or implement log rotation
```rust
// Smaller logs → faster recovery
// 1GB log with 1K-entry batches = 1M batches
// Replay time: ~1M * 100μs = 100s (too slow)
// Solution: Rotate every 100MB → 10 logs, faster recovery
```

## Integration Examples

### With BatchLshIndexCapsule
```rust
// (future integration)
pub struct BatchLshIndexCapsule {
    txn_log: TransactionLogCapsule,
    lsh_index: LshIndex,
    buffer: Vec<LshEntry>,
}

impl BatchLshIndexCapsule {
    pub fn insert(&mut self, entry: LshEntry) -> Result<()> {
        self.buffer.push(entry);

        if self.buffer.len() >= 1000 {
            self.flush()?;
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        let batch = std::mem::take(&mut self.buffer);
        let _gen = self.txn_log.append_batch(&batch)?;
        self.lsh_index.insert_batch(&batch)?;
        Ok(())
    }
}
```

### With Persistent Pipeline
```rust
// (compatible architecture)
pub struct PersistentDedupPipeline {
    lsh_bucketer: MmapLshBucketer,  // T9 Persistent
    txn_log: TransactionLogCapsule,  // T9 Persistent (crash recovery)
    generation: AtomicU64,           // T1 Atomic (coordination)
}
```

## References

- **Implementation**: `src/lsh/transaction_log.rs`
- **Tests**: `tests/transaction_log_integration.rs`
- **Detailed Docs**: `docs/TRANSACTION_LOG_CAPSULE.md`
- **Framework**: `/home/samuel/CLAUDE.md` § UCE34
