# KINDLY DEDUP - Production Deployment Guide

**Version**: v3.1.0
**Target Audience**: DevOps, SRE, System Administrators
**Last Updated**: 2025-11-26

---

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Installation](#installation)
3. [Bare Metal Deployment](#bare-metal-deployment)
4. [Docker Deployment](#docker-deployment)
5. [Production Hardening](#production-hardening)
6. [Performance Tuning](#performance-tuning)
7. [Monitoring & Observability](#monitoring--observability)
8. [Troubleshooting](#troubleshooting)

---

## System Requirements

### Hardware Requirements by Tier

| Tier | Documents | RAM | CPU Cores | Storage | Throughput | Use Case |
|------|-----------|-----|-----------|---------|------------|----------|
| **Tier 1** | 100K | 2 GB | 2 cores | 10 GB | 60K docs/sec | Development, Small Datasets |
| **Tier 2** | 1M | 4 GB | 4 cores | 50 GB | 60K docs/sec | Medium Datasets |
| **Tier 3** | 10M | 8 GB | 8 cores | 200 GB | 60K docs/sec | Large Datasets |
| **Tier 4** | 100M+ | 16 GB | 16 cores | 2 TB | 60K docs/sec | Production, Streaming |

**Notes**:
- Throughput is single-threaded (60K docs/sec validated on AMD Ryzen 9 6900HX)
- Storage requirements assume average document size ~1KB
- For GPU acceleration, add requirements per GPU tier:
  - **iGPU** (AMD/Intel): Built-in, 150K docs/sec (2× speedup)
  - **Entry GPU** (GTX 1650): 4 GB VRAM, 300K docs/sec (4× speedup)
  - **Mid GPU** (RTX 3060): 12 GB VRAM, 500K docs/sec (7× speedup)
  - **High GPU** (RTX 4090): 24 GB VRAM, 1M docs/sec (14× speedup)

### Software Requirements

- **OS**: Linux (Ubuntu 20.04+, RHEL 8+, Debian 11+), macOS 12+, Windows 10+ (WSL2 recommended)
- **Rust**: 1.76.0+ (nightly for SIMD features)
- **Kernel**: Linux 5.4+ (for io_uring support)
- **Filesystem**: ext4, xfs, btrfs (mmap-friendly, O_DIRECT support)
- **GPU Backend** (optional): Vulkan, Metal, DX12, or WebGPU

### Network Requirements

- **HTTP Server**: Port 8080 (configurable)
- **Outbound**: HTTPS (port 443) for license validation (optional)
- **Bandwidth**: 100 Mbps minimum for distributed deployments

---

## Installation

### From Binary (Recommended for Production)

```bash
# Download from kindly.software
wget https://dedup.kindly.software/releases/v3.1.0/kindly_dedup-x86_64-linux.tar.gz

# Verify signature
wget https://dedup.kindly.software/releases/v3.1.0/kindly_dedup-x86_64-linux.tar.gz.asc
gpg --verify kindly_dedup-x86_64-linux.tar.gz.asc kindly_dedup-x86_64-linux.tar.gz

# Extract
tar xzf kindly_dedup-x86_64-linux.tar.gz
cd kindly_dedup-v3.1.0

# Copy to system path
sudo cp kindly_dedup /usr/local/bin/
sudo chmod +x /usr/local/bin/kindly_dedup

# Verify installation
kindly_dedup --version
```

### From Source (Development)

```bash
# Clone repository (requires source access)
git clone https://github.com/kindly-ai/kindly_dedup.git
cd kindly_dedup

# Build release binary
cargo build --release --features "persistent-dedup,audit-trail,gpu-hybrid"

# Optional: Install to system
sudo cp target/release/kindly_dedup /usr/local/bin/
```

### Feature Flags

Choose features based on your deployment needs:

```bash
# Minimal (CPU only, no GPU)
cargo build --release --features "persistent-dedup"

# Full production (GPU + audit trails)
cargo build --release --features "persistent-dedup,audit-trail,gpu-hybrid,adaptive-pipeline"

# Interactive TUI (development)
cargo build --release --bin kindly_dedup --features "interactive"

# GUI application (desktop)
cargo build --release --bin kindly_dedup_gui --features "gui"
```

---

## Bare Metal Deployment

### SystemD Service Configuration

Create `/etc/systemd/system/kindly-dedup.service`:

```ini
[Unit]
Description=Kindly Dedup - LLM Dataset Deduplication Service
After=network.target
Documentation=https://docs.kindly.software/dedup

[Service]
Type=simple
User=kindly-dedup
Group=kindly-dedup
WorkingDirectory=/var/lib/kindly-dedup

# Binary location
ExecStart=/usr/local/bin/kindly_dedup \
    --mode server \
    --port 8080 \
    --data-dir /var/lib/kindly-dedup/data \
    --config /etc/kindly-dedup/config.toml

# Resource limits (adjust per tier)
LimitNOFILE=65536
LimitNPROC=4096
MemoryMax=16G
CPUQuota=1600%  # 16 cores = 1600%

# Restart policy
Restart=on-failure
RestartSec=10s
KillMode=mixed
KillSignal=SIGTERM
TimeoutStopSec=30s

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/kindly-dedup
CapabilityBoundingSet=CAP_NET_BIND_SERVICE

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=kindly-dedup

[Install]
WantedBy=multi-user.target
```

### Service Management

```bash
# Create service user
sudo useradd -r -s /bin/false -d /var/lib/kindly-dedup kindly-dedup

# Create directories
sudo mkdir -p /var/lib/kindly-dedup/{data,logs}
sudo mkdir -p /etc/kindly-dedup
sudo chown -R kindly-dedup:kindly-dedup /var/lib/kindly-dedup

# Install service
sudo systemctl daemon-reload
sudo systemctl enable kindly-dedup
sudo systemctl start kindly-dedup

# Check status
sudo systemctl status kindly-dedup

# View logs
sudo journalctl -u kindly-dedup -f
```

### Log Rotation

Create `/etc/logrotate.d/kindly-dedup`:

```
/var/lib/kindly-dedup/logs/*.log {
    daily
    rotate 30
    compress
    delaycompress
    notifempty
    missingok
    create 0640 kindly-dedup kindly-dedup
    sharedscripts
    postrotate
        systemctl reload kindly-dedup > /dev/null 2>&1 || true
    endscript
}
```

---

## Docker Deployment

### Dockerfile

```dockerfile
FROM rust:1.79-slim-bookworm AS builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy source
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build release
RUN cargo build --release --features "persistent-dedup,audit-trail"

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create user
RUN useradd -r -u 1000 -s /bin/false kindly-dedup

# Copy binary
COPY --from=builder /build/target/release/kindly_dedup /usr/local/bin/

# Create directories
RUN mkdir -p /var/lib/kindly-dedup/data && \
    chown -R kindly-dedup:kindly-dedup /var/lib/kindly-dedup

USER kindly-dedup
WORKDIR /var/lib/kindly-dedup

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/kindly_dedup"]
CMD ["--mode", "server", "--port", "8080", "--data-dir", "/var/lib/kindly-dedup/data"]
```

### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  kindly-dedup:
    build: .
    image: kindly/kindly-dedup:v3.1.0
    container_name: kindly-dedup
    restart: unless-stopped

    ports:
      - "8080:8080"

    volumes:
      - dedup-data:/var/lib/kindly-dedup/data
      - dedup-logs:/var/lib/kindly-dedup/logs
      - ./config.toml:/etc/kindly-dedup/config.toml:ro

    environment:
      - RUST_LOG=info
      - KINDLY_DEDUP_LICENSE_KEY=${LICENSE_KEY}

    # Resource limits (adjust per tier)
    deploy:
      resources:
        limits:
          cpus: '16.0'
          memory: 16G
        reservations:
          cpus: '4.0'
          memory: 4G

    # Health check
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

volumes:
  dedup-data:
    driver: local
  dedup-logs:
    driver: local
```

### Container Management

```bash
# Build and start
docker-compose up -d

# View logs
docker-compose logs -f kindly-dedup

# Scale (for distributed deployments)
docker-compose up -d --scale kindly-dedup=4

# Stop
docker-compose down

# Update
docker-compose pull
docker-compose up -d
```

---

## Production Hardening

### Memory Limits

Enforce O(1) memory usage regardless of corpus size:

```bash
# SystemD (in service file)
MemoryMax=16G
MemoryHigh=14G

# Docker Compose (in deploy.resources.limits)
memory: 16G

# Kubernetes (in resources.limits)
limits:
  memory: "16Gi"
```

### CPU Affinity

Pin deduplication to specific cores for consistent performance:

```bash
# SystemD (in service file)
CPUAffinity=0-15  # Cores 0-15

# Docker
docker run --cpuset-cpus="0-15" kindly/kindly-dedup

# Manual pinning
taskset -c 0-15 kindly_dedup --mode server
```

### File Descriptor Limits

Increase for high-throughput workloads:

```bash
# SystemD (in service file)
LimitNOFILE=65536

# Manual
ulimit -n 65536

# Persistent (in /etc/security/limits.conf)
kindly-dedup soft nofile 65536
kindly-dedup hard nofile 65536
```

### Huge Pages (Optional for >10M documents)

```bash
# Enable huge pages
echo 1024 | sudo tee /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages

# Mount hugetlbfs
sudo mkdir -p /mnt/huge
sudo mount -t hugetlbfs none /mnt/huge

# Add to fstab
echo "none /mnt/huge hugetlbfs defaults 0 0" | sudo tee -a /etc/fstab
```

---

## Performance Tuning

### Thread Count Optimization

Single-threaded is optimal (60K docs/sec validated):

```bash
# Use default (single-threaded DedupPipeline)
kindly_dedup --mode server

# Parallel NOT recommended (regression: 6K docs/sec @ 16 cores)
# ParallelDedupPipeline requires redesign
```

### Batch Size Tuning

For streaming workloads with persistent storage:

```bash
# Small batches (low latency, high overhead)
--batch-size 100

# Optimal (balanced latency/throughput)
--batch-size 1000  # Default

# Large batches (high throughput, higher latency)
--batch-size 10000
```

### GPU Acceleration Setup

#### Detect GPU Capabilities

```bash
# Check GPU detection
kindly_dedup --gpu-info

# Expected output:
# GPU Detected: AMD Radeon 680M (iGPU)
# Backend: Vulkan
# Performance Tier: Entry (2× expected speedup)
```

#### Enable GPU Mode

```bash
# Auto-detect and use GPU if available
kindly_dedup --mode server --gpu auto

# Force GPU mode (fail if no GPU)
kindly_dedup --mode server --gpu force

# Disable GPU (CPU-only)
kindly_dedup --mode server --gpu off
```

#### Adaptive Pipeline (Recommended)

Automatically switches between CPU/GPU based on performance:

```bash
# Enable adaptive mode (T6 Mixed orchestrator)
kindly_dedup --mode server --adaptive-pipeline

# Configuration in config.toml
[adaptive_pipeline]
enabled = true
ema_alpha = 0.3  # Exponential moving average weight
hysteresis = 10  # Consecutive wins before switching
gpu_margin = 0.5  # 50% margin for GPU switch
cpu_margin = 0.2  # 20% margin for CPU switch
```

### Disk I/O Optimization

For persistent deduplication (T9 tier):

```bash
# Use O_DIRECT for mmap (bypass page cache)
echo deadline | sudo tee /sys/block/sda/queue/scheduler

# Increase readahead
sudo blockdev --setra 8192 /dev/sda

# Mount with noatime (reduce write overhead)
sudo mount -o remount,noatime /var/lib/kindly-dedup
```

---

## Monitoring & Observability

### Health Check Endpoint

```bash
# HTTP health check
curl http://localhost:8080/health

# Expected response (HTTP 200):
{
  "status": "healthy",
  "uptime_seconds": 86400,
  "documents_processed": 10000000,
  "throughput_docs_per_sec": 60000,
  "memory_usage_mb": 3500,
  "mode": "gpu"
}
```

### Metrics Endpoint (Prometheus)

```bash
# Prometheus metrics
curl http://localhost:8080/metrics

# Key metrics:
# - kindly_dedup_throughput_docs_per_sec
# - kindly_dedup_memory_usage_bytes
# - kindly_dedup_latency_seconds_bucket
# - kindly_dedup_gpu_utilization_percent
```

### Audit Trail Verification (Q34)

For SOX/SOC2/GDPR/HIPAA compliance:

```bash
# Verify audit trail integrity
kindly_dedup audit verify \
    --input /var/lib/kindly-dedup/audit_trail.jsonl

# Expected output:
# ✓ Hash chain integrity: VALID
# ✓ 1,000,000 entries verified
# ✓ No tampering detected
```

### Log Analysis

```bash
# View real-time logs
journalctl -u kindly-dedup -f

# Filter by level
journalctl -u kindly-dedup -p err -f

# Export to file
journalctl -u kindly-dedup --since "1 hour ago" > /tmp/dedup-logs.txt
```

---

## Troubleshooting

### High Memory Usage

**Symptom**: Memory exceeds tier limits (>16 GB for Tier 4)

**Solution**:
```bash
# Verify O(1) memory guarantee
kindly_dedup --validate-memory --corpus-size 100000000

# If validation fails, check for in-memory LSH buckets (bug)
# Ensure using PersistentDedupPipeline or UniversalDedupPipeline
grep -r "RobinHoodHashCapsule" src/  # Should return NO matches
```

### Low Throughput

**Symptom**: <30K docs/sec (expected 60K)

**Diagnosis**:
```bash
# Profile with flamegraph
cargo flamegraph --release --bin kindly_dedup

# Check CPU affinity
taskset -cp $(pgrep kindly_dedup)

# Verify no CPU throttling
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq
```

### GPU Not Detected

**Symptom**: GPU mode fails with "No GPU detected"

**Solution**:
```bash
# Check Vulkan support (Linux)
vulkaninfo | grep deviceName

# Check Metal support (macOS)
system_profiler SPDisplaysDataType | grep Metal

# Install GPU drivers
# AMD: sudo apt install mesa-vulkan-drivers
# NVIDIA: sudo apt install nvidia-driver-535
# Intel: sudo apt install intel-media-va-driver
```

### Crash Recovery

**Symptom**: Service crashes during large deduplication

**Recovery**:
```bash
# Persistent pipeline supports automatic recovery
# Generation counter validation rebuilds LSH buckets

# Manual recovery
kindly_dedup recover \
    --data-dir /var/lib/kindly-dedup/data \
    --verify-signatures

# Expected: <1 second recovery time
```

### Performance Regression

**Symptom**: Throughput drops over time

**Diagnosis**:
```bash
# Check disk I/O
iostat -x 1 10

# Check memory fragmentation
cat /proc/buddyinfo

# Restart service to clear state
sudo systemctl restart kindly-dedup
```

---

## Support & Contact

- **Documentation**: https://docs.kindly.software/dedup
- **Email**: support@kindly.software
- **Enterprise Support**: enterprise@kindly.software
- **Issue Tracker**: https://github.com/kindly-ai/kindly_dedup/issues (source license holders)

---

**Framework Compliance**: UCE34 Q1-Q34 | Chaos 100% lockfree | ASSUM 99.99% safe | B32 validated | T28 comprehensive | I20 zero-breaking | Q34 audit trails

**Last Updated**: 2025-11-26 | **Version**: v3.1.0
