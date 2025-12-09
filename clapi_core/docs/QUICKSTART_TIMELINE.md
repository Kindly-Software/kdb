# Quick Start - Timeline Aggregation

**Goal**: Get started with Timeline Aggregation in 5 minutes

**What is Timeline Aggregation?** A T4 Batch tier computational capsule that tracks events over time using lockfree atomic operations. Perfect for:
- Tracking API request rates
- Monitoring queue depth trends
- Detecting performance degradation
- Audit trail analytics
- Real-time alerting on anomalies

**Performance**: <100ns append, <50ns query, 100% lockfree (no mutex/RwLock)

---

## Installation (30 seconds)

Add to your `Cargo.toml`:

```toml
[dependencies]
clapi_core = "0.4.9"
tokio = { version = "1", features = ["full"] }
```

---

## Hello World (30 seconds)

Track events in a 60-second timeline with minute-level granularity:

```rust
use clapi_core::capsules::{TimelineAggregationCapsule, BucketGranularity};

fn main() {
    // Create timeline: start at epoch 1000, minute buckets, 1440 capacity (24 hours)
    let timeline = TimelineAggregationCapsule::new(
        1000,                          // Start timestamp (epoch seconds)
        BucketGranularity::Minute,     // 60-second buckets
        1440,                          // 24 hours of buckets
    );

    // Record events (< 100ns each, lockfree)
    timeline.append(1000).unwrap();     // First minute
    timeline.append(1060).unwrap();     // Second minute
    timeline.append(1120).unwrap();     // Third minute

    // Query bucket 0 (first minute)
    let snapshot = timeline.query_bucket(0).unwrap();
    println!("Events in first minute: {}", snapshot.event_count);
    // Output: Events in first minute: 1

    println!("Total events: {}", timeline.total_events());
    // Output: Total events: 3
}
```

**That's it!** You now have a lockfree timeline tracker.

---

## Core Concepts (2 minutes)

### 1. **Buckets** - Time Windows

Buckets divide time into fixed windows:

```rust
// Minute buckets: 60 seconds each
BucketGranularity::Minute   // Use for: real-time monitoring (1-24 hours)

// Hour buckets: 3600 seconds each
BucketGranularity::Hour     // Use for: daily/weekly trends

// Day buckets: 86400 seconds each
BucketGranularity::Day      // Use for: long-term analytics (months/years)
```

**Example**: With `BucketGranularity::Minute` and capacity 1440, you track 24 hours at 60-second resolution.

### 2. **Timestamps** - Epoch Seconds

All timestamps are **epoch seconds** (seconds since Jan 1, 1970 UTC):

```rust
use std::time::{SystemTime, UNIX_EPOCH};

// Get current timestamp
let now = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_secs();

timeline.append(now).unwrap();
```

### 3. **Capacity** - Maximum Buckets

Capacity determines how much history you can track:

```rust
// 24 hours at minute resolution
let timeline = TimelineAggregationCapsule::new(
    start_ts,
    BucketGranularity::Minute,
    1440,  // 24 hours × 60 minutes
);

// 7 days at hour resolution
let timeline = TimelineAggregationCapsule::new(
    start_ts,
    BucketGranularity::Hour,
    168,  // 7 days × 24 hours
);

// 365 days at day resolution
let timeline = TimelineAggregationCapsule::new(
    start_ts,
    BucketGranularity::Day,
    365,  // 1 year
);
```

---

## Common Use Cases (3 minutes)

### Use Case 1: Track API Request Rate

Monitor requests per second in real-time:

```rust
use clapi_core::capsules::{TimelineAggregationCapsule, BucketGranularity};
use std::time::{SystemTime, UNIX_EPOCH};

// Setup: Track last 1 hour at 60-second resolution
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
let timeline = TimelineAggregationCapsule::new(
    now,
    BucketGranularity::Minute,
    60,  // 1 hour = 60 minutes
);

// On each API request:
fn handle_request(timeline: &TimelineAggregationCapsule) {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    timeline.append(ts).unwrap();
}

// Every 60 seconds, check rate:
fn check_rate(timeline: &TimelineAggregationCapsule) {
    let total = timeline.total_events();
    let rate_per_min = total as f64 / 60.0;
    println!("Request rate: {:.1} req/min", rate_per_min);
}
```

**Output**: "Request rate: 127.5 req/min"

### Use Case 2: Monitor Queue Depth

Detect growing queues before they overflow:

```rust
use std::time::{SystemTime, UNIX_EPOCH, Duration};

// Track queue depth every second
fn monitor_queue(timeline: &TimelineAggregationCapsule, queue_size: usize) {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // Record event for each queued item
    for _ in 0..queue_size {
        timeline.append(ts).unwrap();
    }
}

// Check if queue is growing (every 10 seconds)
fn check_queue_trend(timeline: &TimelineAggregationCapsule) -> bool {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // Get counts for last 2 buckets (2 minutes)
    let prev_bucket = timeline.query_bucket_at_ts(now - 60).unwrap();
    let curr_bucket = timeline.query_bucket_at_ts(now).unwrap();

    // Alert if queue grew >20%
    if curr_bucket.event_count > prev_bucket.event_count * 120 / 100 {
        println!("⚠️ Queue growing! {} → {} events",
                 prev_bucket.event_count, curr_bucket.event_count);
        return true;
    }
    false
}
```

**Output**: "⚠️ Queue growing! 1200 → 1560 events"

### Use Case 3: Detect Performance Degradation

Track operation latency and alert on slowdowns:

```rust
// Track slow operations (>100ms) over time
fn track_slow_operation(timeline: &TimelineAggregationCapsule, duration_ms: u64) {
    if duration_ms > 100 {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        timeline.append(ts).unwrap();
    }
}

// Alert if slow operations increasing
fn check_performance_degradation(timeline: &TimelineAggregationCapsule) {
    // Compare last 5 minutes vs previous 5 minutes
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    let recent_count = count_events_in_range(timeline, now - 300, now);
    let prev_count = count_events_in_range(timeline, now - 600, now - 300);

    if recent_count > prev_count * 2 {
        println!("⚠️ Performance degrading! Slow ops doubled: {} → {}",
                 prev_count, recent_count);
    }
}

fn count_events_in_range(
    timeline: &TimelineAggregationCapsule,
    start_ts: u64,
    end_ts: u64,
) -> u64 {
    // Implementation depends on your query pattern
    // See docs/API_REFERENCE.md for query_range() method
    timeline.total_events()  // Simplified for example
}
```

**Output**: "⚠️ Performance degrading! Slow ops doubled: 12 → 27"

---

## Async Usage with TimelineBridge (3 minutes)

For async/tokio applications, use `TimelineBridge`:

```rust
use clapi_core::proxy::TimelineBridge;
use clapi_core::capsules::BucketGranularity;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() {
    // Create async bridge (spawns background worker thread)
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let bridge = Arc::new(TimelineBridge::new(
        now,
        BucketGranularity::Minute,
        1440,  // 24 hours
    ));

    // Spawn concurrent event appenders
    let mut handles = vec![];
    for i in 0..4 {
        let bridge = Arc::clone(&bridge);
        handles.push(tokio::spawn(async move {
            for j in 0..25 {
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                bridge.append_event(ts).await.unwrap();
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }));
    }

    // Wait for completion
    for handle in handles {
        handle.await.unwrap();
    }

    // Wait for worker to process batch
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Query results
    println!("Total events: {}", bridge.total_events());
    println!("Error count: {}", bridge.error_count());

    // Query range
    match bridge.query_range(now, now + 3600).await {
        Ok(snapshots) => {
            println!("\nBuckets in range:");
            for (i, snapshot) in snapshots.iter().enumerate() {
                println!("  Bucket {}: {} events", i, snapshot.event_count);
            }
        }
        Err(e) => println!("Query error: {}", e),
    }
}
```

**Key differences**:
- `TimelineBridge::new()` spawns background worker thread
- `append_event()` is async (uses MPSC channel)
- Batch processing: 100 events or 100ms timeout
- Automatic error handling with exponential backoff retry

---

## Next Steps

### Learn More

- **[API Reference](API_REFERENCE_TIMELINE.md)** - Complete API documentation
- **[Complete Examples](EXAMPLES_TIMELINE.md)** - 5 production-ready examples
- **[Troubleshooting](TROUBLESHOOTING_TIMELINE.md)** - Common errors and solutions
- **[Architecture](ARCHITECTURE_OVERVIEW.md)** - Deep dive into design

### Key API Methods

```rust
// Core operations
timeline.append(timestamp) -> Result<()>              // Add event (<100ns)
timeline.query_bucket(index) -> Result<BucketSnapshot>  // Query bucket (<50ns)
timeline.total_events() -> u64                        // Get total count
timeline.flush_bucket(index) -> Result<u64>           // Compute hash chain

// Async bridge operations
bridge.append_event(ts).await -> Result<()>           // Async append
bridge.query_range(start, end).await -> Result<Vec>   // Range query
bridge.flush_all().await -> Result<()>                // Flush all buckets
```

### Performance Targets

| Operation | Latency | Throughput | Notes |
|-----------|---------|------------|-------|
| `append()` | <100ns | 10M ops/sec | Lockfree atomic increment |
| `query_bucket()` | <50ns | 20M ops/sec | Direct index access |
| `flush_bucket()` | <10µs | 100K ops/sec | Hash chain computation |
| `query_range()` | <1ms | 1K ops/sec | Multiple bucket reads |

All operations are **100% lockfree** (no mutex, no RwLock, no blocking).

---

## Common Gotchas

### 1. Timestamp out of range

**Error**: "Bucket not active"

**Cause**: Timestamp before `start_ts` or after `start_ts + (capacity × bucket_duration)`

**Fix**: Ensure timestamps are within timeline range:

```rust
let start_ts = 1000;
let timeline = TimelineAggregationCapsule::new(
    start_ts,
    BucketGranularity::Minute,
    60,  // 60 minutes = 1 hour
);

// ❌ Wrong: Timestamp before start
timeline.append(500).unwrap();  // Error!

// ❌ Wrong: Timestamp after end (start + 60 minutes = 4600)
timeline.append(5000).unwrap();  // Error!

// ✅ Right: Within range [1000, 4600)
timeline.append(1234).unwrap();  // OK
```

### 2. Async bridge worker lag

**Symptom**: Events appended but `total_events()` returns 0

**Cause**: Background worker hasn't processed batch yet (100ms timeout)

**Fix**: Wait for worker to flush:

```rust
bridge.append_event(ts).await.unwrap();

// Wait for worker batch flush
tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

// Now query will show events
assert!(bridge.total_events() > 0);
```

### 3. Bucket index vs timestamp confusion

Timeline supports both **bucket index** (0, 1, 2...) and **timestamp** queries:

```rust
// Query by bucket index (0-based)
let snapshot = timeline.query_bucket(0)?;  // First bucket

// Query by timestamp
let snapshot = timeline.query_bucket_at_ts(1234)?;  // Bucket containing ts 1234
```

---

## FAQ

**Q: Can I use this in production?**
A: Yes! Timeline Aggregation is production-ready (v0.4.9), fully tested (T28 compliance), and validated with B32 benchmarking.

**Q: Is it safe for concurrent access?**
A: Yes! 100% lockfree using atomic operations. Tested with 1000-thread concurrent appends.

**Q: Can I persist timelines to disk?**
A: Not yet. Phase 6 adds persistence via checkpoint mechanism. See [P1_HIGH_PRIORITY_ENHANCEMENTS.md](../P1_HIGH_PRIORITY_ENHANCEMENTS.md).

**Q: What happens if I exceed capacity?**
A: Old buckets are overwritten (circular buffer pattern). Timeline tracks last `capacity` buckets only.

**Q: Can I track custom metadata per event?**
A: Not directly. Timeline tracks event counts only (optimized for ultra-low latency). For metadata, use separate data structure indexed by bucket.

**Q: How do I convert SystemTime to epoch seconds?**
```rust
use std::time::{SystemTime, UNIX_EPOCH};
let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
```

---

## Need Help?

- **Issues**: [GitHub Issues](https://github.com/kindly/clapi_core/issues)
- **Discussions**: [GitHub Discussions](https://github.com/kindly/clapi_core/discussions)
- **Documentation**: See `docs/` directory for comprehensive guides
- **Examples**: See `examples/timeline_aggregation_demo.rs` for runnable code

---

**Next**: Read [API_REFERENCE_TIMELINE.md](API_REFERENCE_TIMELINE.md) for complete API documentation
