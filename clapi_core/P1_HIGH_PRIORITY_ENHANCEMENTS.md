# P1 High Priority Enhancements - Timeline Aggregation Capsule

**Status**: OPERATIONAL PAIN - Improves deployment, automation, and developer experience
**Total Issues**: 28 P1 high priority issues
**Impact**: 2-3 week deployment effort, enables sustained operations

---

## Table of Contents

1. [Automated Deployment (1 issue)](#automated-deployment)
2. [Documentation (5 issues)](#documentation)
3. [Testing (5 issues)](#testing)
4. [Performance Validation (3 issues)](#performance-validation)
5. [Developer Convenience (7 issues)](#developer-convenience)
6. [Error Messaging (4 issues)](#error-messaging)
7. [Integration (3 issues)](#integration)

---

## Automated Deployment

### Enhancement 1: One-Command Deploy Script

**Current State**: 6 manual steps (error-prone)

**Status**: Already documented in P0_CRITICAL_ENHANCEMENTS.md (Enhancement 10)

---

## Documentation

### Enhancement 2: Quick Start Guide (5-min Hello World)

**Current State**: 30-min learning curve to understand APIs

**Implementation**: Create docs/QUICKSTART.md

```markdown
# Quick Start Guide - Timeline Aggregation

Get started in 5 minutes.

## Installation

```toml
[dependencies]
clapi_core = "0.4.9"
```

## Hello World (30 seconds)

```rust
use clapi_core::capsules::TimelineAggregationCapsuleWrapper;
use std::time::SystemTime;

#[tokio::main]
async fn main() {
    // Create timeline (tracks events in 60-second buckets)
    let timeline = TimelineAggregationCapsuleWrapper::new(
        num_buckets: 1440,      // 24 hours at 60s resolution
        bucket_duration_secs: 60,
    ).unwrap();

    // Record an event (< 1 microsecond)
    timeline.append_system_time(SystemTime::now()).unwrap();

    // Query events from last hour
    let stats = timeline.query_last_hours(1).unwrap();
    println!("Events in last hour: {}", stats.total_count);
}
```

## Common Use Cases

### Use Case 1: Track API Request Rate

```rust
// On each request
timeline.append_system_time(SystemTime::now())?;

// Every 60 seconds, report metrics
let stats = timeline.query_last_hours(1)?;
println!("Requests/hour: {}", stats.total_count);
```

### Use Case 2: Monitor Queue Depth

```rust
// Record queue events every second
for item in queue.iter() {
    timeline.append_system_time(SystemTime::now())?;
}

// Check trends
let last_min = timeline.query_last_hours(1/60)?;
let this_min = timeline.query_last_hours(1/120)?;
if this_min.total_count > last_min.total_count {
    println!("⚠️ Queue growing!");
}
```

### Use Case 3: Track User Activity

```rust
// Record user login
timeline.append_system_time(SystemTime::now())?;

// Get active users in last 24 hours
let users = timeline.query_last_hours(24)?;
println!("Active users: {}", users.unique_count);
```

## API Reference

See docs/API_GUIDE.md for complete API documentation.

## Troubleshooting

**Q: "Bucket not active" error?**
A: Bucket already flushed (data in past). Use current time only.

**Q: Why only timestamps, not custom data?**
A: Intentional design for ultra-low latency (78ns vs 1µs+ with data).

## Next Steps

- [API Guide](docs/API_GUIDE.md) - Full API documentation
- [Performance](docs/PERFORMANCE.md) - Latency SLOs and tuning
- [Architecture](docs/ARCHITECTURE.md) - Deep dive into design

---

**Need help?** Create an issue at https://github.com/kindly/clapi_core/issues
```

**Key Features**:
- Copy-paste ready code
- 3 common use cases
- Troubleshooting section
- Links to detailed docs

**UCE34 Analysis**:
- **Q1**: Problem: 30-min barrier to entry
- **Q28**: Simplicity: 5-min Hello World
- **Q31**: Constraints: Zero configuration needed

**Acceptance Criteria**:
- [ ] Quickstart tested (code compiles + runs)
- [ ] All 3 use cases work
- [ ] <5 minutes to first working code
- [ ] Links to deeper docs for each section

---

### Enhancement 3: Inline Examples in Code

**Current State**: No rustdoc examples

**Implementation**: Add examples to public methods

```rust
// src/capsules/timeline_aggregation_capsule.rs
impl TimelineAggregationCapsuleWrapper {
    /// Append timestamp to timeline.
    ///
    /// Records current system time in the appropriate bucket.
    /// Ultra-low latency (<100ns) suitable for hot paths.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use clapi_core::capsules::TimelineAggregationCapsuleWrapper;
    /// use std::time::SystemTime;
    ///
    /// let timeline = TimelineAggregationCapsuleWrapper::new(1440, 60)?;
    /// timeline.append_system_time(SystemTime::now())?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Bucket capacity exceeded
    /// - Worker thread dead
    /// - Timestamp before UNIX_EPOCH
    ///
    /// # Performance
    ///
    /// - Latency: <100ns (p99)
    /// - Throughput: 10M ops/sec (single thread)
    /// - Memory: O(1) per append (no allocation)
    pub fn append_system_time(&self, time: SystemTime) -> Result<()> {
        // ...
    }

    /// Query event count for last N hours.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let timeline = TimelineAggregationCapsuleWrapper::new(1440, 60)?;
    /// timeline.append_system_time(SystemTime::now())?;
    ///
    /// // Get last hour
    /// let stats = timeline.query_last_hours(1)?;
    /// assert!(stats.total_count > 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn query_last_hours(&self, hours: u64) -> Result<TimelineRange> {
        // ...
    }
}
```

**UCE34 Analysis**:
- **Q1**: Problem: No example usage visible
- **Q28**: Simplicity: Copy-paste examples in docs
- **Q31**: Constraints: Examples must compile

**Acceptance Criteria**:
- [ ] All 10+ public methods have examples
- [ ] All examples compile without error
- [ ] Examples show both success and error paths
- [ ] Doctests pass in CI

---

### Enhancement 4: Troubleshooting Guide

**Current State**: Common errors lack guidance

**Implementation**: Create docs/TROUBLESHOOTING.md

```markdown
# Troubleshooting Guide

## Common Errors and Solutions

### Error: "Bucket not active"

**Cause**: Attempting to query a bucket that has already transitioned to Complete state.

**Solution**:
- Only query active buckets (current time ± 1 bucket window)
- Use `query_last_hours()` instead of direct bucket queries
- Ensure system clock is accurate

**Example**:
```rust
// ❌ Wrong: Query past bucket
let past = SystemTime::now() - Duration::from_secs(3600);
timeline.query_bucket_system_time(past)?;  // Error!

// ✅ Right: Use range query
let stats = timeline.query_last_hours(1)?;
```

### Error: "Worker thread dead"

**Cause**: Background worker thread crashed (rare)

**Solution**:
1. Check logs: `sudo journalctl -u timeline-aggregation`
2. Restart service: `sudo systemctl restart timeline-aggregation`
3. If persistent, check memory pressure:
   - `free -h` - Check available RAM
   - `dmesg | tail -20` - Check for OOM kills

**Prevention**:
- Set memory limit: `SystemMemoryLimit=8G` in systemd config
- Monitor memory: Dashboard → System → Memory

### Error: "Hash chain integrity violation"

**Cause**: Data corruption detected (possible tampering or bug)

**Severity**: CRITICAL

**Solution**:
1. Save checkpoint: `cp ~/.timeline_checkpoint ~/.timeline_checkpoint.backup`
2. Restart service: `sudo systemctl restart timeline-aggregation`
3. Report to security team
4. Run hash chain validation test

**Prevention**:
- Enable background hash chain monitoring
- Dashboard → Health → Hash Chain Integrity

### Error: "SystemTime before UNIX_EPOCH"

**Cause**: System clock skew (clock set to past)

**Solution**:
1. Check system time: `date`
2. Fix clock: `sudo timedatectl set-ntp true`
3. Verify NTP: `timedatectl`

**Prevention**:
- Enable NTP in production
- Monitor clock skew: Dashboard → System → Clock Skew

---

## Performance Issues

### Issue: High Append Latency (>1µs)

**Symptoms**: p99 latency > 1µs (normal: <450ns)

**Diagnosis Steps**:
```bash
# 1. Check CPU
top -b -n 1 | grep -E "Cpu|MEM"

# 2. Check memory pressure
free -h
cat /proc/pressure/memory

# 3. Check jitter
timeline.perf stats  # View latency histogram
```

**Solutions** (in order of likelihood):
1. **CPU contention**: Reduce other workloads
2. **GC pauses**: Use jemalloc allocator
3. **Memory pressure**: Increase available RAM
4. **Cache conflicts**: Review workload pattern

### Issue: Memory Leaks

**Symptoms**: Memory grows unbounded over hours

**Diagnosis**:
```bash
# Watch memory over time
watch -n 5 'ps aux | grep timeline | grep -v grep'

# Check resident memory
pmap -x $(pgrep timeline)
```

**Solutions**:
1. Check for unclosed connections
2. Verify worker thread cleanup
3. Monitor checkpoint file size

---

## Deployment Issues

### Problem: Service won't start after deploy

**Symptoms**: `systemctl status timeline-aggregation` shows "dead"

**Diagnosis**:
```bash
# Check logs
sudo journalctl -u timeline-aggregation -n 50

# Check binary
./target/release/timeline-aggregation --version
```

**Solutions**:
1. Permissions: `sudo chown root:root /usr/local/bin/timeline-aggregation`
2. Compatibility: `ldd /usr/local/bin/timeline-aggregation`
3. Rollback: `./scripts/rollback.sh`

---

## Monitoring Issues

### Problem: No metrics in Grafana

**Symptoms**: Dashboard shows "No data"

**Diagnosis**:
```bash
# Check metrics endpoint
curl http://localhost:8000/timeline/metrics | head -20

# Check Prometheus scrape
curl http://localhost:9090/api/v1/targets
```

**Solutions**:
1. Start service: `sudo systemctl start timeline-aggregation`
2. Check scrape config in Prometheus
3. Check firewall: `sudo ufw status`

---

## FAQ

**Q: Can I use old timestamps?**
A: Yes, but only within the last 24 hours (1440 buckets × 60s)

**Q: What happens if I append faster than 10K ops/sec?**
A: Queue fills (10K capacity), append() returns error after timeout

**Q: Can I persist data to disk?**
A: Not natively. Use persistence API or checkpoint mechanism

**Q: How do I scale to multiple machines?**
A: Each machine runs independent Timeline. Use aggregation layer.

---

## Still Having Issues?

1. Check health endpoint: `curl http://localhost:8000/timeline/health`
2. Enable debug logging: `RUST_LOG=debug ./timeline-aggregation`
3. Run diagnostics: `./scripts/doctor.sh`
4. Create an issue: https://github.com/kindly/clapi_core/issues
```

**UCE34 Analysis**:
- **Q1**: Problem: Errors lack guidance
- **Q28**: Simplicity: Common problems + solutions
- **Q31**: Constraints: Troubleshooting <10 minutes

**Acceptance Criteria**:
- [ ] 10+ common errors documented
- [ ] All solutions tested
- [ ] Diagnostics commands included
- [ ] FAQ covers 80% of support questions

---

### Enhancement 5: Split Architecture Spec

**Current State**: 925-line spec (intimidating)

**Implementation**: Split into sections

```
docs/
├── QUICKSTART.md               (5 min read)
├── API_GUIDE.md                (15 min read)
├── ARCHITECTURE_OVERVIEW.md    (20 min read) ← New
├── ARCHITECTURE_DEEP_DIVE.md   (1 hour read) ← New
├── PERFORMANCE.md              (30 min read) ← New
└── TROUBLESHOOTING.md          (reference)
```

**ARCHITECTURE_OVERVIEW.md** (200 lines):
- High-level design
- Three APIs (Core/Wrapper/Bridge)
- Memory layout
- Performance targets

**ARCHITECTURE_DEEP_DIVE.md** (725 lines):
- Detailed algorithms
- Memory safety analysis
- Concurrency patterns
- Failure modes

**UCE34 Analysis**:
- **Q1**: Problem: 925 lines too long
- **Q28**: Simplicity: Modular documentation
- **Q31**: Constraints: Overview <20 min read time

**Acceptance Criteria**:
- [ ] Overview readable in 20 minutes
- [ ] Deep dive for each tier
- [ ] Examples in each section
- [ ] Cross-links between docs

---

### Enhancement 6: Integration Guide (Grafana + Prometheus)

**Current State**: No deployment guide for monitoring integration

**Implementation**: Create docs/INTEGRATION_GUIDE.md

```markdown
# Integration Guide - Grafana + Prometheus

## Step 1: Prometheus Configuration

```yaml
# /etc/prometheus/prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'timeline'
    static_configs:
      - targets: ['localhost:8000']
    metrics_path: '/timeline/metrics/prometheus'
```

Restart Prometheus:
```bash
sudo systemctl restart prometheus
```

## Step 2: Grafana Dashboard

Import dashboard: `dashboards/timeline_aggregation.json`

```bash
curl -X POST http://localhost:3000/api/dashboards/db \
  -H "Authorization: Bearer $GRAFANA_TOKEN" \
  -d @dashboards/timeline_aggregation.json
```

## Step 3: Alert Rules

```yaml
# /etc/prometheus/rules/timeline.yml
groups:
  - name: timeline_aggregation
    interval: 30s
    rules:
      - alert: TimelineWorkerDead
        expr: timeline_worker_alive == 0
        for: 1m
        annotations:
          summary: "Timeline worker thread dead"

      - alert: TimelineHashChainBroken
        expr: timeline_flush_hash_chain_breaks > 0
        for: 1m
        annotations:
          summary: "Timeline hash chain integrity failure"

      - alert: HighAppendLatency
        expr: timeline_append_latency_ns{quantile="0.99"} > 1000000
        for: 5m
        annotations:
          summary: "Timeline append p99 latency > 1ms"
```

Reload Prometheus:
```bash
curl -X POST http://localhost:9090/-/reload
```

---

## Complete Integration Example

See `examples/grafana_integration.rs` for runnable example.
```

**UCE34 Analysis**:
- **Q1**: Problem: Grafana setup unclear
- **Q28**: Simplicity: Copy-paste YAML configs
- **Q31**: Constraints**: Setup <30 minutes

**Acceptance Criteria**:
- [ ] Integration tested end-to-end
- [ ] YAML configs valid
- [ ] Grafana dashboard imports successfully
- [ ] Alerts trigger on simulated failures

---

## Testing

### Enhancement 7: Concurrent Test Builder

**Current State**: 70-100 lines boilerplate per concurrent test

**Implementation**: Create test utility

```rust
// src/test_utils/concurrent_test_builder.rs
pub struct ConcurrentTestBuilder {
    threads: usize,
    operations_per_thread: usize,
    randomness: f64,
    timeout_secs: u64,
}

impl ConcurrentTestBuilder {
    pub fn new() -> Self {
        Self {
            threads: 100,
            operations_per_thread: 1000,
            randomness: 0.1,
            timeout_secs: 10,
        }
    }

    pub fn threads(mut self, count: usize) -> Self {
        self.threads = count;
        self
    }

    pub fn ops_per_thread(mut self, count: usize) -> Self {
        self.operations_per_thread = count;
        self
    }

    pub fn randomness(mut self, ratio: f64) -> Self {
        self.randomness = ratio;
        self
    }

    pub fn run<F, R>(self, mut operation: F) -> ConcurrentTestResult<R>
    where
        F: FnMut(usize) -> R + Send + 'static,
        R: Send + 'static,
    {
        let start = Instant::now();
        let handles: Vec<_> = (0..self.threads)
            .map(|thread_id| {
                thread::spawn(move || {
                    let mut results = Vec::new();
                    for op_id in 0..self.operations_per_thread {
                        results.push(operation(op_id));
                    }
                    results
                })
            })
            .collect();

        let mut all_results = Vec::new();
        for handle in handles {
            all_results.extend(handle.join().unwrap());
        }

        ConcurrentTestResult {
            elapsed: start.elapsed(),
            operations: all_results.len(),
            threads: self.threads,
        }
    }
}

// Usage:
#[test]
fn test_concurrent_append() {
    let capsule = Arc::new(TimelineAggregationCapsuleCore::new(1440, 60).unwrap());

    let result = ConcurrentTestBuilder::new()
        .threads(1000)
        .ops_per_thread(100)
        .run(|_op_id| {
            let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            capsule.append(ts).is_ok()
        });

    assert!(result.operations == 100_000);
    assert!(result.elapsed < Duration::from_secs(10));
}
```

**Benefits**:
- From 70 lines → 10 lines per test
- Reusable across codebase
- Built-in result analysis
- Timeout protection

**UCE34 Analysis**:
- **Q1**: Problem: Test boilerplate reduces productivity
- **Q28**: Simplicity: Builder pattern API
- **Q31**: Constraints: 10 lines per test max

**Acceptance Criteria**:
- [ ] Builder supports 10+ common patterns
- [ ] Reduces boilerplate 70%
- [ ] 20+ tests use builder
- [ ] Zero logic duplication

---

### Enhancement 8: Test Fixture Library

**Current State**: Timestamp generation repeated 30+ times

**Implementation**: Create test fixtures

```rust
// src/test_utils/fixtures.rs
pub struct TimelineFixture {
    capsule: Arc<TimelineAggregationCapsuleWrapper>,
    events: Vec<SystemTime>,
}

impl TimelineFixture {
    pub fn new() -> Self {
        Self {
            capsule: Arc::new(TimelineAggregationCapsuleWrapper::new(1440, 60).unwrap()),
            events: Vec::new(),
        }
    }

    pub fn with_events(mut self, count: usize) -> Self {
        let now = SystemTime::now();
        for i in 0..count {
            let ts = now - Duration::from_secs(i as u64);
            self.capsule.append_system_time(ts).unwrap();
            self.events.push(ts);
        }
        self
    }

    pub fn with_concentrated_events(mut self, count: usize, bucket: usize) -> Self {
        let ts = SystemTime::UNIX_EPOCH + Duration::from_secs((bucket * 60) as u64);
        for _ in 0..count {
            self.capsule.append_system_time(ts).unwrap();
        }
        self
    }

    pub fn with_random_events(mut self, count: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for _ in 0..count {
            let secs = rng.gen_range(0..86400);  // Last 24 hours
            let ts = SystemTime::now() - Duration::from_secs(secs);
            self.capsule.append_system_time(ts).unwrap();
        }

        self
    }

    pub fn capsule(&self) -> Arc<TimelineAggregationCapsuleWrapper> {
        Arc::clone(&self.capsule)
    }
}

// Usage:
#[test]
fn test_query_concentrated_events() {
    let fixture = TimelineFixture::new()
        .with_concentrated_events(100, 0)
        .with_concentrated_events(50, 1);

    let stats = fixture.capsule().query_last_hours(1).unwrap();
    assert_eq!(stats.total_count, 150);
}
```

**Benefits**:
- Common patterns pre-built
- Fluent API (method chaining)
- Fixtures reused across tests
- Reduced code duplication

**UCE34 Analysis**:
- **Q1**: Problem: Timestamp generation duplicated 30+ times
- **Q28**: Simplicity: Fixture library
- **Q31**: Constraints: Fixtures for common patterns

**Acceptance Criteria**:
- [ ] Fixtures for 5+ common patterns
- [ ] Fluent API for composition
- [ ] Used in 50+ tests
- [ ] Zero duplication in test setup

---

### Enhancement 9: Coverage Dashboard

**Current State**: No coverage metrics

**Implementation**: Set up coverage tracking

```bash
#!/bin/bash
# scripts/coverage.sh

cargo tarpaulin --out Html \
                --output-dir coverage \
                --timeout 120 \
                --exclude-files src/bin/* \
                --min-coverage 80

# Generate report
echo "Coverage report: coverage/index.html"

# Fail if coverage < 80%
if [ $? -ne 0 ]; then
    exit 1
fi
```

**Integration**: GitHub Actions

```yaml
# .github/workflows/coverage.yml
name: Coverage

on: [push, pull_request]

jobs:
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo install cargo-tarpaulin
      - run: bash scripts/coverage.sh
      - uses: codecov/codecov-action@v2
        with:
          files: ./coverage.xml
```

**UCE34 Analysis**:
- **Q1**: Problem: Cannot prove 80% coverage
- **Q30**: Validation: Measure coverage
- **Q32**: Constraints: Enforce 80% minimum

**Acceptance Criteria**:
- [ ] Coverage report generated
- [ ] GitHub Actions integration
- [ ] Coverage trend tracked
- [ ] CI fails if <80% coverage

---

### Enhancement 10: Performance Budget Enforcer

**Current State**: Manual benchmark comparison

**Implementation**: CI enforcement

```rust
// tests/performance_budget.rs
#[test]
fn test_append_latency_p99_budget() {
    let capsule = TimelineAggregationCapsuleCore::new(1440, 60).unwrap();
    let mut latencies = Vec::new();

    for _ in 0..100_000 {
        let start = Instant::now();
        capsule.append(1_634_567_890).unwrap();
        latencies.push(start.elapsed().as_nanos() as u64);
    }

    latencies.sort();
    let p99 = latencies[99_000];

    // Budget: 450ns
    assert!(p99 < 450, "p99 latency {} exceeds budget 450ns", p99);
}

#[test]
fn test_query_latency_p99_budget() {
    let capsule = TimelineAggregationCapsuleCore::new(1440, 60).unwrap();
    let mut latencies = Vec::new();

    for _ in 0..10_000 {
        let start = Instant::now();
        capsule.query_bucket(1_634_567_890).ok();
        latencies.push(start.elapsed().as_nanos() as u64);
    }

    latencies.sort();
    let p99 = latencies[9_900];

    // Budget: 520ns
    assert!(p99 < 520, "p99 latency {} exceeds budget 520ns", p99);
}

#[test]
fn test_throughput_minimum() {
    let capsule = TimelineAggregationCapsuleCore::new(1440, 60).unwrap();
    let start = Instant::now();
    let mut ops = 0;

    while start.elapsed() < Duration::from_secs(1) {
        capsule.append(1_634_567_890).ok();
        ops += 1;
    }

    // Minimum: 9M ops/sec (allows 10% regression)
    assert!(ops > 9_000_000, "throughput {} below budget 9M ops/sec", ops);
}
```

**UCE34 Analysis**:
- **Q1**: Problem: No regression detection
- **Q30**: Validation: Enforce budgets
- **Q32**: Constraints: <10% regression allowed

**Acceptance Criteria**:
- [ ] Budget tests for append/query/flush
- [ ] Tests fail if regression >10%
- [ ] Runs in CI/CD on every commit
- [ ] Results tracked over time

---

## Performance Validation

### Enhancement 11: Sustained Throughput Validation

**Status**: Covered in P0_CRITICAL_ENHANCEMENTS.md (Enhancement 19)

---

### Enhancement 12: Tail Latency Outlier Analysis

**Status**: Covered in P0_CRITICAL_ENHANCEMENTS.md (Enhancement 17)

---

### Enhancement 13: Fair Baseline Comparison

**Status**: Covered in P0_CRITICAL_ENHANCEMENTS.md (Enhancement 18)

---

## Developer Convenience

### Enhancement 14: Builder Pattern for Configuration

**Current State**: New() with positional arguments

**Implementation**:

```rust
// src/capsules/timeline_aggregation_capsule.rs
impl TimelineAggregationCapsuleWrapper {
    pub fn builder() -> TimelineBuilder {
        TimelineBuilder::default()
    }
}

pub struct TimelineBuilder {
    num_buckets: usize,
    bucket_duration_secs: u64,
    enable_worker: bool,
    enable_monitoring: bool,
    checkpoint_enabled: bool,
}

impl Default for TimelineBuilder {
    fn default() -> Self {
        Self {
            num_buckets: 1440,           // 24 hours
            bucket_duration_secs: 60,    // 1 minute
            enable_worker: true,
            enable_monitoring: true,
            checkpoint_enabled: true,
        }
    }
}

impl TimelineBuilder {
    pub fn num_buckets(mut self, n: usize) -> Self {
        self.num_buckets = n;
        self
    }

    pub fn bucket_duration_secs(mut self, secs: u64) -> Self {
        self.bucket_duration_secs = secs;
        self
    }

    pub fn build(self) -> Result<TimelineAggregationCapsuleWrapper> {
        // Validation
        if self.num_buckets == 0 {
            return Err(TimelineError::InvalidConfig("num_buckets must be > 0"));
        }
        if self.bucket_duration_secs == 0 {
            return Err(TimelineError::InvalidConfig("bucket_duration_secs must be > 0"));
        }

        TimelineAggregationCapsuleWrapper::new(
            self.num_buckets,
            self.bucket_duration_secs,
        )
    }
}

// Usage:
let timeline = TimelineBuilder::default()
    .num_buckets(2880)  // 48 hours
    .bucket_duration_secs(30)  // 30 seconds
    .build()?;
```

**Benefits**:
- Self-documenting parameters
- Validation at build time
- Extensible for future options
- Clear default configuration

**UCE34 Analysis**:
- **Q1**: Problem: Positional arguments unclear
- **Q28**: Simplicity: Builder pattern
- **Q31**: Constraints: Validation before use

**Acceptance Criteria**:
- [ ] Builder supports all configuration options
- [ ] Validation in build() method
- [ ] Default configuration documented
- [ ] 20+ tests for builder combinations

---

### Enhancement 15: Aggregation Helper Methods

**Current State**: Manual loops for sum/avg/max

**Status**: Partially covered in P0_CRITICAL_ENHANCEMENTS.md (Enhancement 14)

**Expand to include**:

```rust
impl TimelineAggregationCapsuleWrapper {
    // Percentile calculations
    pub fn percentile(&self, start: SystemTime, end: SystemTime, p: u32) -> Result<f64> {
        let range = self.query_range(start, end)?;
        let counts: Vec<u64> = range.buckets.iter().map(|b| b.count).collect();

        // Calculate percentile from histogram
        let sorted: Vec<u64> = counts.iter().cloned().collect();
        let idx = (sorted.len() * p as usize) / 100;
        Ok(sorted[idx] as f64)
    }

    // Rate of change
    pub fn rate_of_change(&self, duration: Duration) -> Result<f64> {
        let now_count = self.aggregate_sum(
            SystemTime::now() - duration,
            SystemTime::now(),
        )?;

        let prev_count = self.aggregate_sum(
            SystemTime::now() - (duration * 2),
            SystemTime::now() - duration,
        )?;

        if prev_count == 0 {
            return Ok(0.0);
        }

        Ok((now_count as f64 - prev_count as f64) / prev_count as f64)
    }

    // Trend analysis
    pub fn trend(&self, hours: u64) -> Result<Trend> {
        let hour_counts: Vec<u64> = (0..hours)
            .map(|h| {
                let end = SystemTime::now() - Duration::from_secs(h * 3600);
                let start = end - Duration::from_secs(3600);
                self.aggregate_sum(start, end).unwrap_or(0)
            })
            .collect();

        // Simple linear regression
        let up = hour_counts.iter().zip(hour_counts.iter().skip(1))
            .filter(|(a, b)| b > a)
            .count();

        let trend = if up > hours as usize / 2 {
            Trend::Rising
        } else if up < hours as usize / 3 {
            Trend::Falling
        } else {
            Trend::Stable
        };

        Ok(trend)
    }
}

#[derive(Debug)]
pub enum Trend {
    Rising,
    Falling,
    Stable,
}
```

**UCE34 Analysis**:
- **Q1**: Problem: Manual loops repeated
- **Q28**: Simplicity: Pre-built helpers
- **Q31**: Constraints: Common operations only

**Acceptance Criteria**:
- [ ] 5+ aggregation helpers
- [ ] Helpers correct for edge cases
- [ ] 30+ tests per helper
- [ ] <10µs latency per helper

---

### Enhancement 16: Composition Patterns Guide

**Current State**: Unclear how to use multi-capsule setups

**Implementation**: Create docs/COMPOSITION_PATTERNS.md

```markdown
# Composition Patterns

## Pattern 1: Per-User Metrics (Multi-Capsule)

Track metrics per user without cross-contamination.

```rust
use std::collections::HashMap;

pub struct UserMetrics {
    timelines: HashMap<UserId, Arc<TimelineAggregationCapsuleWrapper>>,
}

impl UserMetrics {
    pub fn new() -> Self {
        Self {
            timelines: HashMap::new(),
        }
    }

    pub fn append(&mut self, user_id: UserId) -> Result<()> {
        self.timelines
            .entry(user_id)
            .or_insert_with(|| {
                Arc::new(TimelineAggregationCapsuleWrapper::new(1440, 60).unwrap())
            })
            .append_system_time(SystemTime::now())
    }

    pub fn get_user_stats(&self, user_id: UserId, hours: u64) -> Result<TimelineRange> {
        self.timelines
            .get(&user_id)
            .ok_or(TimelineError::UserNotFound)?
            .query_last_hours(hours)
    }
}
```

## Pattern 2: Multi-Tenant Aggregation

Combine metrics from multiple timelines.

```rust
pub struct TenantAggregation {
    timelines: Vec<Arc<TimelineAggregationCapsuleWrapper>>,
}

impl TenantAggregation {
    pub fn aggregate_sum(&self, start: SystemTime, end: SystemTime) -> Result<u64> {
        self.timelines
            .iter()
            .map(|t| t.aggregate_sum(start, end))
            .sum::<Result<u64>>()
    }
}
```

## Pattern 3: Hierarchical Aggregation

Multi-level aggregation (minute → hour → day).

```rust
pub struct HierarchicalTimeline {
    minute_level: TimelineAggregationCapsuleWrapper,  // 1440 buckets @ 60s
    hour_level: TimelineAggregationCapsuleWrapper,    // 168 buckets @ 1h
    day_level: TimelineAggregationCapsuleWrapper,     // 365 buckets @ 1d
}
```
```

**UCE34 Analysis**:
- **Q1**: Problem: Multi-capsule usage unclear
- **I20**: Integration: Composition patterns documented
- **Q28**: Simplicity**: 3+ common patterns

**Acceptance Criteria**:
- [ ] 3+ composition patterns documented
- [ ] All patterns have working examples
- [ ] Performance impact analyzed
- [ ] Trade-offs discussed

---

### Enhancement 17: CLI Command Utilities

**Current State**: No command-line interface

**Implementation**: Create `timeline-cli` tool

```bash
#!/bin/bash
# scripts/timeline-cli

TIMELINE_URL=${TIMELINE_URL:-http://localhost:8000}

case "$1" in
    metrics)
        curl -s "$TIMELINE_URL/timeline/metrics" | jq .
        ;;
    health)
        curl -s "$TIMELINE_URL/timeline/health" | jq .
        ;;
    verify-hash-chain)
        curl -s -X POST "$TIMELINE_URL/timeline/verify-hash-chain" | jq .
        ;;
    *)
        echo "Usage: timeline-cli [metrics|health|verify-hash-chain]"
        exit 1
        ;;
esac
```

**UCE34 Analysis**:
- **Q1**: Problem: Manual curl commands tedious
- **Q28**: Simplicity: Single-purpose CLI
- **Q31**: Constraints: <1 second response time

**Acceptance Criteria**:
- [ ] 5+ CLI commands
- [ ] All commands documented
- [ ] Fast response (<1 sec)
- [ ] Useful for debugging

---

## Error Messaging

### Enhancement 18: Error Classification

**Current State**: Generic "Bucket not active" errors

**Implementation**:

```rust
// src/proxy/error.rs
#[derive(thiserror::Error, Debug)]
pub enum TimelineError {
    // Transient errors (retry)
    #[error("Queue full - retry after backoff")]
    QueueFull,

    #[error("Bucket temporarily locked - retry immediately")]
    BucketLocked,

    // Permanent errors (alert)
    #[error("Bucket {bucket_id} not active - timestamp too old")]
    BucketNotActive { bucket_id: u32 },

    #[error("Worker thread dead - service degraded")]
    WorkerDead,

    // Configuration errors (fix code)
    #[error("Invalid num_buckets: {value} (must be > 0)")]
    InvalidConfig { value: usize },

    // User errors (provide guidance)
    #[error("SystemTime before UNIX_EPOCH: {time:?}\n\
             Hint: Check system clock with 'date'")]
    SystemTimeTooOld { time: SystemTime },
}

impl TimelineError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::QueueFull | Self::BucketLocked)
    }

    pub fn is_permanent(&self) -> bool {
        matches!(self, Self::BucketNotActive { .. } | Self::WorkerDead)
    }

    pub fn is_bug(&self) -> bool {
        matches!(self, Self::InvalidConfig { .. })
    }

    pub fn suggested_action(&self) -> &'static str {
        match self {
            Self::QueueFull => "Wait 100ms then retry",
            Self::WorkerDead => "Restart service: systemctl restart timeline-aggregation",
            Self::SystemTimeTooOld => "Fix system clock: timedatectl set-ntp true",
            _ => "See docs/TROUBLESHOOTING.md",
        }
    }
}
```

**UCE34 Analysis**:
- **Q1**: Problem: Generic error messages
- **Q28**: Simplicity: Error classification
- **Q31**: Constraints: Suggested action included

**Acceptance Criteria**:
- [ ] All errors classified (transient/permanent/bug)
- [ ] Suggested action for each error
- [ ] Error messages <100 characters
- [ ] 50+ test cases for error paths

---

### Enhancement 19-20: Error Documentation + Worker Crash Recovery

**Status**: Covered in P0 (Enhancement 6 + 7)

---

### Enhancement 21: Structured Logging Integration

**Current State**: Errors disappear into void

**Implementation**:

```rust
use tracing::{error, warn, info, debug, span, Level};

impl TimelineAggregationCapsuleWrapper {
    pub fn append_with_logging(&self, time: SystemTime) -> Result<()> {
        let span = span!(Level::DEBUG, "timeline_append", ?time);
        let _enter = span.enter();

        match self.append_system_time(time) {
            Ok(_) => {
                debug!("Append successful");
                Ok(())
            }
            Err(e) => {
                error!(
                    error = ?e,
                    error_type = std::any::type_name_of_val(&e),
                    "Append failed"
                );
                Err(e)
            }
        }
    }
}

// Structured logs output:
// {"timestamp":"2025-10-21T14:30:45Z","level":"ERROR",
//  "message":"Append failed","error":"Bucket not active","bucket_id":123}
```

**UCE34 Analysis**:
- **Q1**: Problem: Silent failures
- **Q11**: Rust: Use `tracing` crate
- **Q28**: Simplicity: Structured logging

**Acceptance Criteria**:
- [ ] All error paths logged
- [ ] Structured fields for debugging
- [ ] Log levels appropriate (DEBUG/WARN/ERROR)
- [ ] Logs parseable by aggregators (ELK, Datadog)

---

## Integration

### Enhancement 22: Per-User Metrics Aggregation

**Status**: Covered in Enhancement 15 (Composition patterns)

---

### Enhancement 23: Dashboard Integration Guide

**Status**: Covered in P1 Enhancement 6 (Integration Guide)

---

### Enhancement 24: Multi-Tenant Support

**Current State**: Single timeline per service

**Implementation**:

```rust
pub struct MultiTenantTimeline {
    timelines: DashMap<TenantId, Arc<TimelineAggregationCapsuleWrapper>>,
    default_config: TimelineConfig,
}

impl MultiTenantTimeline {
    pub fn append(&self, tenant_id: TenantId, time: SystemTime) -> Result<()> {
        let timeline = self.timelines
            .entry(tenant_id)
            .or_insert_with(|| {
                Arc::new(
                    TimelineAggregationCapsuleWrapper::builder()
                        .num_buckets(self.default_config.num_buckets)
                        .bucket_duration_secs(self.default_config.bucket_duration_secs)
                        .build()
                        .unwrap()
                )
            });

        timeline.append_system_time(time)
    }

    pub fn query_tenant(&self, tenant_id: TenantId, hours: u64) -> Result<TimelineRange> {
        self.timelines
            .get(&tenant_id)
            .ok_or(TimelineError::TenantNotFound)?
            .query_last_hours(hours)
    }
}
```

**UCE34 Analysis**:
- **Q1**: Problem: Single timeline limitation
- **I20**: Integration: Multi-tenancy
- **Q10**: Tier: T4 (DashMap for tenant registry)

**Acceptance Criteria**:
- [ ] Supports 1000+ tenants
- [ ] Per-tenant isolation (no data leakage)
- [ ] <100µs append overhead for tenant lookup
- [ ] 50+ tests for multi-tenant scenarios

---

## Summary

**Total P1 Enhancements**: 24 issues organized by category

**Priority Order for Implementation**:

1. **Documentation** (E2-E6) - Enable self-service (1 week)
2. **Testing** (E7-E10) - Improve developer productivity (1 week)
3. **Developer Convenience** (E14-E17) - Improve DX (1 week)
4. **Error Messaging** (E18-E21) - Support team enablement (3 days)
5. **Integration** (E22-E24) - Advanced features (1 week)

**Estimated Total Effort**: 4-5 weeks for all P1 enhancements

---

**Next Step**: Review P2_MEDIUM_PRIORITY_ENHANCEMENTS.md for additional improvements
