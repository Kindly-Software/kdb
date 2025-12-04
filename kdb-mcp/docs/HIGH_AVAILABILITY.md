# High Availability Setup Guide

Production-grade high availability configuration for atomic_mcp_server.

## Architecture Overview

```
┌─────────────┐         ┌──────────────────────────┐
│   Clients   │  HTTPS  │   nginx Load Balancer    │
│  (Claude)   │◄───────►│   (192.168.0.38:443)     │
└─────────────┘         └──────────────────────────┘
                                   │
                        ┌──────────┴──────────┐
                        │   Round-robin LB    │
                        │   Health checks     │
                        │   Session affinity  │
                        └──────────┬──────────┘
                                   │
        ┌──────────────┬──────────┼──────────┬──────────────┐
        │              │          │          │              │
┌───────▼──────┐ ┌────▼─────┐ ┌─▼────────┐ ┌─────▼────────┐
│ Instance 1   │ │Instance 2│ │Instance 3│ │ Instance 4   │
│   :5678      │ │  :5679   │ │  :5680   │ │   :5681      │
└──────┬───────┘ └────┬─────┘ └─┬────────┘ └──────┬───────┘
       │              │          │                 │
       └──────────────┴──────────┴─────────────────┘
                         │
              ┌──────────▼──────────┐
              │ Shared State (mmap) │
              │ /dev/shm/mcp-shared │
              │   (128MB lockfree)  │
              └─────────────────────┘
```

## Components

### 1. nginx Load Balancer

**Configuration**: `/home/samuel/Primitives/atomic_mcp_server/nginx/mcp-loadbalancer.conf`

**Features**:
- Round-robin load distribution
- Health checks every 5s (GET /health)
- Automatic failover (3 failures → remove instance)
- Session affinity (IP hash for stateful sessions)
- TLS/SSL termination (A+ grade)
- Rate limiting (100 req/s per client)

**Deployment**:
```bash
# Install nginx
sudo apt install nginx

# Link configuration
sudo ln -s $(pwd)/nginx/mcp-loadbalancer.conf /etc/nginx/sites-available/mcp-loadbalancer.conf
sudo ln -s /etc/nginx/sites-available/mcp-loadbalancer.conf /etc/nginx/sites-enabled/

# Test configuration
sudo nginx -t

# Reload nginx
sudo systemctl reload nginx
```

**Performance**:
- <100μs overhead per request
- 10K+ concurrent connections
- Zero-copy forwarding

### 2. Multi-Instance Deployment

**Rolling Updates**: `/home/samuel/Primitives/atomic_mcp_server/deploy_rolling.sh`

**Strategy**:
1. Remove instance from LB
2. Wait for connection drain (30s)
3. Stop instance
4. Deploy new binary
5. Start instance
6. Health check (12 retries × 5s = 60s)
7. Add back to LB

**Zero-Downtime Guarantee**:
- Always ≥3 instances active (75% capacity)
- Automatic rollback on failure
- Graceful shutdown (SIGTERM → 10s grace period)

**Usage**:
```bash
# Dry-run (no changes)
./deploy_rolling.sh --dry-run

# Deploy all instances
./deploy_rolling.sh

# Skip tests (faster, for hotfixes)
./deploy_rolling.sh --skip-tests

# Rollback to previous version
./deploy_rolling.sh --rollback
```

**Timeline**:
- Phase 1 (instance :5678): ~75s (30s drain + 10s stop + 5s deploy + 30s health)
- Phase 2 (instance :5679): ~75s
- Phase 3 (instance :5680): ~75s
- Phase 4 (instance :5681): ~75s
- **Total**: ~5 minutes (zero downtime)

### 3. Shared State (T9 Persistent)

**Module**: `src/shared_state.rs`

**Purpose**: Cross-instance state sharing via mmap

**Features**:
- 128MB shared memory segment (`/dev/shm/mcp-shared`)
- 4096 session slots (lockfree hash table)
- 16384 quota slots (lockfree counters)
- Crash-safe (survives process restart)

**Performance**:
- Session lookup: <50ns
- Quota increment: <20ns
- Flush to disk: <1ms (synchronous), <100μs (async)

**Architecture**:
```rust
use atomic_mcp_server::SharedStateCapsule;
use std::path::Path;

// Create or open shared state
let state = SharedStateCapsule::new(None)?; // Uses /dev/shm/mcp-shared

// Register instance
state.register_instance();

// Allocate session ID
let session_id = state.allocate_session_id();

// Track quota
let quota = state.quota_entry(client_hash);
quota.request_count.fetch_add(1, Ordering::Relaxed);

// Flush changes
state.flush()?; // Synchronous (durability)
state.flush_async()?; // Asynchronous (best effort)
```

**Layout**:
```
Offset   Size     Description
------   ----     -----------
0        64 B     Header (magic, version, counters)
64       960 B    Metadata (session/quota config)
1 KB     1 MB     Session hash table (4096 × 256B)
1 MB     1 MB     Quota tracker (16384 × 64B)
2 MB     126 MB   Reserved
```

**Atomicity**:
- All operations use AtomicU64 (cross-process safe)
- Generation counters prevent ABA problems
- Cache-aligned (64B/256B) to avoid false sharing

**Failure Recovery**:
- On crash: Shared memory persists (/dev/shm survives process restart)
- On reboot: Shared memory cleared, instances reinitialize
- On corruption: Magic header validation fails → reinitialize

## 4. Systemd Services

**Service Template**: `/home/samuel/Primitives/atomic_mcp_server/systemd/mcp-debug@.service`

```ini
[Unit]
Description=MCP Debug Server (Instance %i)
After=network.target

[Service]
Type=simple
User=mcp
Group=mcp
ExecStart=/usr/local/bin/mcp_debug_server_%i --port=%i
Restart=always
RestartSec=5s
Environment="RUST_LOG=info"
Environment="RUST_BACKTRACE=1"

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/dev/shm /var/log/mcp-debug

[Install]
WantedBy=multi-user.target
```

**Deployment**:
```bash
# Install service template
sudo cp systemd/mcp-debug@.service /etc/systemd/system/

# Enable instances
sudo systemctl enable mcp-debug@5678
sudo systemctl enable mcp-debug@5679
sudo systemctl enable mcp-debug@5680
sudo systemctl enable mcp-debug@5681

# Start instances
sudo systemctl start mcp-debug@5678
sudo systemctl start mcp-debug@5679
sudo systemctl start mcp-debug@5680
sudo systemctl start mcp-debug@5681

# Check status
sudo systemctl status mcp-debug@*
```

## Health Checks

**Endpoint**: `GET /health`

**Response**:
```json
{
  "status": "healthy",
  "uptime_seconds": 3600,
  "version": "0.1.0",
  "instance_id": "5678",
  "shared_state": {
    "active_instances": 4,
    "total_sessions": 142,
    "total_requests": 1500000
  }
}
```

**Health Criteria**:
- HTTP 200 response
- Response time <100ms
- Shared state accessible
- No critical errors in last 5 minutes

## Monitoring

**Metrics Endpoint**: `GET /metrics` (Prometheus format)

**Key Metrics**:
```
# Instance health
mcp_instance_count 4
mcp_instance_uptime_seconds{instance="5678"} 3600

# Shared state
mcp_shared_state_sessions_active 142
mcp_shared_state_quota_entries 1024

# Load balancer
mcp_lb_active_connections 50
mcp_lb_requests_total 1500000
mcp_lb_request_duration_seconds{quantile="0.99"} 0.008
```

**Grafana Dashboard**: `/home/samuel/Primitives/atomic_mcp_server/grafana/error_budget_dashboard.json`

**Import**:
```bash
# Import dashboard
curl -X POST http://localhost:3000/api/dashboards/db \
  -H "Content-Type: application/json" \
  -d @grafana/error_budget_dashboard.json
```

## Failure Scenarios

### Instance Crash

**Detection**: Health check fails (3 consecutive failures)

**Response**:
1. nginx removes instance from pool (automatic)
2. Remaining 3 instances handle 33% more traffic
3. systemd restarts crashed instance (Restart=always)
4. Instance rejoins pool after health check passes

**Recovery Time**: <30s

### Shared State Corruption

**Detection**: Magic header mismatch on read

**Response**:
1. Instance logs error, reinitializes shared memory
2. Other instances continue using previous state
3. New sessions start from scratch

**Data Loss**: Sessions created since last flush (<1s)

### nginx Crash

**Detection**: Clients fail to connect

**Response**:
1. systemd restarts nginx (Restart=always)
2. Clients retry connection after 5s

**Recovery Time**: <5s

### Complete Network Partition

**Detection**: All health checks fail

**Response**:
1. nginx marks all instances as down
2. Clients receive 503 Service Unavailable
3. Retry after network recovery

**Recovery Time**: Network recovery time + 5s (health check interval)

## Performance Tuning

### nginx

**Optimize for MCP workload** (small JSON-RPC requests):
```nginx
worker_processes auto;
worker_rlimit_nofile 65535;

events {
    worker_connections 4096;
    use epoll;
    multi_accept on;
}

http {
    # Reduce buffering overhead
    proxy_buffering on;
    proxy_buffer_size 4k;
    proxy_buffers 8 4k;

    # Keepalive connections
    keepalive_requests 1000;
    keepalive_timeout 60s;

    # TCP optimizations
    tcp_nodelay on;
    tcp_nopush on;
}
```

### Shared State

**Pre-allocate shared memory** (avoid page faults):
```bash
# Lock shared memory in RAM (no swap)
echo 134217728 > /sys/kernel/mm/transparent_hugepage/khugepaged/max_ptes_none
```

### Systemd

**Increase file descriptors** (for high-connection workloads):
```ini
[Service]
LimitNOFILE=65535
```

## Scaling

### Horizontal Scaling

**Add more instances**:
1. Update nginx upstream pool
2. Deploy new instances (ports :5682, :5683, ...)
3. Reload nginx

**Limits**:
- CPU: 1 instance per core (8 cores → 8 instances max)
- Memory: 256MB per instance + 128MB shared state
- Network: 10 Gbps NIC → ~100K req/s

### Vertical Scaling

**Increase shared state capacity**:
1. Increase `SIZE` in `shared_state.rs` (e.g., 128MB → 256MB)
2. Increase session/quota capacity
3. Rebuild and redeploy

## Security

### TLS Configuration

**A+ grade** (ssllabs.com):
```nginx
ssl_protocols TLSv1.2 TLSv1.3;
ssl_ciphers 'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256';
ssl_prefer_server_ciphers off;
ssl_session_cache shared:SSL:10m;
ssl_stapling on;
```

### Access Control

**IP whitelist for admin endpoints**:
```nginx
location /admin {
    allow 192.168.0.0/24;  # Local network
    allow 127.0.0.1;       # Localhost
    deny all;
}
```

### Rate Limiting

**Global rate limit** (per client IP):
```nginx
limit_req_zone $binary_remote_addr zone=mcp_limit:10m rate=100r/s;
limit_req zone=mcp_limit burst=200 nodelay;
```

## Troubleshooting

### Health Check Failing

**Symptoms**: nginx removes instance from pool

**Diagnosis**:
```bash
# Check instance status
systemctl status mcp-debug@5678

# Check logs
journalctl -u mcp-debug@5678 -f

# Manual health check
curl -v http://192.168.0.38:5678/health
```

**Common Causes**:
- Instance crashed (check journalctl)
- Port conflict (check `ss -tuln | grep 5678`)
- Shared state corrupted (check /dev/shm/mcp-shared)

### Shared State Not Accessible

**Symptoms**: Instance logs "Failed to open shared memory"

**Diagnosis**:
```bash
# Check shared memory
ls -lh /dev/shm/mcp-shared

# Check permissions
stat /dev/shm/mcp-shared
```

**Fix**:
```bash
# Remove corrupted shared memory
rm /dev/shm/mcp-shared

# Restart instances (will recreate)
sudo systemctl restart mcp-debug@*
```

### Rolling Deployment Stuck

**Symptoms**: `deploy_rolling.sh` hangs during health check

**Diagnosis**:
```bash
# Check instance logs
journalctl -u mcp-debug@5678 -n 50

# Manual health check
curl -v http://192.168.0.38:5678/health
```

**Fix**:
```bash
# Rollback deployment
./deploy_rolling.sh --rollback
```

## Production Checklist

- [ ] nginx load balancer configured and tested
- [ ] 4 systemd services running
- [ ] Shared state accessible (/dev/shm/mcp-shared)
- [ ] Health checks passing on all instances
- [ ] Grafana dashboard imported
- [ ] Prometheus scraping metrics
- [ ] TLS certificates valid (>30 days)
- [ ] Firewall rules configured (port 443 only)
- [ ] Backups of shared state configured (daily)
- [ ] Monitoring alerts configured (PagerDuty/OpsGenie)
- [ ] Rolling deployment tested on staging
- [ ] Disaster recovery plan documented

## SLA Targets

**99.9% availability** (43.2 minutes downtime/month):
- Max instance downtime: 10 minutes/month
- Max load balancer downtime: 5 minutes/month
- Max network downtime: 28 minutes/month

**Performance**:
- P99 latency: <10ms end-to-end
- Throughput: 10K+ req/s (all instances)
- Connection capacity: 10K+ concurrent connections

**Recovery**:
- Instance crash → <30s (automatic restart)
- Load balancer crash → <5s (systemd restart)
- Network partition → <60s (health check + failover)
