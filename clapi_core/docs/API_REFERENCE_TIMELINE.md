# API Reference - Timeline Aggregation Capsule

**Version**: 0.4.9
**Tier**: T4 (Batch)
**Architecture**: 100% lockfree (no mutex/RwLock)

Complete API reference for Timeline Aggregation computational capsule.

---

## Table of Contents

1. [Core Types](#core-types)
2. [TimelineAggregationCapsule](#timelineaggregationcapsule-sync)
3. [TimelineBridge](#timelinebridge-async)
4. [BucketSnapshot](#bucketsnapshot)
5. [Error Handling](#error-handling)
6. [Performance Characteristics](#performance-characteristics)

---

## Core Types

### BucketGranularity

Time window granularity for buckets.

```rust
pub enum BucketGranularity {
    Minute,  // 60 seconds
    Hour,    // 3600 seconds
    Day,     // 86400 seconds
}
```

**Methods**:

```rust
impl BucketGranularity {
    // Get duration in seconds (const fn, compile-time)
    pub const fn duration_secs(self) -> u64;

    // Convert from u8 (0=Minute, 1=Hour, 2=Day)
    pub fn from_u8(value: u8) -> Self;
}
```

**Examples**:

```rust
let minute = BucketGranularity::Minute;
assert_eq!(minute.duration_secs(), 60);

let hour = BucketGranularity::from_u8(1);
assert_eq!(hour, BucketGranularity::Hour);
```

---

### BucketStatus

Bucket lifecycle status.

```rust
pub enum BucketStatus {
    Active,    // Accepting events
    Complete,  // Time boundary crossed, no longer accepting events
    Flushed,   // Persisted to disk (hash computed)
}
```

**Methods**:

```rust
impl BucketStatus {
    pub fn from_u8(value: u8) -> Self;
}
```

**State Transitions**:

```
Active → Complete → Flushed
   ↓         ↓          ↓
append()  mark_complete()  flush_bucket()
```

---

## TimelineAggregationCapsule (Sync)

Core lockfree timeline capsule for synchronous usage.

### Creation

```rust
impl TimelineAggregationCapsule {
    pub fn new(
        start_ts: u64,               // Timeline start timestamp (epoch seconds)
        granularity: BucketGranularity,  // Bucket duration
        capacity: usize,             // Maximum number of buckets
    ) -> Self;
}
```

**Parameters**:
- `start_ts`: Timeline start time in epoch seconds (Jan 1, 1970 UTC)
- `granularity`: Bucket duration (Minute/Hour/Day)
- `capacity`: Maximum buckets (timeline tracks last `capacity` buckets)

**Examples**:

```rust
use clapi_core::capsules::{TimelineAggregationCapsule, BucketGranularity};

// 24 hours at minute resolution
let timeline = TimelineAggregationCapsule::new(
    1000,                          // Start at epoch 1000
    BucketGranularity::Minute,     // 60-second buckets
    1440,                          // 24 hours × 60 minutes
);

// 7 days at hour resolution
let timeline = TimelineAggregationCapsule::new(
    1000,
    BucketGranularity::Hour,
    168,  // 7 days × 24 hours
);

// 1 year at day resolution
let timeline = TimelineAggregationCapsule::new(
    1000,
    BucketGranularity::Day,
    365,
);
```

---

### Core Methods

#### append()

Append event to timeline (lockfree, <100ns).

```rust
pub fn append(&self, timestamp: u64) -> ClapiResult<()>;
```

**Parameters**:
- `timestamp`: Event timestamp (epoch seconds)

**Returns**:
- `Ok(())` on success
- `Err(ClapiError::IoError)` if bucket not active or timestamp out of range

**Performance**: <100ns (lockfree atomic increment)

**Thread Safety**: Safe for concurrent calls from multiple threads

**Examples**:

```rust
use std::time::{SystemTime, UNIX_EPOCH};

// Append current time
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
timeline.append(now)?;

// Append specific timestamp
timeline.append(1234567890)?;

// Concurrent appends (safe)
std::thread::scope(|s| {
    for _ in 0..10 {
        s.spawn(|| {
            timeline.append(now).unwrap();
        });
    }
});
```

**Error Handling**:

```rust
match timeline.append(timestamp) {
    Ok(()) => println!("Event recorded"),
    Err(e) => {
        if e.to_string().contains("Bucket not active") {
            // Timestamp out of range or bucket completed
            eprintln!("Timestamp {} is outside timeline range", timestamp);
        }
    }
}
```

---

#### query_bucket()

Query bucket by index (lockfree, <50ns).

```rust
pub fn query_bucket(&self, bucket_index: u64) -> ClapiResult<BucketSnapshot>;
```

**Parameters**:
- `bucket_index`: Bucket index (0-based, < capacity)

**Returns**:
- `Ok(BucketSnapshot)` with bucket data
- `Err(ClapiError::IoError)` if index out of bounds

**Performance**: <50ns (direct array index access)

**Examples**:

```rust
// Query first bucket
let snapshot = timeline.query_bucket(0)?;
println!("Bucket 0: {} events", snapshot.event_count);

// Query last 5 buckets
for i in 0..5 {
    let snapshot = timeline.query_bucket(i)?;
    println!("Bucket {}: {} events (status: {:?})",
             i, snapshot.event_count, snapshot.status);
}
```

---

#### query_bucket_at_ts()

Query bucket containing specific timestamp.

```rust
pub fn query_bucket_at_ts(&self, timestamp: u64) -> ClapiResult<BucketSnapshot>;
```

**Parameters**:
- `timestamp`: Event timestamp (epoch seconds)

**Returns**:
- `Ok(BucketSnapshot)` for bucket containing this timestamp
- `Err(ClapiError::IoError)` if timestamp out of range

**Examples**:

```rust
use std::time::{SystemTime, UNIX_EPOCH};

// Query bucket for current time
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
let snapshot = timeline.query_bucket_at_ts(now)?;
println!("Current bucket: {} events", snapshot.event_count);

// Query bucket for specific time
let snapshot = timeline.query_bucket_at_ts(1234567890)?;
```

**Bucket Index Calculation**:

```rust
// Internal formula (for reference):
let bucket_index = (timestamp - start_ts) / granularity.duration_secs();
```

---

#### flush_bucket()

Flush bucket and compute hash (10µs).

```rust
pub fn flush_bucket(&self, bucket_index: u64) -> ClapiResult<u64>;
```

**Parameters**:
- `bucket_index`: Bucket index to flush

**Returns**:
- `Ok(hash)` - FNV-1a hash of bucket (hash chain link)
- `Err(ClapiError::IoError)` if index out of bounds

**Performance**: <10µs (FNV-1a hash computation)

**Side Effects**:
- Marks bucket as `Flushed`
- Computes hash chain (depends on previous bucket hash)
- Bucket no longer accepts events

**Examples**:

```rust
// Flush bucket 0
let hash = timeline.flush_bucket(0)?;
println!("Bucket 0 flushed with hash: 0x{:x}", hash);

// Flush all buckets (build hash chain)
let capacity = timeline.capacity();
for i in 0..capacity {
    match timeline.flush_bucket(i) {
        Ok(hash) => println!("Bucket {} hash: 0x{:x}", i, hash),
        Err(e) => eprintln!("Flush error for bucket {}: {}", i, e),
    }
}
```

**Hash Chain Integrity**:

Hash chain provides tamper detection:
- Each bucket hash depends on previous bucket hash
- Modification to any bucket invalidates all subsequent hashes
- Enables audit trail reconstruction

---

### Metrics Methods

#### total_events()

Get total events processed (lockfree, <10ns).

```rust
pub fn total_events(&self) -> u64;
```

**Returns**: Total events appended since creation

**Performance**: <10ns (atomic read, Relaxed ordering)

**Examples**:

```rust
let count = timeline.total_events();
println!("Total events: {}", count);

// Calculate rate
let duration_secs = 3600;  // 1 hour
let rate_per_sec = count as f64 / duration_secs as f64;
println!("Rate: {:.2} events/sec", rate_per_sec);
```

---

#### head()

Get current head bucket index (lockfree, <10ns).

```rust
pub fn head(&self) -> u64;
```

**Returns**: Index of most recent bucket written to

**Performance**: <10ns (atomic read)

**Examples**:

```rust
let head_idx = timeline.head();
println!("Current head bucket: {}", head_idx);

// Query head bucket
let snapshot = timeline.query_bucket(head_idx)?;
println!("Head bucket has {} events", snapshot.event_count);
```

---

#### capacity()

Get maximum bucket capacity (const, 0ns).

```rust
pub fn capacity(&self) -> u64;
```

**Returns**: Maximum number of buckets

**Performance**: 0ns (direct field access, no atomic)

**Examples**:

```rust
let cap = timeline.capacity();
println!("Timeline capacity: {} buckets", cap);

let duration_secs = cap * 60;  // For minute buckets
println!("Timeline tracks {} seconds of history", duration_secs);
```

---

## TimelineBridge (Async)

Async/blocking bridge for timeline aggregation in tokio runtime.

### Creation

```rust
impl TimelineBridge {
    pub fn new(
        start_ts: u64,
        granularity: BucketGranularity,
        capacity: usize,
    ) -> Self;
}
```

**Side Effects**:
- Spawns background worker thread
- Creates MPSC channel (1024 capacity)
- Starts batch processing loop (100 events or 100ms timeout)

**Examples**:

```rust
use clapi_core::proxy::TimelineBridge;
use clapi_core::capsules::BucketGranularity;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let bridge = Arc::new(TimelineBridge::new(
        1000,
        BucketGranularity::Minute,
        1440,
    ));

    // Bridge is ready for async operations
}
```

---

### Async Methods

#### append_event()

Append event to timeline (async, <100ns channel send).

```rust
pub async fn append_event(&self, timestamp: u64) -> ClapiResult<()>;
```

**Parameters**:
- `timestamp`: Event timestamp (epoch seconds)

**Returns**:
- `Ok(())` if event queued for processing
- `Err(ClapiError::IoError)` if channel closed or full

**Performance**: <100ns (MPSC channel send, non-blocking)

**Batching**: Worker processes events in batches:
- Batch size: 100 events
- Timeout: 100ms
- Retry policy: Exponential backoff (3 attempts: 10ms, 20ms, 40ms)

**Examples**:

```rust
use std::time::{SystemTime, UNIX_EPOCH};

// Append single event
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
bridge.append_event(now).await?;

// Concurrent appends (safe)
let mut handles = vec![];
for i in 0..10 {
    let bridge = Arc::clone(&bridge);
    handles.push(tokio::spawn(async move {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        bridge.append_event(ts).await.unwrap();
    }));
}

// Wait for all appends
for handle in handles {
    handle.await.unwrap();
}

// Wait for worker to process batch
tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
```

---

#### query_range()

Query buckets in timestamp range (async).

```rust
pub async fn query_range(
    &self,
    start_ts: u64,
    end_ts: u64,
) -> ClapiResult<Vec<BucketSnapshot>>;
```

**Parameters**:
- `start_ts`: Range start (epoch seconds, inclusive)
- `end_ts`: Range end (epoch seconds, exclusive)

**Returns**:
- `Ok(Vec<BucketSnapshot>)` for all buckets in range
- `Err(ClapiError::IoError)` if range invalid

**Performance**: <1ms for 100 buckets (depends on range size)

**Examples**:

```rust
// Query last hour (60 minute buckets)
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
let start = now - 3600;  // 1 hour ago
let snapshots = bridge.query_range(start, now).await?;

println!("Buckets in last hour: {}", snapshots.len());
for (i, snapshot) in snapshots.iter().enumerate() {
    println!("  Bucket {}: {} events", i, snapshot.event_count);
}

// Calculate statistics
let total: u64 = snapshots.iter().map(|s| s.event_count).sum();
let avg = total as f64 / snapshots.len() as f64;
let max = snapshots.iter().map(|s| s.event_count).max().unwrap();

println!("Total: {}, Avg: {:.1}, Max: {}", total, avg, max);
```

---

#### flush_all()

Flush all buckets and build hash chain (async).

```rust
pub async fn flush_all(&self) -> ClapiResult<()>;
```

**Returns**:
- `Ok(())` on success
- `Err(ClapiError::IoError)` if flush fails

**Performance**: <100ms for 1000 buckets (depends on capacity)

**Side Effects**:
- Computes hash chain for all buckets
- Marks all buckets as `Flushed`
- No buckets accept events after flush

**Examples**:

```rust
// Flush all buckets at end of session
bridge.flush_all().await?;

println!("All buckets flushed");
println!("Last flushed: {}", bridge.last_flushed());

// Verify hash chain integrity (Phase 6)
// bridge.verify_hash_chain().await?;  // Future API
```

---

### Metrics Methods (Async Bridge)

#### total_events()

Get total events processed.

```rust
pub fn total_events(&self) -> u64;
```

**Returns**: Total events (includes in-flight batches)

**Examples**:

```rust
let count = bridge.total_events();
println!("Total events: {}", count);
```

---

#### error_count()

Get worker thread error count.

```rust
pub fn error_count(&self) -> u64;
```

**Returns**: Number of errors encountered by worker thread

**Examples**:

```rust
let errors = bridge.error_count();
if errors > 0 {
    println!("⚠️ Worker encountered {} errors", errors);
}
```

---

#### last_flushed()

Get last flushed bucket index.

```rust
pub fn last_flushed(&self) -> u64;
```

**Returns**: Index of last flushed bucket (0 if none flushed)

**Examples**:

```rust
let last = bridge.last_flushed();
println!("Last flushed bucket: {}", last);
```

---

## BucketSnapshot

Immutable snapshot of bucket state at query time.

```rust
pub struct BucketSnapshot {
    pub start_ts: u64,      // Bucket start timestamp (epoch seconds)
    pub end_ts: u64,        // Bucket end timestamp (exclusive)
    pub event_count: u64,   // Number of events in bucket
    pub prev_hash: u64,     // Hash of previous bucket (hash chain)
    pub hash: u64,          // Hash of this bucket
    pub status: BucketStatus, // Bucket status (Active/Complete/Flushed)
}
```

**Examples**:

```rust
let snapshot = timeline.query_bucket(0)?;

println!("Bucket 0:");
println!("  Time range: [{}, {})", snapshot.start_ts, snapshot.end_ts);
println!("  Events: {}", snapshot.event_count);
println!("  Status: {:?}", snapshot.status);
println!("  Hash: 0x{:x}", snapshot.hash);
println!("  Previous hash: 0x{:x}", snapshot.prev_hash);

// Check if bucket is empty
if snapshot.event_count == 0 {
    println!("  No events in this bucket");
}

// Check if bucket is flushed
if snapshot.status == BucketStatus::Flushed {
    println!("  Hash chain link: 0x{:x}", snapshot.hash);
}
```

---

## Error Handling

### ClapiError Types

Timeline operations return `ClapiResult<T>` where `Err` is:

```rust
pub enum ClapiError {
    // Bucket not active (timestamp out of range)
    IoError(String),

    // Other errors (see error.rs for complete list)
    // ...
}
```

### Error Classification

```rust
impl ClapiError {
    // Check if error is retryable
    pub fn is_retryable(&self) -> bool;

    // Get suggested action for error
    pub fn suggested_action(&self) -> &'static str;

    // Get error category
    pub fn category(&self) -> ErrorCategory;

    // Get alert severity
    pub fn alert_severity(&self) -> AlertSeverity;
}
```

**Examples**:

```rust
match timeline.append(timestamp) {
    Ok(()) => println!("Event recorded"),
    Err(e) => {
        eprintln!("Error: {}", e);
        eprintln!("Category: {:?}", e.category());
        eprintln!("Suggested action: {}", e.suggested_action());

        if e.is_retryable() {
            // Retry with exponential backoff
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            timeline.append(timestamp)?;
        } else {
            // Alert operations team
            eprintln!("⚠️ Non-retryable error, alerting ops team");
        }
    }
}
```

---

## Performance Characteristics

### Latency SLOs

| Operation | Target | Typical | Notes |
|-----------|--------|---------|-------|
| `append()` | <100ns | ~60ns | Lockfree atomic increment |
| `query_bucket()` | <50ns | ~30ns | Direct array index access |
| `flush_bucket()` | <10µs | ~5µs | FNV-1a hash computation |
| `query_range()` | <1ms | ~500µs | Multiple bucket reads (100 buckets) |
| `total_events()` | <10ns | ~5ns | Atomic read (Relaxed) |
| `capacity()` | 0ns | 0ns | Direct field access |

### Throughput

| Operation | Single Thread | 8 Threads | Scaling |
|-----------|---------------|-----------|---------|
| `append()` | 10M ops/sec | 60M ops/sec | Linear (lockfree) |
| `query_bucket()` | 20M ops/sec | 100M ops/sec | Linear (read-only) |
| `flush_bucket()` | 100K ops/sec | 400K ops/sec | Sublinear (hash compute) |

### Memory Usage

| Configuration | Memory | Notes |
|---------------|--------|-------|
| 1440 buckets (24h @ 1min) | ~92KB | 64B per bucket |
| 168 buckets (7d @ 1h) | ~11KB | 64B per bucket |
| 365 buckets (1y @ 1d) | ~23KB | 64B per bucket |

**Formula**: `memory_bytes = capacity × 64`

---

## Thread Safety

All operations are **100% thread-safe** and **lockfree**:

- **append()**: Lockfree atomic increment (safe concurrent calls)
- **query_bucket()**: Lockfree read (no synchronization required)
- **flush_bucket()**: Atomic CAS for state transition (safe concurrent flush)
- **total_events()**: Lockfree atomic read (Relaxed ordering)

**Tested**: 1000-thread concurrent append stress tests (T28 compliance)

---

## Advanced Topics

### Hash Chain Integrity

Hash chain provides tamper detection:

```rust
// Build hash chain
for i in 0..capacity {
    let hash = timeline.flush_bucket(i)?;
    println!("Bucket {} hash: 0x{:x}", i, hash);
}

// Verify integrity (manual)
let snapshot0 = timeline.query_bucket(0)?;
let snapshot1 = timeline.query_bucket(1)?;

// Bucket 1's prev_hash must equal bucket 0's hash
assert_eq!(snapshot1.prev_hash, snapshot0.hash);
```

### Custom Aggregations

Build custom aggregations on top of timeline:

```rust
// Calculate moving average
fn moving_average(bridge: &TimelineBridge, window_size: usize) -> f64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let start = now - (window_size as u64 * 60);  // Minute buckets

    let snapshots = bridge.query_range(start, now).await.unwrap();
    let total: u64 = snapshots.iter().map(|s| s.event_count).sum();

    total as f64 / snapshots.len() as f64
}
```

---

## See Also

- **[Quick Start](QUICKSTART_TIMELINE.md)** - 5-minute getting started guide
- **[Complete Examples](EXAMPLES_TIMELINE.md)** - 5 production-ready examples
- **[Troubleshooting](TROUBLESHOOTING_TIMELINE.md)** - Common errors and solutions
- **[Architecture](ARCHITECTURE_OVERVIEW.md)** - Deep dive into T4 Batch tier design
