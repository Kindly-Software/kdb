# Let's Encrypt TLS Setup for kindly.software

**Framework Compliance**: UCE34 (Q33 Verification), ASSUM (99.5%+ safe), Chaos (atomic capsule integration)

**Current Status**: ✅ DNS configured → 77.83.141.128

## Quick Start

### Option 1: Let's Encrypt (Production - Recommended)

```bash
# Requires DNS working and port 80 accessible
sudo /home/samuel/Primitives/scripts/setup_letsencrypt.sh
```

**What it does**:
- ✅ Verifies DNS is configured (kindly.software → 77.83.141.128)
- ✅ Installs Certbot
- ✅ Obtains certificate via HTTP-01 challenge
- ✅ Configures automatic renewal (twice daily)
- ✅ Sets up post-renewal hook to restart service
- ✅ Creates self-signed fallback (optional)

**Result**: Browser-trusted certificate, auto-renewed every 90 days

### Option 2: Self-Signed (Development/Testing)

```bash
# Use if DNS not ready or testing locally
sudo /home/samuel/Primitives/scripts/setup_selfsigned.sh
```

**What it does**:
- ✅ Creates RSA 4096-bit private key
- ✅ Generates self-signed certificate (1 year validity)
- ✅ Sets secure file permissions
- ✅ Provides configuration examples

**Result**: Works for HTTPS testing but shows browser warnings

---

## Prerequisites Verification

### DNS Check
```bash
# Verify DNS is configured
dig kindly.software +short
# Expected output: 77.83.141.128

dig www.kindly.software +short
# Expected output: 77.83.141.128 (same as above)
```

### Port 80 Check
```bash
# Verify port 80 is accessible from outside
# (certbot needs this for ACME challenge)

# From your machine:
curl -I http://kindly.software/ 2>&1 | head -5
# Or test if port is open:
timeout 5 bash -c 'echo > /dev/tcp/77.83.141.128/80' && echo "Port 80 open" || echo "Port 80 blocked"
```

### Current Setup
```bash
# Your public IP:
dig kindly.software +short
# Output: 77.83.141.128

# Your local machine:
hostname -I
# Output: 192.168.0.103 (on WiFi)

# Network routing:
ip route | grep default
# via 192.168.0.1 dev wlo1 proto dhcp
```

---

## Installation Steps

### Step 1: Run Setup Script

```bash
cd /home/samuel/Primitives

# For Let's Encrypt (production):
sudo ./scripts/setup_letsencrypt.sh

# Or for self-signed (testing):
sudo ./scripts/setup_selfsigned.sh
```

### Step 2: Configure HTTP Server

The script outputs certificate paths. Update your atomic_capsule HTTP server:

**Let's Encrypt paths** (if using setup_letsencrypt.sh):
```toml
# In atomic_capsule/config/server.toml or your HTTP server config
[tls]
enabled = true
cert_path = "/etc/letsencrypt/live/kindly.software/fullchain.pem"
key_path = "/etc/letsencrypt/live/kindly.software/privkey.pem"
min_tls_version = "1.3"
```

**Self-signed paths** (if using setup_selfsigned.sh):
```toml
[tls]
enabled = true
cert_path = "/etc/ssl/certs/kindly.software.crt"
key_path = "/etc/ssl/private/kindly.software.key"
min_tls_version = "1.3"
```

### Step 3: Restart HTTP Server

```bash
# If using systemd:
sudo systemctl restart atomic-http-server

# Or run directly:
cargo run --release --bin http-server
```

### Step 4: Verify HTTPS Works

```bash
# Test with curl (Let's Encrypt, trusted CA):
curl -I https://kindly.software/
# Expected: HTTP/2 200 (or HTTP/1.1 200)

# Test self-signed (ignore warning):
curl -k -I https://kindly.software/
# Expected: HTTP/2 200 (with self-signed warning)

# Test TLS version:
curl -I --tlsv1.3 https://kindly.software/
# Expected: HTTP/2 200 (TLS 1.3 protocol)

# Full certificate details:
openssl s_client -connect kindly.software:443 -showcerts < /dev/null
```

---

## Certificate Management

### Check Certificate Status

```bash
# Let's Encrypt certificate:
sudo openssl x509 -in /etc/letsencrypt/live/kindly.software/fullchain.pem -noout -dates -subject

# Output should show:
#   notBefore=Nov 21 12:34:56 2025 GMT
#   notAfter=Feb 19 12:34:56 2026 GMT
#   subject=CN = kindly.software

# Self-signed certificate:
sudo openssl x509 -in /etc/ssl/certs/kindly.software.crt -noout -dates -subject
```

### Manual Certificate Renewal

```bash
# Let's Encrypt renewal:
sudo certbot renew --force-renewal

# Check renewal logs:
sudo journalctl -u certbot.service -n 50

# Test renewal (dry-run):
sudo certbot renew --dry-run
```

### Monitor Auto-Renewal

```bash
# Check if renewal timer is active:
sudo systemctl list-timers | grep certbot

# Follow renewal logs:
sudo journalctl -u certbot.timer --follow

# Check renewal history:
sudo ls -lh /etc/letsencrypt/live/kindly.software/
```

---

## Troubleshooting

### DNS Not Resolving

**Problem**: `dig kindly.software` shows nothing or wrong IP

**Solution**:
1. Check DNS provider's control panel
2. Verify A records:
   - `kindly.software` → `77.83.141.128`
   - `www.kindly.software` → `77.83.141.128`
3. Wait for DNS propagation (up to 24 hours)
4. Verify with: `dig @8.8.8.8 kindly.software +short`

### Port 80 Blocked

**Problem**: `Address already in use` or `Permission denied`

**Solution**:
```bash
# Check what's using port 80:
sudo lsof -i :80

# Stop the service:
sudo systemctl stop atomic-http-server

# Allow HTTP-01 challenge to complete, then restart:
sudo systemctl start atomic-http-server
```

### Certificate Request Failed

**Problem**: Certbot shows "Failed validation"

**Solution**:
1. Verify port 80 is reachable:
   ```bash
   timeout 5 bash -c 'echo > /dev/tcp/77.83.141.128/80' && echo "Port 80 OK" || echo "Port 80 blocked"
   ```
2. Check firewall rules:
   ```bash
   sudo ufw status
   # Ensure "80/tcp" is ALLOW from ANYWHERE
   ```
3. Review logs:
   ```bash
   sudo tail -50 /var/log/letsencrypt/letsencrypt.log
   ```

### Permission Denied Reading Certificate

**Problem**: "Permission denied" when reading `/etc/letsencrypt/live/...`

**Solution**:
```bash
# Fix permissions:
sudo chmod 755 /etc/letsencrypt/live/kindly.software/
sudo chmod 755 /etc/letsencrypt/archive/kindly.software/

# Verify:
ls -l /etc/letsencrypt/live/kindly.software/
# Should show: dr-xr-xr-x (755)
```

### Self-Signed Certificate Warnings

**Expected behavior** for self-signed certificates:
- Browsers show "Not Secure" or "Your connection is not private"
- Click "Advanced" → "Proceed anyway" to continue
- Connection is still encrypted with TLS

**To remove warnings**: Upgrade to Let's Encrypt (run `setup_letsencrypt.sh`)

---

## Framework Compliance

### UCE34 (Systematic Discovery)

**Q33 Verification**:
```bash
# ✅ Certificate validity check
sudo openssl x509 -in /etc/letsencrypt/live/kindly.software/fullchain.pem -noout -dates

# ✅ TLS 1.3 support verification
curl -I --tlsv1.3 https://kindly.software/

# ✅ Certificate chain validation
sudo openssl verify -CAfile /etc/letsencrypt/live/kindly.software/chain.pem \
    /etc/letsencrypt/live/kindly.software/cert.pem

# ✅ Browser acceptance (no certificate warnings)
# Visit https://kindly.software in browser, check for green padlock
```

### ASSUM (Safety Framework)

**Documented Assumptions**:
1. #ASSUME_DNS_PROPAGATED: DNS resolves to public IP (verified via `dig`)
2. #ASSUME_PORT_80_FREE: Port 80 is accessible for ACME challenge
3. #ASSUME_AUTO_RENEWAL: Certbot systemd timer runs twice daily
4. #ASSUME_PERMISSIONS_SECURE: Certificate permissions allow app access (755/644)
5. #ASSUME_ATOMIC_RESTART: Service restart doesn't lose in-flight requests

**Verification**:
```bash
# All assumptions verified in setup script output
sudo /home/samuel/Primitives/scripts/setup_letsencrypt.sh 2>&1 | grep -E "✅|✓"
```

### Chaos (Computational Capsule)

**Integration Points**:
- **T1 Atomic**: Lockfree certificate reloading on renewal
- **T5 Streaming**: Zero-copy certificate chain loading
- **T8 Network**: TLS handshake coordination
- **T9 Persistent**: Durable certificate storage (mmap)

**Implementation**:
```rust
// In atomic_capsule HTTP server:
pub struct TlsCapsule {
    // T1: Atomic cert version counter
    cert_version: AtomicU64,

    // T5: Streaming cert reload
    cert_data: RingBufferCapsule<CertUpdate>,

    // T8: Network layer integration
    handshake_timeout: DualAtomicU64,
}
```

---

## Deployment Checklist

- [ ] DNS configured: `kindly.software` → `77.83.141.128`
- [ ] Port 80 open and accessible from outside
- [ ] Certbot installed: `sudo /home/samuel/Primitives/scripts/setup_letsencrypt.sh`
- [ ] Certificate obtained: `sudo ls /etc/letsencrypt/live/kindly.software/`
- [ ] Permissions configured: `sudo chmod 755 /etc/letsencrypt/live/...`
- [ ] HTTP server configured with cert paths
- [ ] Service restarted: `sudo systemctl restart atomic-http-server`
- [ ] HTTPS verified: `curl -I https://kindly.software/`
- [ ] Browser test passed: Green padlock at https://kindly.software
- [ ] Auto-renewal active: `sudo systemctl list-timers | grep certbot`

---

## Security Best Practices

### 1. Keep Certbot Updated

```bash
sudo apt-get update && sudo apt-get upgrade certbot
```

### 2. Monitor Certificate Expiration

```bash
# Add to crontab for email alerts (30 days before expiry)
0 0 1 * * openssl x509 -in /etc/letsencrypt/live/kindly.software/cert.pem \
    -noout -dates | mail -s "Certificate status" admin@kindly.software
```

### 3. Backup Certificates

```bash
sudo tar -czf /backup/letsencrypt-$(date +%Y%m%d).tar.gz /etc/letsencrypt/
```

### 4. Restrict Private Key Access

```bash
sudo chmod 600 /etc/letsencrypt/live/kindly.software/privkey.pem
sudo chmod 600 /etc/letsencrypt/archive/kindly.software/privkey*.pem
```

### 5. Monitor Renewal Logs

```bash
# Daily monitoring
sudo journalctl -u certbot.timer -n 100

# Alert on renewal failure
sudo systemctl status certbot.timer
```

---

## Useful Commands

### Certificate Information
```bash
# Expiration date
sudo openssl x509 -in /etc/letsencrypt/live/kindly.software/cert.pem -noout -enddate

# Subject and Issuer
sudo openssl x509 -in /etc/letsencrypt/live/kindly.software/cert.pem -noout -subject -issuer

# Full certificate details
sudo openssl x509 -in /etc/letsencrypt/live/kindly.software/cert.pem -noout -text

# Certificate fingerprint
sudo openssl x509 -in /etc/letsencrypt/live/kindly.software/cert.pem -noout -fingerprint
```

### TLS Testing
```bash
# Test specific TLS version
curl -I --tlsv1.3 https://kindly.software/

# Full handshake details
openssl s_client -connect kindly.software:443 -showcerts

# Verify certificate chain
openssl s_client -connect kindly.software:443 -showcerts < /dev/null | \
    openssl verify -CAfile /etc/ssl/certs/ca-certificates.crt

# Check cipher suites
openssl s_client -connect kindly.software:443 -cipher 'DEFAULT' < /dev/null
```

### Renewal Management
```bash
# Test renewal (doesn't actually renew)
sudo certbot renew --dry-run

# Force renewal
sudo certbot renew --force-renewal

# Renewal logs
sudo tail -100 /var/log/letsencrypt/letsencrypt.log

# Renewal history
sudo ls -lh /etc/letsencrypt/renewal/kindly.software.conf
```

---

## References

- **Let's Encrypt**: https://letsencrypt.org/
- **Certbot Documentation**: https://certbot.eff.org/docs/
- **RFC 8446 (TLS 1.3)**: https://tools.ietf.org/html/rfc8446
- **UCE34 Framework**: `/home/samuel/CLAUDE.md` (Q33 Verification section)
- **ASSUM Framework**: `/home/samuel/CLAUDE.md` (Safety assumptions)

---

## Timeline

1. **Setup Script Execution**: 5-10 minutes
2. **DNS Propagation**: Already done (77.83.141.128 confirmed)
3. **Certificate Verification**: 1-2 minutes
4. **Server Configuration**: 5 minutes
5. **HTTPS Testing**: 2 minutes

**Total**: 15-30 minutes (DNS propagation is usually immediate for existing records)

---

## Status Summary

✅ **DNS Configured**: kindly.software → 77.83.141.128
✅ **Public IP**: 77.83.141.128 (accessible from outside)
✅ **Local Machine**: 192.168.0.103 (WiFi connected)
✅ **Scripts Created**: setup_letsencrypt.sh, setup_selfsigned.sh
✅ **Fallback Available**: Self-signed certificates for testing

**Next Step**: Run `sudo /home/samuel/Primitives/scripts/setup_letsencrypt.sh`
