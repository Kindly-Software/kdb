# KDB Signup Service Deployment Guide

Production deployment configuration for the Kindly Debugger signup service with Ed25519 license generation, email verification, and early adopter tracking.

## Overview

**Service**: kdb-signup
**Port**: 8091
**Protocol**: HTTP JSON API
**Target**: kindly-hub (192.168.0.38)
**User**: kdb
**WorkingDirectory**: /opt/kdb-signup

## Quick Start

```bash
# 1. Deploy to kindly-hub (builds, copies, installs, starts)
cd /home/samuel/Primitives/Kindly-Debugger/kdb-signup
./deploy/deploy.sh

# 2. Configure environment (if first-time deployment)
ssh samuel@kindly-hub
sudo nano /etc/kdb/kdb-signup.env
# Fill in RESEND_API_KEY, SIGNING_KEY_PATH, etc.

# 3. Start service
sudo systemctl start kdb-signup

# 4. Verify health
curl http://localhost:8091/health
```

## Prerequisites

### Local Machine

- Rust toolchain (nightly recommended)
- SSH access to kindly-hub
- SSH keys configured for samuel@kindly-hub

### Remote Machine (kindly-hub)

- Ubuntu Server 24.04 LTS
- Systemd
- curl (for health checks)
- User: kdb (created automatically by deploy.sh)

### External Services

- **Resend Account**: Get API key from https://resend.com/api-keys
- **Verified Domain**: Configure noreply@kindly.software in Resend
- **Ed25519 Key Pair**: Generate with `kdb keygen --output /etc/kdb/private_key.hex`

## Environment Variables

### Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `RESEND_API_KEY` | Resend API key for email delivery | `re_123abc...` |
| `SIGNING_KEY_PATH` | Path to Ed25519 private key (hex-encoded) | `/etc/kdb/private_key.hex` |
| `KINDLYDB_URL` | KindlyDB API endpoint | `http://localhost:8080` |
| `VERIFICATION_URL` | Email verification service URL | `https://api.kindly.software/v1/verify` |
| `FROM_EMAIL` | Sender email address (must be verified in Resend) | `noreply@kindly.software` |
| `PROMO_END_TIMESTAMP` | Unix timestamp for early adopter promo end | `1734220799` |

### Optional Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Logging level (error/warn/info/debug/trace) | `info` |
| `PORT` | HTTP server port | `8091` |
| `HOST` | HTTP server bind address | `0.0.0.0` |
| `COMPANY_NAME` | Email template company name | `Kindly Technologies` |
| `SUPPORT_EMAIL` | Email template support address | `support@kindly.software` |

### Generating Required Values

**Ed25519 Key Pair** (run on kindly-hub):
```bash
# Generate private key
kdb keygen --output /etc/kdb/private_key.hex

# Extract public key (for distribution)
kdb pubkey --input /etc/kdb/private_key.hex --output /etc/kdb/public_key.hex
```

**Promo End Timestamp** (7 days from now):
```bash
date -d "+7 days" +%s
# Example output: 1734220799
```

**Resend API Key**:
1. Sign up at https://resend.com
2. Verify domain (kindly.software)
3. Create API key: Dashboard → API Keys → Create API Key
4. Copy key (starts with `re_`)

## Deployment

### Full Deployment (Build + Deploy)

```bash
cd /home/samuel/Primitives/Kindly-Debugger/kdb-signup
./deploy/deploy.sh
```

**Steps**:
1. Check SSH connectivity to kindly-hub
2. Build release binary locally (`cargo build --release`)
3. Verify binary size and permissions
4. Create remote directories (/opt/kdb-signup, /etc/kdb, /var/log/kdb-signup)
5. Create kdb user (if not exists)
6. Copy binary to remote
7. Install systemd service file
8. Check/copy environment configuration
9. Enable and start service
10. Verify health endpoint

### Deploy Without Rebuild

```bash
./deploy/deploy.sh --no-build
```

Use when binary is already built and you only want to update the remote installation.

### Restart Only

```bash
./deploy/deploy.sh --restart-only
```

Restart service without rebuilding or copying files.

## Service Management

### SystemD Commands

```bash
# Start service
sudo systemctl start kdb-signup

# Stop service
sudo systemctl stop kdb-signup

# Restart service
sudo systemctl restart kdb-signup

# Enable auto-start on boot
sudo systemctl enable kdb-signup

# Disable auto-start
sudo systemctl disable kdb-signup

# Check status
sudo systemctl status kdb-signup

# View logs (last 50 lines)
sudo journalctl -u kdb-signup -n 50

# Follow logs (real-time)
sudo journalctl -u kdb-signup -f

# View logs since 1 hour ago
sudo journalctl -u kdb-signup --since "1 hour ago"
```

### Remote Management

```bash
# Status
ssh samuel@kindly-hub 'sudo systemctl status kdb-signup'

# Logs
ssh samuel@kindly-hub 'sudo journalctl -u kdb-signup -n 100'

# Restart
ssh samuel@kindly-hub 'sudo systemctl restart kdb-signup'

# Health check
ssh samuel@kindly-hub 'curl http://localhost:8091/health'
```

## Health Checks

### Health Endpoint

```bash
curl http://localhost:8091/health
```

**Expected Response**:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

**Status Codes**:
- `200 OK`: Service healthy
- `503 Service Unavailable`: Service degraded (check logs)

### Service Status Verification

```bash
# Check systemd service status
sudo systemctl status kdb-signup

# Expected output should show:
# Active: active (running)
# Main PID: [number]
```

### Port Verification

```bash
# Check if service is listening on port 8091
sudo ss -tlnp | grep 8091

# Expected output:
# LISTEN 0 128 0.0.0.0:8091 0.0.0.0:* users:(("kdb-signup",pid=12345,fd=3))
```

## API Endpoints

### POST /signup

Early adopter signup with email verification and license generation.

**Request**:
```bash
curl -X POST http://localhost:8091/signup \
  -H "Content-Type: application/json" \
  -d '{"email": "user@example.com"}'
```

**Response (202 Accepted)**:
```json
{
  "message": "Verification email sent. Please check your inbox.",
  "expires_in_seconds": 900
}
```

**Response (400 Bad Request)**:
```json
{
  "error": "Invalid email format"
}
```

**Response (410 Gone)**:
```json
{
  "error": "Early adopter promotion has ended"
}
```

### GET /health

Service health check.

**Request**:
```bash
curl http://localhost:8091/health
```

**Response (200 OK)**:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

## Troubleshooting

### Service Won't Start

**Check logs**:
```bash
sudo journalctl -u kdb-signup -n 100
```

**Common issues**:

1. **Missing environment file**:
   ```
   Error: /etc/kdb/kdb-signup.env not found
   ```
   **Fix**: Copy template and configure:
   ```bash
   sudo cp /opt/kdb-signup/deploy/kdb-signup.env.template /etc/kdb/kdb-signup.env
   sudo nano /etc/kdb/kdb-signup.env
   sudo chmod 600 /etc/kdb/kdb-signup.env
   sudo chown kdb:kdb /etc/kdb/kdb-signup.env
   ```

2. **Port 8091 already in use**:
   ```
   Error: Address already in use (os error 98)
   ```
   **Fix**: Find and kill process using port:
   ```bash
   sudo ss -tlnp | grep 8091
   sudo kill [PID]
   ```

3. **Missing signing key**:
   ```
   Error: Cannot read signing key from /etc/kdb/private_key.hex
   ```
   **Fix**: Generate Ed25519 key pair:
   ```bash
   kdb keygen --output /etc/kdb/private_key.hex
   sudo chmod 600 /etc/kdb/private_key.hex
   sudo chown kdb:kdb /etc/kdb/private_key.hex
   ```

4. **Permission denied**:
   ```
   Error: Permission denied (os error 13)
   ```
   **Fix**: Check file ownership and permissions:
   ```bash
   sudo chown -R kdb:kdb /opt/kdb-signup
   sudo chown -R kdb:kdb /var/log/kdb-signup
   sudo chmod 600 /etc/kdb/kdb-signup.env
   ```

### Email Delivery Issues

**Check Resend API key**:
```bash
# Test API key manually
curl https://api.resend.com/emails \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "from": "noreply@kindly.software",
    "to": "test@example.com",
    "subject": "Test",
    "html": "<p>Test email</p>"
  }'
```

**Common issues**:
- Invalid API key (check /etc/kdb/kdb-signup.env)
- Unverified sender domain (verify in Resend dashboard)
- Rate limiting (check Resend dashboard for quota)

### Health Check Fails

**Check service status**:
```bash
sudo systemctl status kdb-signup
```

**Check if service is listening**:
```bash
sudo ss -tlnp | grep 8091
```

**Check logs for errors**:
```bash
sudo journalctl -u kdb-signup -n 50
```

**Test locally on server**:
```bash
ssh samuel@kindly-hub 'curl http://localhost:8091/health'
```

### High CPU/Memory Usage

**Check resource usage**:
```bash
sudo systemctl status kdb-signup
# Look for Memory and CPU lines
```

**Check logs for errors**:
```bash
sudo journalctl -u kdb-signup --since "1 hour ago" | grep -i error
```

**Restart service**:
```bash
sudo systemctl restart kdb-signup
```

## Security

### File Permissions

```bash
# Binary
sudo chown kdb:kdb /opt/kdb-signup/kdb-signup
sudo chmod 755 /opt/kdb-signup/kdb-signup

# Environment file (contains secrets)
sudo chown kdb:kdb /etc/kdb/kdb-signup.env
sudo chmod 600 /etc/kdb/kdb-signup.env

# Signing key (private key)
sudo chown kdb:kdb /etc/kdb/private_key.hex
sudo chmod 600 /etc/kdb/private_key.hex

# Config directory
sudo chmod 700 /etc/kdb

# Log directory
sudo chown -R kdb:kdb /var/log/kdb-signup
sudo chmod 750 /var/log/kdb-signup
```

### SystemD Security Features

- **NoNewPrivileges**: Prevents privilege escalation
- **PrivateTmp**: Isolated /tmp directory
- **ProtectSystem=strict**: Read-only system directories
- **ProtectHome=true**: No access to user home directories
- **ReadOnlyPaths**: /etc/kdb (read-only access to config)

### Network Security

- Service binds to `0.0.0.0:8091` (all interfaces)
- Use firewall (ufw) to restrict access:
  ```bash
  sudo ufw allow from 192.168.0.0/24 to any port 8091
  sudo ufw enable
  ```

## Monitoring

### Log Rotation

Systemd handles log rotation automatically via journald.

**Configure retention**:
```bash
# Edit /etc/systemd/journald.conf
sudo nano /etc/systemd/journald.conf

# Set retention
SystemMaxUse=500M
SystemKeepFree=1G
SystemMaxFileSize=100M

# Restart journald
sudo systemctl restart systemd-journald
```

### Prometheus Metrics (Future)

Planned integration with atomic_capsule telemetry for:
- Request rate (/signup endpoint)
- Email delivery success rate
- Early adopter count
- License generation latency
- Error rate

## Backup and Recovery

### Backup Environment Config

```bash
# Backup to local machine
scp samuel@kindly-hub:/etc/kdb/kdb-signup.env ./backup/kdb-signup.env.$(date +%Y%m%d)
```

### Backup Signing Key

```bash
# Backup private key (CRITICAL)
scp samuel@kindly-hub:/etc/kdb/private_key.hex ./backup/private_key.hex.$(date +%Y%m%d)

# Store securely offline
```

### Restore Service

```bash
# 1. Copy backup files to remote
scp ./backup/kdb-signup.env.20251207 samuel@kindly-hub:/tmp/
scp ./backup/private_key.hex.20251207 samuel@kindly-hub:/tmp/

# 2. Restore on remote
ssh samuel@kindly-hub
sudo cp /tmp/kdb-signup.env.20251207 /etc/kdb/kdb-signup.env
sudo cp /tmp/private_key.hex.20251207 /etc/kdb/private_key.hex
sudo chown kdb:kdb /etc/kdb/*
sudo chmod 600 /etc/kdb/*

# 3. Restart service
sudo systemctl restart kdb-signup
```

## Files and Directories

### Local (Development)

```
/home/samuel/Primitives/Kindly-Debugger/kdb-signup/
├── deploy/
│   ├── deploy.sh                    # Deployment script
│   ├── kdb-signup.service          # SystemD unit file
│   ├── kdb-signup.env.template     # Environment template
│   └── README.md                   # This file
├── src/
│   └── main.rs                     # Service implementation
├── Cargo.toml
└── target/release/kdb-signup       # Binary (after build)
```

### Remote (Production - kindly-hub)

```
/opt/kdb-signup/
└── kdb-signup                      # Binary

/etc/kdb/
├── kdb-signup.env                  # Environment config (secrets)
└── private_key.hex                 # Ed25519 private key

/etc/systemd/system/
└── kdb-signup.service              # SystemD unit file

/var/log/kdb-signup/                # Logs (if needed beyond journald)
```

## References

- **Service Implementation**: `/home/samuel/Primitives/Kindly-Debugger/kdb-signup/src/main.rs`
- **Universal Config**: `/home/samuel/CLAUDE.md` (UCE34 framework, infrastructure)
- **Resend Documentation**: https://resend.com/docs
- **SystemD Documentation**: https://www.freedesktop.org/software/systemd/man/systemd.service.html

## Support

**Logs**: `sudo journalctl -u kdb-signup -f`
**Health**: `curl http://localhost:8091/health`
**Status**: `sudo systemctl status kdb-signup`
**Remote**: `ssh samuel@kindly-hub`
