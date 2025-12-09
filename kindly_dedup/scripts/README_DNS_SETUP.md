# DNS Configuration Guide for dedup.kindly.software

**Sprint Day 1 Task** - Distribution Infrastructure Setup
**Time Estimate**: 1-2 hours (including DNS propagation)
**Framework**: UCE34 Q34 Auditable with hash-chained audit trail

---

## Quick Start

```bash
cd /home/samuel/Primitives/kindly_dedup
./scripts/configure_dns.sh
```

---

## Prerequisites

### 1. Obtain Namecheap API Credentials (30 minutes)

**Step-by-step**:

1. **Log in to Namecheap**:
   - Go to: https://namecheap.com
   - Sign in with your account

2. **Enable API Access**:
   - Navigate to: **Profile** (top-right) → **Tools** → **API Access**
   - Or direct link: https://ap.www.namecheap.com/settings/tools/apiaccess/

3. **Generate API Key**:
   - Click **Enable API Access**
   - Accept the terms of service
   - Your API key will be generated (save this securely!)

4. **Whitelist Your IP Address**:
   - In the API Access page, add your current IP to the whitelist
   - Get your IP: `curl ifconfig.me` or `curl ipinfo.io/ip`
   - Add both your local IP and CI/CD runner IP (if applicable)

5. **Note Your Credentials**:
   - **API Username**: (shown on API Access page)
   - **API Key**: (generated above - SAVE THIS!)
   - **Account Username**: (your Namecheap login username)
   - **Client IP**: (the IP you whitelisted)

### 2. Create Credentials File (5 minutes)

**Option A: Let the script prompt you** (recommended for first-time setup)
- Run `./scripts/configure_dns.sh` and follow prompts
- Script will offer to save credentials to `~/.namecheap-credentials`

**Option B: Manual creation**
```bash
# Create credentials file
cat > ~/.namecheap-credentials <<'EOF'
# Namecheap API Credentials
# Generated: $(date)
# SECURITY: Do NOT commit this file to version control

export NAMECHEAP_API_USER='your-api-username'
export NAMECHEAP_API_KEY='your-api-key-here'
export NAMECHEAP_USERNAME='your-namecheap-account-username'
export NAMECHEAP_CLIENT_IP='your-whitelisted-ip'
EOF

# Secure the file (owner read-only)
chmod 600 ~/.namecheap-credentials

# Verify
cat ~/.namecheap-credentials
```

### 3. Choose CDN Provider

The script will prompt you to select a CDN provider:

#### Option 1: BunnyCDN (Recommended)
**Cost**: $1/month + $0.01/GB transfer
**Speed**: Global CDN with 114+ locations
**Setup**: https://bunny.net

**Steps**:
1. Sign up at https://bunny.net
2. Create a **Storage Zone**: `kindly-dedup-releases`
3. Create a **Pull Zone** linked to the storage zone
4. Note the pull zone URL: `<zone-name>.b-cdn.net`
5. Enable HTTPS (automatic via Bunny's SSL)

**Benefits**:
- Fastest setup (5 minutes)
- Lowest cost for small traffic
- Simple API (curl-based uploads)
- Built-in SSL certificate

#### Option 2: Cloudflare
**Cost**: Free tier available
**Speed**: Global CDN
**Setup**: https://cloudflare.com

**Steps**:
1. Add `kindly.software` domain to Cloudflare
2. Create R2 bucket for binary storage (or use Workers)
3. Set up custom domain: `dedup.kindly.software`
4. Note the endpoint URL

**Benefits**:
- Free tier generous
- DDoS protection included
- Advanced caching rules

#### Option 3: Fly.io CDN
**Cost**: Included with Fly.io platform
**Speed**: Integrated with existing deployment
**Setup**: Use existing Fly.io account

**Steps**:
1. Create Fly.io app: `flyctl apps create kindly-dedup-cdn`
2. Deploy static file server or Nginx
3. Map custom domain: `flyctl certs add dedup.kindly.software`
4. Endpoint: `kindly-dedup-cdn.fly.dev`

**Benefits**:
- Integrated with existing Fly.io services
- Single billing
- Easy deployment

---

## Running the DNS Configuration Script

### Interactive Mode (Recommended)

```bash
cd /home/samuel/Primitives/kindly_dedup
./scripts/configure_dns.sh
```

The script will:
1. ✅ Check for credentials in `~/.namecheap-credentials`
2. ✅ Prompt for credentials if not found (and offer to save)
3. ✅ Validate credentials with Namecheap API
4. ✅ Fetch existing DNS records for `kindly.software`
5. ✅ Prompt to select CDN provider (BunnyCDN, Cloudflare, Fly.io, or custom)
6. ✅ Create/update CNAME record: `dedup.kindly.software` → CDN endpoint
7. ✅ Log all changes to Q34 audit trail (`logs/dns_audit.log`)
8. ✅ Display next steps and verification commands

### Expected Output

```
╔════════════════════════════════════════════════════════════════╗
║  Namecheap DNS Configuration for dedup.kindly.software         ║
║  Sprint Day 1 - Distribution Infrastructure Setup              ║
╚════════════════════════════════════════════════════════════════╝

ℹ Loading credentials from ~/.namecheap-credentials
[AUDIT] INIT: DNS configuration started for dedup.kindly.software
ℹ Credentials validated

Choose CDN provider for binary distribution:
  1) BunnyCDN (recommended - $1/month + usage)
  2) Cloudflare (free tier available)
  3) Fly.io CDN (integrated with existing deployment)
  4) Custom CNAME target

Select option (1-4): 1

ℹ BunnyCDN selected. You'll need to:
  1. Create storage zone: kindly-dedup-releases
  2. Get pull zone URL: <zone-name>.b-cdn.net

Enter BunnyCDN pull zone URL (e.g., kindly-dedup.b-cdn.net): kindly-dedup.b-cdn.net

[AUDIT] CDN_SELECTION: Provider: BunnyCDN, Target: kindly-dedup.b-cdn.net
✓ CDN target validated: kindly-dedup.b-cdn.net

ℹ Fetching existing DNS records for kindly.software...
✓ Retrieved existing DNS records
[AUDIT] DNS_FETCH: Retrieved kindly.software records

ℹ Configuring CNAME record: dedup.kindly.software -> kindly-dedup.b-cdn.net
ℹ Building DNS update request...
✓ DNS record created/updated successfully!
[AUDIT] DNS_UPDATE: CNAME dedup.kindly.software -> kindly-dedup.b-cdn.net

╔════════════════════════════════════════════════════════════════╗
║  DNS Configuration Complete!                                   ║
╚════════════════════════════════════════════════════════════════╝

Configuration summary:
  Domain:        dedup.kindly.software
  Record Type:   CNAME
  Target:        kindly-dedup.b-cdn.net
  TTL:           1800 seconds (30 minutes)
  Provider:      BunnyCDN

Next steps:
  1. Wait for DNS propagation (typically 5-30 minutes)
  2. Verify DNS: dig dedup.kindly.software
  3. Set up SSL certificate (Let's Encrypt)
  4. Upload binary to CDN: scripts/upload_release.sh
  5. Test download: curl https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64

DNS propagation check:
  dig dedup.kindly.software             # Check propagation
  nslookup dedup.kindly.software        # Alternative check
  curl -I https://dedup.kindly.software # Test HTTPS (after SSL setup)

Audit trail saved to: logs/dns_audit.log
Verify hash chain: sha256sum logs/dns_audit.log

✓ Day 1 Task 1 Complete: DNS configured for dedup.kindly.software
```

---

## Verifying DNS Propagation

### Check DNS Record

```bash
# Using dig (recommended)
dig dedup.kindly.software

# Expected output:
# dedup.kindly.software. 1800 IN CNAME kindly-dedup.b-cdn.net.

# Using nslookup
nslookup dedup.kindly.software

# Expected output:
# dedup.kindly.software canonical name = kindly-dedup.b-cdn.net.

# Check propagation globally
curl https://www.whatsmydns.net/#CNAME/dedup.kindly.software
```

### Propagation Timing

| DNS Server | Typical Time |
|------------|--------------|
| Namecheap nameservers | 5-15 minutes |
| Global DNS propagation | 30 minutes - 24 hours |
| Local ISP DNS | 1-4 hours |

**Tip**: Use Google DNS (8.8.8.8) for faster propagation:
```bash
dig @8.8.8.8 dedup.kindly.software
```

---

## Q34 Audit Trail

The script creates a **hash-chained audit log** for compliance:

**Location**: `/home/samuel/Primitives/kindly_dedup/logs/dns_audit.log`

**Format**:
```
TIMESTAMP | ACTION | DETAILS | HASH:sha256(entry+prev_hash)
```

**Example**:
```
2025-11-18T14:23:45Z | INIT | DNS configuration started for dedup.kindly.software | HASH:a1b2c3...
2025-11-18T14:24:12Z | CDN_SELECTION | Provider: BunnyCDN, Target: kindly-dedup.b-cdn.net | HASH:d4e5f6...
2025-11-18T14:24:58Z | DNS_FETCH | Retrieved kindly.software records | HASH:g7h8i9...
2025-11-18T14:25:34Z | DNS_UPDATE | CNAME dedup.kindly.software -> kindly-dedup.b-cdn.net | HASH:j0k1l2...
2025-11-18T14:25:35Z | COMPLETE | DNS configuration finished successfully | HASH:m3n4o5...
```

**Verification**:
```bash
# Verify hash chain integrity
cat logs/dns_audit.log | while read line; do
    echo "$line" | awk -F'|' '{print $NF}' | sed 's/HASH://'
done | sha256sum
```

---

## Troubleshooting

### Issue: "API Authentication Failed"

**Cause**: Invalid API credentials or IP not whitelisted

**Solution**:
1. Verify credentials in `~/.namecheap-credentials`
2. Check whitelisted IP: https://ap.www.namecheap.com/settings/tools/apiaccess/
3. Get current IP: `curl ifconfig.me`
4. Re-run script with correct credentials

### Issue: "DNS Record Already Exists"

**Cause**: Subdomain `dedup.kindly.software` already configured

**Solution**:
- Script will prompt to update existing record
- Choose 'y' to overwrite with new CDN target
- Or manually delete record in Namecheap dashboard first

### Issue: "DNS Not Propagating"

**Cause**: DNS caching or TTL not expired

**Solution**:
```bash
# Flush local DNS cache (Linux)
sudo systemd-resolve --flush-caches

# Check authoritative nameservers directly
dig @dns1.registrar-servers.com dedup.kindly.software

# Wait and retry (TTL is 1800 seconds = 30 minutes)
```

### Issue: "HTTPS Not Working After DNS Setup"

**Cause**: SSL certificate not configured yet

**Solution**:
- BunnyCDN: Automatic SSL (enable in dashboard)
- Cloudflare: Automatic SSL (enable Universal SSL)
- Fly.io: Run `flyctl certs add dedup.kindly.software`
- Let's Encrypt manual: See Day 1 Task 2 in LAUNCH_SPRINT_PLAN.md

---

## Security Best Practices

### Credential Storage

✅ **DO**:
- Store credentials in `~/.namecheap-credentials` with mode 600
- Use CI/CD secrets for automated deployments
- Rotate API keys every 90 days

❌ **DON'T**:
- Commit credentials to version control
- Share API keys in Slack/email
- Use production API keys in development

### API Key Protection

```bash
# Secure credentials file
chmod 600 ~/.namecheap-credentials

# Verify permissions
ls -la ~/.namecheap-credentials
# Expected: -rw------- 1 samuel samuel (owner read/write only)

# Add to .gitignore (project root)
echo ".namecheap-credentials" >> .gitignore
echo "logs/dns_audit.log" >> .gitignore  # Optional (or commit audit trail)
```

---

## Next Steps After DNS Configuration

1. **Verify DNS Propagation** (5-30 minutes)
   ```bash
   dig dedup.kindly.software
   ```

2. **Set Up CDN** (Day 1 Task 2)
   - BunnyCDN: Create storage zone, configure pull zone
   - Cloudflare: Create R2 bucket or Workers script
   - Fly.io: Deploy static file server

3. **Configure SSL Certificate** (Day 1 Task 3)
   - BunnyCDN: Auto-SSL in dashboard
   - Cloudflare: Universal SSL (automatic)
   - Fly.io: `flyctl certs add dedup.kindly.software`

4. **Upload Binary** (Day 2)
   - Run: `scripts/upload_release.sh v2.0.0`
   - Sign binary with GPG
   - Upload to CDN storage zone

5. **Test Download** (Day 2 Validation)
   ```bash
   curl -O https://dedup.kindly.software/latest/kindly_dedup-linux-x86_64
   gpg --verify kindly_dedup-linux-x86_64.asc
   ```

---

## References

- **Namecheap API Docs**: https://www.namecheap.com/support/api/intro/
- **DNS Record Types**: https://www.namecheap.com/support/knowledgebase/article.aspx/579/2237/which-record-type-should-i-choose-for-the-dns-settings
- **BunnyCDN Setup**: https://docs.bunny.net/docs/stream-getting-started
- **Cloudflare DNS**: https://developers.cloudflare.com/dns/
- **Fly.io Custom Domains**: https://fly.io/docs/networking/custom-domain/

---

## Summary Checklist

- [ ] Namecheap API credentials obtained
- [ ] `~/.namecheap-credentials` file created (mode 600)
- [ ] CDN provider selected (BunnyCDN/Cloudflare/Fly.io)
- [ ] CDN endpoint URL noted
- [ ] `./scripts/configure_dns.sh` executed successfully
- [ ] DNS record created: `dedup.kindly.software CNAME <cdn-endpoint>`
- [ ] DNS propagation verified: `dig dedup.kindly.software`
- [ ] Audit trail generated: `logs/dns_audit.log`
- [ ] SSL certificate configured (CDN provider dashboard)
- [ ] HTTPS working: `curl -I https://dedup.kindly.software`

**Sprint Progress**: Day 1 Task 1 of 10 Complete ✅
