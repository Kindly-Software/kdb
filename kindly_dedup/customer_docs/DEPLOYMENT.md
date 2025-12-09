# Deployment Guide

Instructions for deploying Kindly Dedup in production environments.

## System Requirements

### Minimum Requirements

- **CPU**: 2 cores, x86_64 or ARM64 architecture
- **RAM**: 2 GB
- **Storage**: 10 GB (1 GB for binary, rest for data)
- **OS**: Linux (Ubuntu 20.04+), macOS (10.15+), Windows Server (2019+)

### Recommended Production Setup

- **CPU**: 8+ cores, modern x86_64 processor (Intel Xeon, AMD EPYC, or equivalent)
- **RAM**: 16-64 GB (depends on dataset size)
- **Storage**: SSD with 100+ GB (NVMe recommended for persistent mode)
- **Network**: 1 Gbps+ for distributed deployments
- **OS**: Ubuntu Server 22.04 LTS or newer

## Installation

### Binary Installation (Recommended)

1. Download the latest release for your platform:

```bash
# Linux (x86_64)
wget https://releases.kindly.ai/kindly-dedup/v3.0.0/kindly-dedup-linux-x64.tar.gz
tar -xzf kindly-dedup-linux-x64.tar.gz
sudo mv kindly-dedup /usr/local/bin/
sudo chmod +x /usr/local/bin/kindly-dedup
```

2. Verify installation:

```bash
kindly-dedup --version
```

### Docker Deployment

Official Docker images available on Docker Hub:

```bash
docker pull kindlyai/kindly-dedup:latest
docker run -v /data:/data kindlyai/kindly-dedup:latest deduplicate --input /data/input.jsonl --output /data/output.json
```

**Docker Compose Example**:

```yaml
version: '3.8'
services:
  kindly-dedup:
    image: kindlyai/kindly-dedup:latest
    volumes:
      - ./data:/data
      - ./storage:/storage
    environment:
      - KINDLY_DEDUP_LICENSE=${LICENSE_KEY}
      - KINDLY_DEDUP_THREADS=16
    command: serve --host 0.0.0.0 --port 8080
    ports:
      - "8080:8080"
```

### Kubernetes Deployment

Example deployment manifest:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: kindly-dedup
spec:
  replicas: 3
  selector:
    matchLabels:
      app: kindly-dedup
  template:
    metadata:
      labels:
        app: kindly-dedup
    spec:
      containers:
      - name: kindly-dedup
        image: kindlyai/kindly-dedup:latest
        resources:
          requests:
            memory: "8Gi"
            cpu: "4"
          limits:
            memory: "16Gi"
            cpu: "8"
        env:
        - name: KINDLY_DEDUP_LICENSE
          valueFrom:
            secretKeyRef:
              name: kindly-license
              key: license-key
        ports:
        - containerPort: 8080
```

## Configuration

### Environment Variables

Set these in your deployment environment:

```bash
# Required
export KINDLY_DEDUP_LICENSE="your-license-key-here"

# Optional
export KINDLY_DEDUP_THREADS=16
export KINDLY_DEDUP_LOG_LEVEL=info
export KINDLY_DEDUP_STORAGE=/var/lib/kindly-dedup
```

### Systemd Service (Linux)

Create `/etc/systemd/system/kindly-dedup.service`:

```ini
[Unit]
Description=Kindly Dedup API Server
After=network.target

[Service]
Type=simple
User=kindly
Group=kindly
WorkingDirectory=/opt/kindly-dedup
Environment="KINDLY_DEDUP_LICENSE=your-license-key"
Environment="KINDLY_DEDUP_THREADS=16"
ExecStart=/usr/local/bin/kindly-dedup serve --host 0.0.0.0 --port 8080
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable kindly-dedup
sudo systemctl start kindly-dedup
sudo systemctl status kindly-dedup
```

### Logging

Configure logging verbosity:

```bash
# Debug mode (verbose)
export KINDLY_DEDUP_LOG_LEVEL=debug

# Production mode (errors only)
export KINDLY_DEDUP_LOG_LEVEL=error
```

Logs are written to stdout/stderr by default. Redirect to files:

```bash
kindly-dedup serve > /var/log/kindly-dedup.log 2>&1
```

## Performance Optimization

### CPU Affinity

Pin processes to specific CPU cores for optimal cache utilization:

```bash
# Linux
taskset -c 0-7 kindly-dedup deduplicate --input data.jsonl --output results.json

# Systemd
[Service]
CPUAffinity=0-7
```

### Memory Limits

Prevent out-of-memory issues:

```bash
# Linux (ulimit)
ulimit -v 16000000  # 16 GB virtual memory limit

# Docker
docker run --memory=16g kindlyai/kindly-dedup:latest

# Kubernetes (see resources section in deployment manifest above)
```

### Storage Configuration

**For Persistent Mode**:

- Use NVMe SSD for best performance
- Ensure 10× dataset size available space
- Mount with noatime for faster I/O:

```bash
# /etc/fstab
/dev/nvme0n1 /var/lib/kindly-dedup ext4 defaults,noatime 0 2
```

### Network Tuning (API Server)

Optimize for high request rates:

```bash
# Increase file descriptor limits
ulimit -n 65536

# TCP tuning (Linux)
sudo sysctl -w net.core.somaxconn=4096
sudo sysctl -w net.ipv4.tcp_max_syn_backlog=4096
```

## High Availability

### Load Balancing

Use HAProxy or Nginx for distributing API requests:

**Nginx Example**:

```nginx
upstream kindly_dedup {
    least_conn;
    server 192.168.1.10:8080;
    server 192.168.1.11:8080;
    server 192.168.1.12:8080;
}

server {
    listen 80;
    location / {
        proxy_pass http://kindly_dedup;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Health Checks

Monitor service health:

```bash
# HTTP health endpoint
curl http://localhost:8080/health

# Expected response: {"status": "healthy", "uptime_seconds": 1234}
```

Integrate with monitoring systems (Prometheus, Datadog, etc.).

## Security

### License Activation

1. Obtain license key from sales@kindly.ai
2. Set environment variable:

```bash
export KINDLY_DEDUP_LICENSE="your-license-key-here"
```

3. Verify activation:

```bash
kindly-dedup --version
# Should show: "Licensed to: Your Organization"
```

### Network Security

**Firewall Configuration**:

```bash
# Allow API port (8080)
sudo ufw allow 8080/tcp

# Restrict to specific IPs
sudo ufw allow from 192.168.1.0/24 to any port 8080
```

**TLS/HTTPS**: Use reverse proxy (Nginx, Caddy) for TLS termination.

### File Permissions

Restrict access to data directories:

```bash
sudo chown -R kindly:kindly /var/lib/kindly-dedup
sudo chmod 750 /var/lib/kindly-dedup
```

## Backup and Recovery

### Persistent Storage Backup

```bash
# Stop service
sudo systemctl stop kindly-dedup

# Backup storage directory
tar -czf kindly-dedup-backup-$(date +%Y%m%d).tar.gz /var/lib/kindly-dedup

# Restart service
sudo systemctl start kindly-dedup
```

### Disaster Recovery

```bash
# Restore from backup
sudo systemctl stop kindly-dedup
tar -xzf kindly-dedup-backup-20251125.tar.gz -C /
sudo systemctl start kindly-dedup
```

Recovery time: < 1 second (automatic state validation)

## Monitoring

### Key Metrics

Monitor these metrics in production:

- **Throughput**: documents/second
- **Latency**: processing time per document
- **Memory Usage**: RSS, heap, persistent storage
- **Error Rate**: failed requests, corrupted documents
- **Uptime**: service availability

### Logging Best Practices

```bash
# Structured JSON logging
export KINDLY_DEDUP_LOG_FORMAT=json

# Log rotation (logrotate)
cat > /etc/logrotate.d/kindly-dedup <<EOF
/var/log/kindly-dedup.log {
    daily
    rotate 7
    compress
    delaycompress
    notifempty
    create 0640 kindly kindly
}
EOF
```

## Troubleshooting

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for common deployment issues.

## Support

For production support, enterprise features, and custom deployments:
- Email: enterprise@kindly.ai
- Documentation: https://docs.kindly.ai
- Status Page: https://status.kindly.ai
