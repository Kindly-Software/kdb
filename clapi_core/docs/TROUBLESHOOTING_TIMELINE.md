# Troubleshooting Guide - Timeline Aggregation

Common errors, solutions, and debugging strategies for Timeline Aggregation capsules.

---

## Table of Contents

1. [Common Errors](#common-errors)
2. [Performance Issues](#performance-issues)
3. [Deployment Problems](#deployment-problems)
4. [Monitoring Issues](#monitoring-issues)
5. [Data Integrity](#data-integrity)
6. [FAQ](#faq)

---

## Common Errors

### Error: "Bucket not active"

**Symptoms**:
```rust
Err(IoError("Bucket not active"))
```

**Cause**: Attempting to append to bucket that is:
- Outside timeline range (timestamp too old or too new)
- Already marked Complete or Flushed

**Diagnosis**:

```bash
# Check timestamp is within range
echo "Start timestamp: $START_TS"
echo "Current timestamp: $(date +%s)"
echo "Timeline duration: $((CAPACITY * BUCKET_DURATION))"
echo "End timestamp: $((START_TS + CAPACITY * BUCKET_DURATION))"
```

**Solutions**:

1. **Timestamp out of range** - Adjust start_ts or capacity:

```rust
// ❌ Wrong: Timestamp before start
let timeline = TimelineAggregationCapsule::new(1000, BucketGranularity::Minute, 60);
timeline.append(500)?;  // Error! 500 < 1000

// ✅ Right: Use current time as start
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
let timeline = TimelineAggregationCapsule::new(
    now,
    BucketGranularity::Minute,
    60,
);
timeline.append(now)?;  // OK
```

2. **Bucket already flushed** - Don't append after flush:

```rust
// ❌ Wrong: Append after flush
timeline.flush_bucket(0)?;
timeline.append(1000)?;  // Error! Bucket 0 flushed

// ✅ Right: Append before flush
timeline.append(1000)?;
timeline.flush_bucket(0)?;  // OK
```

3. **Circular buffer overflow** - Increase capacity:

```rust
// ❌ Wrong: Capacity too small (only 10 minutes)
let timeline = TimelineAggregationCapsule::new(
    now,
    BucketGranularity::Minute,
    10,  // Only tracks 10 minutes
);

// ✅ Right: Sufficient capacity (24 hours)
let timeline = TimelineAggregationCapsule::new(
    now,
    BucketGranularity::Minute,
    1440,  // Tracks 24 hours
);
```

---

### Error: "Channel closed" (TimelineBridge)

**Symptoms**:
```rust
Err(IoError("Channel closed"))
```

**Cause**: Background worker thread crashed or shut down

**Diagnosis**:

```bash
# Check worker thread status
ps aux | grep timeline-aggregation

# Check logs for worker crash
journalctl -u timeline-aggregation | grep -i "worker"

# Check error counter
curl http://localhost:8000/timeline/metrics | jq '.error_count'
```

**Solutions**:

1. **Worker thread panic** - Check logs for panic:

```bash
# Look for panic in logs
journalctl -u timeline-aggregation -n 100 | grep -i "panic"

# Common causes:
# - Memory exhaustion (OOM killer)
# - Stack overflow (infinite recursion)
# - Assertion failure (bug in code)
```

2. **Restart service**:

```bash
sudo systemctl restart timeline-aggregation
```

3. **Increase memory limit** (if OOM):

```bash
# Edit systemd service
sudo systemctl edit timeline-aggregation

# Add memory limit:
[Service]
MemoryLimit=8G
MemoryMax=10G

sudo systemctl daemon-reload
sudo systemctl restart timeline-aggregation
```

4. **Enable debug logging**:

```bash
# Set environment variable
export RUST_LOG=debug

# Or in systemd service:
[Service]
Environment="RUST_LOG=debug"
```

---

### Error: "SystemTime before UNIX_EPOCH"

**Symptoms**:
```rust
Err(IoError("SystemTime before UNIX_EPOCH"))
```

**Cause**: System clock set to time before Jan 1, 1970 UTC (clock skew)

**Diagnosis**:

```bash
# Check current time
date

# Check NTP status
timedatectl

# Check clock skew
timedatectl status | grep "RTC time"
```

**Solutions**:

1. **Enable NTP** (recommended):

```bash
sudo timedatectl set-ntp true
sudo systemctl restart systemd-timesyncd

# Verify NTP active
timedatectl | grep "NTP"
```

2. **Manually set time**:

```bash
# Set time (example: Oct 22, 2025, 14:30:00 UTC)
sudo timedatectl set-time "2025-10-22 14:30:00"
```

3. **Check hardware clock**:

```bash
# Sync hardware clock to system clock
sudo hwclock --systohc

# Or sync system clock to hardware clock
sudo hwclock --hctosys
```

4. **Monitor clock skew** (prevention):

```bash
# Add monitoring alert
# Alert if clock skew > 10 seconds
curl http://localhost:9090/api/v1/query -d 'query=abs(time() - timestamp(node_time_seconds)) > 10'
```

---

### Error: Index out of bounds

**Symptoms**:
```rust
Err(IoError("Index out of bounds"))
```

**Cause**: Querying bucket index >= capacity

**Diagnosis**:

```rust
let capacity = timeline.capacity();
println!("Timeline capacity: {}", capacity);
println!("Attempted index: {}", attempted_index);
```

**Solutions**:

```rust
// ❌ Wrong: Query beyond capacity
let timeline = TimelineAggregationCapsule::new(1000, BucketGranularity::Minute, 60);
timeline.query_bucket(100)?;  // Error! 100 >= 60

// ✅ Right: Check capacity first
let capacity = timeline.capacity();
if bucket_index < capacity {
    timeline.query_bucket(bucket_index)?;
} else {
    eprintln!("Bucket index {} out of range [0, {})", bucket_index, capacity);
}
```

---

## Performance Issues

### Issue: High Append Latency (>1µs)

**Symptoms**: p99 latency > 1µs (normal: <100ns)

**Diagnosis**:

```bash
# 1. Check CPU usage
top -b -n 1 | grep -E "Cpu|MEM"

# 2. Check memory pressure
free -h
cat /proc/pressure/memory

# 3. Check for CPU throttling
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq

# 4. Check context switches
vmstat 1 5 | grep -E "cs|in"
```

**Solutions** (in order of likelihood):

1. **CPU contention** - Reduce other workloads:

```bash
# Find CPU-intensive processes
ps aux --sort=-%cpu | head -20

# Reduce priority of other processes
sudo renice +10 -p <PID>
```

2. **Memory pressure** - Increase available RAM:

```bash
# Check memory usage
free -h

# Clear page cache (temporary)
sudo sync && echo 3 > /proc/sys/vm/drop_caches

# Add swap (if none)
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
```

3. **CPU frequency scaling** - Set performance governor:

```bash
# Check current governor
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor

# Set performance governor (all CPUs)
for cpu in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
    echo "performance" | sudo tee $cpu
done
```

4. **Allocator overhead** - Use jemalloc:

```toml
# Cargo.toml
[dependencies]
jemallocator = "0.5"

# main.rs
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;
```

---

### Issue: Memory Leak

**Symptoms**: Memory grows unbounded over hours/days

**Diagnosis**:

```bash
# 1. Watch memory over time
watch -n 5 'ps aux | grep timeline | grep -v grep'

# 2. Check resident memory
pmap -x $(pgrep timeline)

# 3. Check heap profiling (if available)
MALLOC_CONF="prof_leak:true,lg_prof_sample:0,prof_final:true" ./timeline-aggregation

# 4. Check for circular buffer overflow
curl http://localhost:8000/timeline/metrics | jq '.capacity, .head'
```

**Solutions**:

1. **Unclosed TimelineBridge** - Ensure proper shutdown:

```rust
// ❌ Wrong: Bridge never closed
{
    let bridge = TimelineBridge::new(...);
    bridge.append_event(ts).await?;
    // Bridge dropped without shutdown
}

// ✅ Right: Explicit shutdown
{
    let bridge = TimelineBridge::new(...);
    bridge.append_event(ts).await?;
    bridge.shutdown().await?;  // Graceful shutdown
}
```

2. **Arc cycle** - Check for reference cycles:

```rust
// ❌ Wrong: Arc cycle
struct Tracker {
    bridge: Arc<TimelineBridge>,
    self_ref: Option<Arc<Tracker>>,  // Cycle!
}

// ✅ Right: No cycle
struct Tracker {
    bridge: Arc<TimelineBridge>,
    // No self-reference
}
```

3. **Monitor memory** - Set up alerting:

```bash
# Prometheus alert rule
- alert: TimelineMemoryLeak
  expr: process_resident_memory_bytes{job="timeline"} > 1e9  # 1GB
  for: 1h
  annotations:
    summary: "Timeline memory leak detected"
```

---

### Issue: Worker Thread Lag (TimelineBridge)

**Symptoms**: Events appended but `total_events()` returns stale value

**Diagnosis**:

```rust
// Check worker lag
let appended = 1000;
bridge.append_event(ts).await?;  // Append 1000 events

tokio::time::sleep(Duration::from_millis(50)).await;
let processed = bridge.total_events();

if processed < appended {
    println!("⚠️ Worker lag: {} events pending", appended - processed);
}
```

**Solutions**:

1. **Wait for batch flush** - Allow 200ms for processing:

```rust
// ❌ Wrong: Query immediately
bridge.append_event(ts).await?;
assert_eq!(bridge.total_events(), 1);  // Fails! Worker hasn't flushed

// ✅ Right: Wait for flush
bridge.append_event(ts).await?;
tokio::time::sleep(Duration::from_millis(200)).await;
assert_eq!(bridge.total_events(), 1);  // OK
```

2. **Increase channel capacity** - If many concurrent appenders:

```rust
// In TimelineBridge::new() (requires code change):
let (sender_tx, receiver_rx) = mpsc::channel::<TimelineEvent>(10_000);  // Was 1024
```

3. **Monitor worker health**:

```bash
# Check worker alive
curl http://localhost:8000/timeline/health | jq '.worker_alive'

# Check error count
curl http://localhost:8000/timeline/metrics | jq '.error_count'
```

---

## Deployment Problems

### Problem: Service won't start after deploy

**Symptoms**: `systemctl status timeline-aggregation` shows "dead" or "failed"

**Diagnosis**:

```bash
# 1. Check logs
sudo journalctl -u timeline-aggregation -n 50

# 2. Check binary
./target/release/timeline-aggregation --version

# 3. Check dependencies
ldd /usr/local/bin/timeline-aggregation

# 4. Check permissions
ls -la /usr/local/bin/timeline-aggregation
ls -la /var/lib/timeline/  # Data directory
```

**Solutions**:

1. **Permission denied**:

```bash
# Fix binary permissions
sudo chown root:root /usr/local/bin/timeline-aggregation
sudo chmod 755 /usr/local/bin/timeline-aggregation

# Fix data directory
sudo chown timeline:timeline /var/lib/timeline
sudo chmod 755 /var/lib/timeline
```

2. **Missing shared library**:

```bash
# Check missing libraries
ldd /usr/local/bin/timeline-aggregation | grep "not found"

# Install missing dependencies (example: libssl)
sudo apt-get install libssl-dev
```

3. **Port already in use**:

```bash
# Check port 8000 in use
sudo netstat -tulpn | grep :8000

# Kill process or change port in config
sudo kill -9 <PID>
# Or edit clapi.toml:
# [server]
# listen_addr = "0.0.0.0:8001"
```

4. **Rollback** (if new deploy broken):

```bash
# Automatic rollback script
./scripts/rollback.sh

# Or manual:
sudo systemctl stop timeline-aggregation
sudo cp /usr/local/bin/timeline-aggregation.backup /usr/local/bin/timeline-aggregation
sudo systemctl start timeline-aggregation
```

---

## Monitoring Issues

### Problem: No metrics in Grafana

**Symptoms**: Grafana dashboard shows "No data"

**Diagnosis**:

```bash
# 1. Check metrics endpoint
curl http://localhost:8000/timeline/metrics

# 2. Check Prometheus scrape
curl http://localhost:9090/api/v1/targets | jq '.data.activeTargets[] | select(.job=="timeline")'

# 3. Check Prometheus logs
sudo journalctl -u prometheus -n 50 | grep timeline

# 4. Check network connectivity
telnet localhost 8000
```

**Solutions**:

1. **Service not running**:

```bash
sudo systemctl status timeline-aggregation
sudo systemctl start timeline-aggregation
```

2. **Prometheus not scraping** - Fix scrape config:

```yaml
# /etc/prometheus/prometheus.yml
scrape_configs:
  - job_name: 'timeline'
    static_configs:
      - targets: ['localhost:8000']
    metrics_path: '/timeline/metrics/prometheus'
    scrape_interval: 15s
```

```bash
# Reload Prometheus
sudo systemctl reload prometheus
# Or
curl -X POST http://localhost:9090/-/reload
```

3. **Firewall blocking** - Allow port 8000:

```bash
# Check firewall
sudo ufw status

# Allow port 8000
sudo ufw allow 8000/tcp
```

4. **Grafana datasource** - Configure Prometheus:

```bash
# Check datasource
curl -H "Authorization: Bearer $GRAFANA_TOKEN" http://localhost:3000/api/datasources

# Add datasource (if missing)
curl -X POST http://localhost:3000/api/datasources \
  -H "Authorization: Bearer $GRAFANA_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Prometheus",
    "type": "prometheus",
    "url": "http://localhost:9090",
    "access": "proxy"
  }'
```

---

## Data Integrity

### Problem: Hash chain validation failed

**Symptoms**: Hash chain integrity check reports corruption

**Severity**: **CRITICAL** - Possible tampering or bug

**Diagnosis**:

```bash
# 1. Save checkpoint immediately
cp ~/.timeline_checkpoint ~/.timeline_checkpoint.backup.$(date +%s)

# 2. Check logs for corruption
journalctl -u timeline-aggregation | grep -i "hash chain"

# 3. Verify which bucket corrupted
curl http://localhost:8000/timeline/verify-hash-chain | jq '.corrupted_bucket'
```

**Solutions**:

1. **Immediate response**:

```bash
# 1. Stop service (prevent further writes)
sudo systemctl stop timeline-aggregation

# 2. Notify security team
echo "Hash chain corruption detected on $(hostname) at $(date)" | \
  mail -s "CRITICAL: Timeline hash chain corruption" security@example.com

# 3. Preserve evidence
sudo tar czf /tmp/timeline-evidence-$(date +%s).tar.gz \
  /var/lib/timeline/ \
  /var/log/timeline/ \
  ~/.timeline_checkpoint
```

2. **Investigate cause**:

```bash
# Check for hardware errors
dmesg | grep -i error

# Check for disk corruption
sudo smartctl -a /dev/sda

# Check for memory errors
sudo dmidecode -t memory | grep -i error
```

3. **Recovery**:

```bash
# Option 1: Restore from backup
sudo systemctl stop timeline-aggregation
sudo cp ~/.timeline_checkpoint.backup ~/.timeline_checkpoint
sudo systemctl start timeline-aggregation

# Option 2: Rebuild timeline from audit log
# (Requires Phase 6 persistence implementation)
```

4. **Prevention**:

```bash
# Enable background hash chain validation
# Edit clapi.toml:
[timeline]
hash_chain_validation = true
validation_interval_secs = 3600  # Every hour
```

---

## FAQ

### Q: Can I use old timestamps?

**A**: Yes, within timeline range:

```rust
// Timeline range: [start_ts, start_ts + (capacity × bucket_duration))
let start_ts = 1000;
let capacity = 1440;  // 24 hours
let duration = 60;    // Minute buckets

// Valid range: [1000, 1000 + (1440 × 60)) = [1000, 87400)
timeline.append(1000)?;    // OK (first bucket)
timeline.append(50000)?;   // OK (within range)
timeline.append(87399)?;   // OK (last valid timestamp)
timeline.append(87400)?;   // Error! Out of range
```

### Q: What happens if I append faster than 10M ops/sec?

**A**: Throughput depends on number of threads:

- **Single thread**: ~10M ops/sec (100ns per append)
- **8 threads**: ~60M ops/sec (linear scaling, lockfree)
- **Bottleneck**: Memory bandwidth (atomic operations)

If you exceed throughput, use TimelineBridge with batching:

```rust
// TimelineBridge batches 100 events per 100ms
// Effective throughput: 1M events/sec per bridge
// Use multiple bridges for higher throughput
```

### Q: Can I persist data to disk?

**A**: Not yet in v0.4.9. Phase 6 adds:
- Checkpoint mechanism (periodic saves)
- Restore from checkpoint on restart
- Audit log replay

Workaround: Periodically export snapshots:

```rust
// Export snapshot to JSON
let snapshots = bridge.query_range(start, end).await?;
let json = serde_json::to_string(&snapshots)?;
std::fs::write("timeline_snapshot.json", json)?;
```

### Q: How do I scale to multiple machines?

**A**: Each machine runs independent timeline:

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  Machine 1  │    │  Machine 2  │    │  Machine 3  │
│  Timeline   │    │  Timeline   │    │  Timeline   │
└──────┬──────┘    └──────┬──────┘    └──────┬──────┘
       │                  │                  │
       └──────────────────┴──────────────────┘
                          │
                  ┌───────▼────────┐
                  │   Aggregator   │
                  │  (Combines)    │
                  └────────────────┘
```

Aggregation layer combines timelines:

```rust
// Aggregate across machines
async fn aggregate_timelines(machines: Vec<String>) -> u64 {
    let mut total = 0;
    for machine in machines {
        let url = format!("http://{}/timeline/metrics", machine);
        let resp: serde_json::Value = reqwest::get(&url).await?.json().await?;
        total += resp["total_events"].as_u64().unwrap();
    }
    total
}
```

### Q: What's the difference between TimelineAggregationCapsule and TimelineBridge?

**A**:

| Feature | TimelineAggregationCapsule | TimelineBridge |
|---------|---------------------------|----------------|
| **API** | Synchronous | Asynchronous (tokio) |
| **Threading** | Caller's thread | Background worker thread |
| **Batching** | No batching | Batches 100 events or 100ms |
| **Error handling** | Immediate | Exponential backoff retry |
| **Use case** | Sync apps, benchmarks | Async apps, production |
| **Performance** | <100ns direct append | <100ns channel send + batch overhead |

**Recommendation**: Use TimelineBridge for production async apps.

---

## Still Having Issues?

### Diagnostics Script

Run comprehensive diagnostics:

```bash
#!/bin/bash
# scripts/timeline-diagnostics.sh

echo "=== Timeline Diagnostics ==="
echo ""

echo "1. Service Status:"
systemctl status timeline-aggregation --no-pager

echo ""
echo "2. Metrics:"
curl -s http://localhost:8000/timeline/metrics | jq '.'

echo ""
echo "3. Health:"
curl -s http://localhost:8000/timeline/health | jq '.'

echo ""
echo "4. System Resources:"
echo "CPU: $(top -bn1 | grep "Cpu(s)" | awk '{print $2}')"
echo "Memory: $(free -h | grep Mem | awk '{print $3 "/" $2}')"

echo ""
echo "5. Recent Logs:"
journalctl -u timeline-aggregation -n 20 --no-pager

echo ""
echo "6. Network:"
netstat -tulpn | grep :8000
```

### Get Help

1. **GitHub Issues**: https://github.com/kindly/clapi_core/issues
2. **Discussions**: https://github.com/kindly/clapi_core/discussions
3. **Documentation**: See `docs/` directory
4. **Examples**: See `examples/timeline_aggregation_demo.rs`

When reporting issues, include:
- Timeline configuration (capacity, granularity)
- Error message (full stack trace)
- Diagnostics output (see script above)
- Rust version (`rustc --version`)
- OS version (`uname -a`)

---

**Next**: See [EXAMPLES_TIMELINE.md](EXAMPLES_TIMELINE.md) for complete working examples
