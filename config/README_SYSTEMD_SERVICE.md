# Atomic Capsule HTTP Server - Systemd Service Guide

Production-grade systemd service configuration for the Atomic Capsule HTTP Server (MCP) with security hardening, auto-restart, and comprehensive health monitoring.

## Quick Start

### 1. Install Service

```bash
sudo /home/samuel/Primitives/scripts/manage_service.sh install
```

This will:
- Copy service file to `/etc/systemd/system/atomic-http-server.service`
- Create required directories (`logs`, `data`)
- Reload systemd daemon

### 2. Start Service

```bash
sudo /home/samuel/Primitives/scripts/manage_service.sh start
```

### 3. Enable Auto-Start (Optional)

```bash
sudo /home/samuel/Primitives/scripts/manage_service.sh enable
```

### 4. Verify Status

```bash
/home/samuel/Primitives/scripts/manage_service.sh status
```

## Service Details

### Service File Location

**Primary**: `/etc/systemd/system/atomic-http-server.service`
**Source**: `/home/samuel/Primitives/config/atomic-http-server.service`

### Configuration

**Config File**: `/home/samuel/Primitives/config/server.toml`

Edit to customize:
- Server host/port (default: 127.0.0.1:3030)
- Worker count (default: 8)
- Request timeouts
- Resource limits
- TLS settings
- CORS configuration
- Rate limiting

### Directories

| Path | Purpose | Permissions |
|------|---------|-------------|
| `/home/samuel/Primitives/logs` | Service logs | 755 (samuel:samuel) |
| `/home/samuel/Primitives/data` | Application data | 755 (samuel:samuel) |
| `/home/samuel/Primitives/config` | Configuration | 755 (samuel:samuel) |

## Management Commands

### Service Control

```bash
# Start the service
sudo /home/samuel/Primitives/scripts/manage_service.sh start

# Stop the service
sudo /home/samuel/Primitives/scripts/manage_service.sh stop

# Restart the service
sudo /home/samuel/Primitives/scripts/manage_service.sh restart

# Reload configuration (SIGHUP, graceful reload)
sudo /home/samuel/Primitives/scripts/manage_service.sh reload
```

### Auto-Start Management

```bash
# Enable auto-start on boot
sudo /home/samuel/Primitives/scripts/manage_service.sh enable

# Disable auto-start
sudo /home/samuel/Primitives/scripts/manage_service.sh disable

# Check if enabled
systemctl is-enabled atomic-http-server
```

### Monitoring & Diagnostics

```bash
# Show current status and process info
/home/samuel/Primitives/scripts/manage_service.sh status

# Follow logs in real-time
/home/samuel/Primitives/scripts/manage_service.sh logs

# Show last 50 lines
/home/samuel/Primitives/scripts/manage_service.sh logs-tail

# Run health check
/home/samuel/Primitives/scripts/manage_service.sh health

# Security profile analysis
sudo /home/samuel/Primitives/scripts/manage_service.sh security

# Validate configuration
/home/samuel/Primitives/scripts/manage_service.sh validate
```

### Testing & Recovery

```bash
# Test service startup
sudo /home/samuel/Primitives/scripts/manage_service.sh test-startup

# Test crash recovery (kills process, checks auto-restart)
sudo /home/samuel/Primitives/scripts/manage_service.sh test-crash
```

## Security Hardening

The service implements multiple security layers:

### Privilege Isolation
- **User**: Non-root `samuel` user (unprivileged)
- **NoNewPrivileges**: Prevents privilege escalation
- **PrivateDevices**: No access to system devices

### Filesystem Isolation
- **ProtectSystem=strict**: Read-only `/usr`, `/boot`, `/etc` except specified paths
- **ProtectHome=read-only**: Home directory is read-only
- **ReadWritePaths**: Only `/home/samuel/Primitives/*` is writable
- **PrivateTmp**: Isolated `/tmp` and `/var/tmp`
- **PrivateMounts**: Isolated mount namespace

### Kernel Hardening
- **ProtectKernelTunables**: Prevents sysctl modification
- **ProtectKernelModules**: No kernel module loading
- **ProtectControlGroups**: No cgroup modification
- **LockPersonality**: Prevents personality(2) changes
- **RestrictRealtime**: No real-time scheduling
- **RestrictSUIDSGID**: No SUID/SGID execution

### Network Isolation
- **RestrictAddressFamilies**: Only IPv4, IPv6, and Unix sockets
- **RestrictNamespaces**: Isolated from namespace operations

### Resource Limits
- **LimitNOFILE**: 1,048,576 (1M file descriptors)
- **LimitNPROC**: 32,768 processes
- **TasksMax**: 32,768 tasks
- **MemoryMax**: 2 GB hard limit
- **MemoryHigh**: 1.5 GB soft limit
- **CPUQuota**: 80% CPU maximum

### IPC Isolation
- **RemoveIPC**: Message queues and semaphores removed on exit

## Auto-Restart Configuration

The service will automatically restart on crashes:

```ini
Restart=always                    # Always restart
RestartSec=5s                     # Wait 5 seconds before restart
StartLimitInterval=60s            # 60-second window
StartLimitBurst=3                 # Max 3 restarts per window
```

**Behavior**: If the service crashes, it will restart up to 3 times within any 60-second period, with 5 seconds between attempts.

## Resource Limits Explained

### File Descriptors (LimitNOFILE=1048576)
- Supports ~500K concurrent connections
- Each connection needs 2+ file descriptors (socket, optional buffering)
- Typical deployment needs: 10K-100K connections

### Processes (LimitNPROC=32768)
- Supports 32K threads/lightweight processes
- Worker pool can safely scale to 8-16 workers
- NUMA rebalancing threads included

### Memory
- **Hard limit**: 2 GB (MemoryMax)
- **Soft limit**: 1.5 GB (MemoryHigh) - warns before limit
- Prevents runaway memory consumption

### CPU (CPUQuota=80%)
- Limits to 80% of total CPU on single core
- Multi-core systems: 0.8 × number_of_cores available
- Prevents monopolizing system resources

## Logging

Logs are sent to **journald** (systemd journal):

### View Logs

```bash
# Real-time follow
sudo journalctl -u atomic-http-server -f

# Last 50 lines
sudo journalctl -u atomic-http-server -n 50

# Last hour
sudo journalctl -u atomic-http-server --since "1 hour ago"

# By priority
sudo journalctl -u atomic-http-server -p warn  # Warnings and errors
sudo journalctl -u atomic-http-server -p err   # Errors only

# With timestamps
sudo journalctl -u atomic-http-server -o short-precise

# Full output
sudo journalctl -u atomic-http-server -o verbose
```

### Log Configuration

Current setup logs to journald only. To add file logging, edit `/home/samuel/Primitives/config/server.toml`:

```toml
[logging]
level = "info"
format = "json"
output = "journald"
# Uncomment for file logging:
# file_path = "/home/samuel/Primitives/logs/server.log"
# file_rotation = "daily"
# file_retention_days = 7
```

## Performance Monitoring

### Check Resource Usage

```bash
/home/samuel/Primitives/scripts/manage_service.sh health
```

Shows:
- Process ID and responsiveness
- Memory usage (RSS and virtual)
- CPU percentage
- Resource limits

### Monitor with systemd

```bash
# Real-time monitoring
systemd-cgtop -u samuel -b -n 10

# Memory pressure
systemctl status atomic-http-server --no-pager --full

# CPU time
ps -u samuel -o pid,cputime,cmd | grep mcp_http_server
```

### Metrics Endpoint

If metrics are enabled in config (default):

```bash
curl http://127.0.0.1:9090/metrics
```

Prometheus format metrics available at `/metrics` on port 9090.

## Troubleshooting

### Service Won't Start

```bash
# Check for errors
sudo journalctl -u atomic-http-server -n 50

# Validate configuration
/home/samuel/Primitives/scripts/manage_service.sh validate

# Run security check
sudo /home/samuel/Primitives/scripts/manage_service.sh security

# Check permissions
ls -la /home/samuel/Primitives/target/release/mcp_http_server
ls -la /home/samuel/Primitives/config/
```

### Service Crashes Repeatedly

```bash
# Check system resources
free -h
df -h /home/samuel/Primitives

# Monitor logs
sudo journalctl -u atomic-http-server -f

# Check restart count
systemctl show atomic-http-server -p NRestarts

# View restart times
sudo journalctl -u atomic-http-server | grep "restart"
```

### High Memory Usage

1. Check limits:
   ```bash
   cat /proc/$(pgrep -f mcp_http_server)/limits
   ```

2. Adjust memory limit in service file:
   ```ini
   MemoryMax=4G          # Increase hard limit
   MemoryHigh=3G         # Increase soft limit
   ```

3. Reload and restart:
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl restart atomic-http-server
   ```

### Port Already in Use

Edit `/home/samuel/Primitives/config/server.toml`:

```toml
[server]
port = 3031  # Change to different port
```

Then restart service:
```bash
sudo /home/samuel/Primitives/scripts/manage_service.sh restart
```

## System Integration

### Boot Integration

After enabling auto-start:
```bash
sudo /home/samuel/Primitives/scripts/manage_service.sh enable
```

The service will automatically start when the system boots.

### Dependency Management

Service depends on:
- `network-online.target` - Waits for network to be ready
- `multi-user.target` - Standard system target

### Service Dependencies

Other services can depend on this service:

```ini
[Unit]
After=atomic-http-server.service
Requires=atomic-http-server.service
```

## Advanced Configuration

### Custom Environment Variables

Edit service file, add:
```ini
[Service]
Environment="CUSTOM_VAR=value"
Environment="RUST_BACKTRACE=full"
```

### Custom Command-Line Args

Edit service file, modify ExecStart:
```ini
ExecStart=/home/samuel/Primitives/target/release/mcp_http_server \
    --config /path/to/config.toml \
    --log-level debug
```

### Multiple Instances

For multiple service instances:

1. Copy service file:
   ```bash
   cp atomic-http-server.service atomic-http-server-2.service
   ```

2. Edit port and name in service file

3. Install both:
   ```bash
   sudo cp atomic-http-server.service /etc/systemd/system/
   sudo cp atomic-http-server-2.service /etc/systemd/system/
   sudo systemctl daemon-reload
   ```

4. Start individually:
   ```bash
   sudo systemctl start atomic-http-server
   sudo systemctl start atomic-http-server-2
   ```

## Compliance & Standards

### Security Standards
- **CIS Docker Benchmark**: Most security controls implemented
- **NIST Cybersecurity Framework**: Access control, isolation
- **PCI-DSS**: Logging, resource control
- **HIPAA**: Encryption-capable, audit trails

### Performance Standards
- **Target Throughput**: 1000+ req/s
- **Target Latency**: <100ms p95
- **Target Uptime**: 99.99%+
- **Recovery Time**: <10 seconds after crash

## Framework Compliance (UCE34)

### Q33 Verification
- ✅ Service starts on boot
- ✅ Auto-restarts on crash (max 3 per 60s)
- ✅ Security hardening applied
- ✅ Resource limits enforced
- ✅ Logging configured
- ✅ Health monitoring available

### Q34 Auditability
- ✅ Comprehensive journald logging
- ✅ Structured JSON format
- ✅ Process tracking (PID, threads)
- ✅ Resource consumption monitoring

## Related Files

- **Service**: `/etc/systemd/system/atomic-http-server.service`
- **Config**: `/home/samuel/Primitives/config/server.toml`
- **Manager**: `/home/samuel/Primitives/scripts/manage_service.sh`
- **Binary**: `/home/samuel/Primitives/target/release/mcp_http_server`
- **Logs**: `journalctl -u atomic-http-server`

## Version

- **Service Version**: 1.0 (Production)
- **systemd Version**: 255+ (Ubuntu 24.04)
- **Created**: November 2025
- **Framework**: UCE34 (Computational Capsule Architecture)

## Support

For issues or questions:
1. Check logs: `sudo journalctl -u atomic-http-server -f`
2. Run diagnostics: `/home/samuel/Primitives/scripts/manage_service.sh health`
3. Review config: `/home/samuel/Primitives/config/server.toml`
4. Security audit: `sudo /home/samuel/Primitives/scripts/manage_service.sh security`
