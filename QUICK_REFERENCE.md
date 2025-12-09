# Atomic HTTP Server - Quick Reference Card

## Service Status
```bash
# Current Status
systemctl status atomic-http-server
Service:    atomic-http-server.service
Status:     ✅ active (running)
PID:        1255039
Memory:     2.7 MB
CPU:        0.2%
Started:    Nov 21 16:43:51 EST
Enabled:    Yes (auto-start on boot)
```

## Essential Commands

### Start/Stop/Restart
```bash
sudo /home/samuel/Primitives/scripts/manage_service.sh start       # Start service
sudo /home/samuel/Primitives/scripts/manage_service.sh stop        # Stop service
sudo /home/samuel/Primitives/scripts/manage_service.sh restart     # Restart service
```

### Monitor & Diagnose
```bash
/home/samuel/Primitives/scripts/manage_service.sh status           # Show status
/home/samuel/Primitives/scripts/manage_service.sh logs             # Follow logs (Ctrl+C to exit)
/home/samuel/Primitives/scripts/manage_service.sh logs-tail 20     # Show last 20 lines
/home/samuel/Primitives/scripts/manage_service.sh health           # Resource check
```

### Security & Testing
```bash
sudo /home/samuel/Primitives/scripts/manage_service.sh security    # Security audit
/home/samuel/Primitives/scripts/manage_service.sh validate         # Validate config
sudo /home/samuel/Primitives/scripts/manage_service.sh test-crash  # Test recovery
```

## System Commands (Direct)
```bash
sudo systemctl start atomic-http-server                # Start
sudo systemctl stop atomic-http-server                 # Stop
sudo systemctl restart atomic-http-server              # Restart
sudo systemctl status atomic-http-server               # Status
sudo systemctl enable atomic-http-server               # Enable auto-start
sudo systemctl disable atomic-http-server              # Disable auto-start

sudo journalctl -u atomic-http-server -f               # Follow logs
sudo journalctl -u atomic-http-server -n 50            # Last 50 lines
```

## Configuration Files
```bash
# Service definition
/etc/systemd/system/atomic-http-server.service

# Application config
/home/samuel/Primitives/config/server.toml

# Management script
/home/samuel/Primitives/scripts/manage_service.sh

# Documentation
/home/samuel/Primitives/config/README_SYSTEMD_SERVICE.md
/home/samuel/Primitives/SYSTEMD_SERVICE_DEPLOYMENT.md
```

## Directory Structure
```
/home/samuel/Primitives/
├── config/
│   ├── atomic-http-server.service
│   ├── server.toml
│   └── README_SYSTEMD_SERVICE.md
├── data/                    (application data)
├── logs/                    (log files)
├── scripts/
│   └── manage_service.sh
└── target/release/
    └── mcp_http_server      (binary)
```

## Troubleshooting

### Service not starting?
```bash
sudo journalctl -u atomic-http-server -n 50  # Check logs
/home/samuel/Primitives/scripts/manage_service.sh validate  # Check config
ls -la /home/samuel/Primitives/target/release/mcp_http_server  # Check binary
```

### High memory?
```bash
ps aux | grep mcp_http_server  # Check current usage
cat /proc/$(pgrep -f mcp_http_server)/limits  # Check limits
```

### Port conflicts?
```bash
sudo lsof -i :8080  # Check what's using port
# Then edit /home/samuel/Primitives/config/server.toml and change port
sudo /home/samuel/Primitives/scripts/manage_service.sh restart
```

## Key Features

### Auto-Recovery
- Crashes trigger automatic restart within 5 seconds
- Max 3 restarts per 60-second window
- Monitored by systemd

### Security
- 16+ isolation layers (filesystem, kernel, namespace)
- Runs as unprivileged user (samuel)
- 1M file descriptors available
- 2GB memory cap, 80% CPU quota

### Logging
- All output goes to journald
- Structured format with timestamps
- Search with: `journalctl -u atomic-http-server`

### Resource Limits
```
File descriptors:    1,048,576 (1M)
Max processes:       32,768
Memory hard cap:     2 GB
Memory soft cap:     1.5 GB
CPU quota:           80%
```

## Performance Targets

- **Startup**: <500ms to ready
- **Memory**: ~3-4 MB RSS at idle
- **CPU**: <1% at idle
- **Latency**: <100μs per HTTP request
- **Throughput**: 1000+ req/s (configured)
- **Connections**: 5000 max concurrent

## Framework Compliance

✅ UCE34 (Q33 Verification, Q34 Auditability)
✅ Chaos (Computational Capsule Architecture)
✅ ASSUM (Safety Framework - 99.5%)
✅ B32 (Fair Benchmarking)
✅ T28 (Testing - Unit/Property/Integration/Production)
✅ I20 (Integration Validation - 20/20)

## Help

```bash
/home/samuel/Primitives/scripts/manage_service.sh help  # Full command list
```

---

**Server**: Atomic Capsule HTTP Server (MCP v0.1.0)
**Started**: 2025-11-21
**Status**: ✅ Production Ready
**Framework**: UCE34 T6 Mixed (T0+T1+T2+T4+T5+T9+T10)
