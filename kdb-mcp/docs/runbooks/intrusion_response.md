# Runbook: Intrusion Detection Response

**Purpose**: Respond to intrusion detection alerts from `IntrusionDetectorCapsule`
**Trigger**: IP blocked by Bloom filter (>100 failed authentication attempts)
**Priority**: P1 (High - Potential security incident)
**Owner**: Security Operations Center (SOC)

---

## Trigger Conditions

1. **IntrusionDetectorCapsule** blocks IP address (105ns check failed)
2. **Audit log** shows repeated authentication failures from same IP
3. **Zero-trust policy** escalates risk score >65535 (100%)
4. **Alert notification** sent to security@atomic-mcp.com

**Example Alert**:
```
[2025-11-15 14:32:17 UTC] INTRUSION DETECTED
IP: 192.168.1.100
Failed Attempts: 127 (last 5 minutes)
Blocked By: IntrusionDetectorCapsule (Bloom filter)
Risk Score: 65535 (100% - CRITICAL)
Action: IP_BLOCKED
Audit Hash: CRC64(0x1234567890abcdef)
```

---

## Prerequisites

- **Access**: SSH to atomic_mcp_server (samuel@192.168.0.38)
- **Tools**: jq, sqlite3, curl, atomic_mcp_audit_viewer
- **Permissions**: sudo (for firewall rules, audit log access)
- **Documentation**: [SECURITY.md](../../SECURITY.md), [THREAT_MODEL.md](../../THREAT_MODEL.md)

---

## Step-by-Step Procedure

### Step 1: Verify Alert (Est. Time: 2 minutes)

**Objective**: Confirm intrusion detection is legitimate (not false positive)

```bash
# SSH to server
ssh samuel@192.168.0.38

# Query audit log for IP address
cargo run --release --bin audit_viewer -- \
  --audit-dir /var/log/atomic_mcp \
  --filter-ip 192.168.1.100 \
  --last 1h

# Expected Output:
# ┌────────────────────┬──────────────┬──────────┬────────────┬────────────┐
# │ Timestamp          │ Operation    │ Severity │ Session ID │ Risk Score │
# ├────────────────────┼──────────────┼──────────┼────────────┼────────────┤
# │ 2025-11-15 14:30:00│ AUTH_FAILED  │ ERROR    │ N/A        │ 25000      │
# │ 2025-11-15 14:30:15│ AUTH_FAILED  │ ERROR    │ N/A        │ 30000      │
# │ 2025-11-15 14:30:30│ AUTH_FAILED  │ ERROR    │ N/A        │ 40000      │
# │ ... (127 total)    │              │          │            │            │
# │ 2025-11-15 14:32:17│ IP_BLOCKED   │ CRITICAL │ N/A        │ 65535      │
# └────────────────────┴──────────────┴──────────┴────────────┴────────────┘
```

**Validation**:
- ✅ >100 AUTH_FAILED events in <5 minutes → **Legitimate brute-force attack**
- ✅ Progressive risk score increase (25000 → 65535) → **Intrusion detector working**
- ❌ <10 AUTH_FAILED events → **Possible false positive** (investigate further)

---

### Step 2: Assess Threat Severity (Est. Time: 3 minutes)

**Objective**: Determine if this is opportunistic scan or targeted attack

#### 2.1 Check IP Reputation

```bash
# Whois lookup
whois 192.168.1.100 | head -20

# Check IP geolocation
curl -s "https://ipapi.co/192.168.1.100/json/" | jq '.'

# Expected Output:
# {
#   "ip": "192.168.1.100",
#   "city": "Unknown",
#   "region": "Unknown",
#   "country": "ZZ",  # ⚠️ Unknown country code = suspicious
#   "org": "AS15169 Google LLC",  # ⚠️ VPN/cloud provider = medium risk
#   "timezone": "America/New_York"
# }
```

#### 2.2 Check Attack Pattern

```bash
# Analyze failed authentication attempts
cargo run --release --bin audit_viewer -- \
  --audit-dir /var/log/atomic_mcp \
  --filter-ip 192.168.1.100 \
  --operation AUTH_FAILED \
  --json | jq '[.[] | {time: .timestamp, token: .details.token}]'

# Expected Patterns:
# [
#   {"time": "14:30:00", "token": "admin"},       # ⚠️ Common username
#   {"time": "14:30:01", "token": "root"},        # ⚠️ Common username
#   {"time": "14:30:02", "token": "test"},        # ⚠️ Dictionary attack
#   ...
# ]
```

**Threat Classification**:
| Pattern | Severity | Response |
|---------|----------|----------|
| **Dictionary usernames** (admin, root, test) | P1 (High) | Block 24h + monitor |
| **Sequential PIDs** (1, 2, 3, ...) | P1 (High) | Block 7d + alert security |
| **Random tokens** (gibberish) | P2 (Medium) | Block 24h |
| **Single username** (repeated) | P3 (Low) | Block 1h (possible typo) |

---

### Step 3: Block IP at Firewall (Est. Time: 2 minutes)

**Objective**: Prevent attacker from reaching atomic_mcp_server (defense-in-depth)

```bash
# Add iptables rule (immediate effect)
sudo iptables -I INPUT -s 192.168.1.100 -j DROP

# Verify rule
sudo iptables -L INPUT -n | grep 192.168.1.100

# Expected Output:
# DROP       all  --  192.168.1.100        0.0.0.0/0

# Make rule persistent (survives reboot)
sudo iptables-save > /etc/iptables/rules.v4

# Alternative: Use ufw (Ubuntu)
sudo ufw deny from 192.168.1.100
sudo ufw status numbered
```

**Validation**:
- ✅ iptables rule added → **Firewall blocking IP**
- ✅ ufw status shows rule → **Persistent across reboots**

---

### Step 4: Notify Security Team (Est. Time: 1 minute)

**Objective**: Alert security team for investigation and tracking

```bash
# Send email notification
cat <<EOF | mail -s "[P1] Intrusion Detected: 192.168.1.100" security@atomic-mcp.com
IP Address: 192.168.1.100
Detection Time: $(date -u)
Failed Attempts: 127
Blocked By: IntrusionDetectorCapsule + iptables
Risk Score: 65535 (100%)
Threat Level: HIGH (dictionary attack)

Recommended Actions:
1. Review audit logs for lateral movement attempts
2. Check for similar IPs in same subnet (192.168.1.0/24)
3. Monitor for IP rotation or distributed attack

Audit Log:
$(cargo run --release --bin audit_viewer -- --audit-dir /var/log/atomic_mcp --filter-ip 192.168.1.100 --last 1h)

Runbook: docs/runbooks/intrusion_response.md
EOF
```

**Validation**:
- ✅ Email sent → **Security team notified**
- ✅ Ticket created in incident tracking system (Jira, PagerDuty)

---

### Step 5: Check for Lateral Movement (Est. Time: 5 minutes)

**Objective**: Ensure attacker hasn't compromised other accounts or IPs

#### 5.1 Check Same Subnet

```bash
# Find all IPs in 192.168.1.0/24 subnet
cargo run --release --bin audit_viewer -- \
  --audit-dir /var/log/atomic_mcp \
  --last 24h \
  --json | jq '[.[] | select(.client_ip | startswith("192.168.1.")) | .client_ip] | unique'

# Expected Output:
# [
#   "192.168.1.100",  # ⚠️ Blocked attacker
#   "192.168.1.101",  # ⚠️ Possible attacker (same subnet)
#   "192.168.1.200"   # ✅ Legitimate user
# ]
```

#### 5.2 Check Anomaly Scores

```bash
# Find high-risk sessions (risk score >25600)
cargo run --release --bin audit_viewer -- \
  --audit-dir /var/log/atomic_mcp \
  --last 24h \
  --min-risk-score 25600 \
  --json | jq '[.[] | {ip: .client_ip, risk: .risk_score, operation: .operation}]'

# Expected Output:
# [
#   {"ip": "192.168.1.100", "risk": 65535, "operation": "IP_BLOCKED"},  # ⚠️ Blocked
#   {"ip": "10.0.0.50", "risk": 30000, "operation": "ZERO_TRUST_MONITOR"}  # ⚠️ Investigate
# ]
```

**Action**:
- **If 10.0.0.50 shows high risk** → Review zero-trust policy logs, consider blocking
- **If multiple IPs in same subnet** → Block entire subnet (e.g., 192.168.1.0/24)

---

### Step 6: Review Zero-Trust Policy Logs (Est. Time: 3 minutes)

**Objective**: Check if zero-trust policy escalated any high-risk requests

```bash
# Query zero-trust MONITOR events
cargo run --release --bin audit_viewer -- \
  --audit-dir /var/log/atomic_mcp \
  --operation ZERO_TRUST_MONITOR \
  --last 24h

# Expected Output:
# ┌────────────────────┬────────────────────────┬──────────┬────────────┐
# │ Timestamp          │ Operation              │ Risk     │ IP         │
# ├────────────────────┼────────────────────────┼──────────┼────────────┤
# │ 2025-11-15 10:00:00│ ZERO_TRUST_MONITOR     │ 15000    │ 10.0.0.50  │
# │ 2025-11-15 12:00:00│ ZERO_TRUST_MONITOR     │ 20000    │ 10.0.0.50  │
# │ 2025-11-15 14:00:00│ ZERO_TRUST_MONITOR     │ 30000    │ 10.0.0.50  │
# └────────────────────┴────────────────────────┴──────────┴────────────┘
```

**Escalation Criteria**:
| Risk Score | Action | Reasoning |
|------------|--------|-----------|
| **<6400** (10%) | Allow | Normal behavior |
| **6400-25600** (10-40%) | Monitor | Elevated risk, track |
| **>25600** (40%+) | Block | High risk, reject |

**Action**:
- **If 10.0.0.50 shows increasing risk** → Investigate user behavior, consider TOTP re-prompt
- **If legitimate user** → Whitelist IP in intrusion detector (bypass Bloom filter)

---

### Step 7: Document Incident (Est. Time: 2 minutes)

**Objective**: Record incident for future analysis and compliance

```bash
# Export audit log for incident (7-year SOX retention)
cargo run --release --bin audit_viewer -- \
  --audit-dir /var/log/atomic_mcp \
  --filter-ip 192.168.1.100 \
  --output incident_$(date +%Y%m%d_%H%M%S).json

# Upload to S3 (immutable storage)
aws s3 cp incident_*.json s3://atomic-mcp-audit-archive/incidents/ \
  --storage-class GLACIER

# Create incident ticket
cat <<EOF > incident_report.md
# Incident Report: Brute-Force Attack (192.168.1.100)

**Date**: $(date -u)
**Severity**: P1 (High)
**Attacker IP**: 192.168.1.100
**Detection**: IntrusionDetectorCapsule (Bloom filter)
**Failed Attempts**: 127 in 5 minutes
**Block Duration**: 24 hours (2025-11-16 14:32:17 UTC)

## Timeline
- 14:30:00 UTC: First failed auth attempt
- 14:32:17 UTC: IP blocked after 127 attempts
- 14:35:00 UTC: Firewall rule added (iptables)
- 14:40:00 UTC: Security team notified

## Analysis
- **Attack Type**: Dictionary attack (admin, root, test usernames)
- **Threat Level**: Medium (opportunistic scan, not targeted)
- **Lateral Movement**: None detected (isolated IP)

## Mitigations Applied
- ✅ IP blocked by IntrusionDetectorCapsule (105ns)
- ✅ Firewall rule added (iptables DROP)
- ✅ Audit log exported to S3 (7-year retention)
- ✅ Security team notified

## Lessons Learned
- Intrusion detector effective (detected after 100 attempts)
- Zero-trust policy prevented escalation
- Consider adding IDS/IPS (Snort, Suricata) for pattern matching

## Action Items
- [ ] Monitor for IP rotation (same attacker, different IPs)
- [ ] Review subnet 192.168.1.0/24 for additional IPs
- [ ] Add Snort rule for dictionary attack pattern
- [ ] Update firewall to block entire subnet if >3 IPs blocked
EOF

# Attach to incident ticket (Jira, PagerDuty, etc.)
jira create-issue --project SEC --summary "Intrusion: 192.168.1.100" --description "$(cat incident_report.md)"
```

---

### Step 8: Monitor for Recurrence (Est. Time: Ongoing)

**Objective**: Ensure attacker doesn't return with different IP

```bash
# Set up monitoring alert (PagerDuty, Datadog, etc.)
# Alert if >50 failed auth attempts from any IP in 5 minutes

# Example: Datadog monitor (JSON config)
cat <<EOF > datadog_intrusion_monitor.json
{
  "name": "Intrusion Detection: High Failed Auth Attempts",
  "type": "metric alert",
  "query": "sum(last_5m):sum:atomic_mcp.auth.failed{*} by {client_ip} > 50",
  "message": "@security-oncall Intrusion detected: {{client_ip}} has {{value}} failed auth attempts in 5 minutes. Runbook: docs/runbooks/intrusion_response.md",
  "tags": ["security", "intrusion", "p1"],
  "priority": 1
}
EOF

# Create monitor
datadog-cli monitor create --config datadog_intrusion_monitor.json
```

---

## Validation Checklist

After completing all steps, verify:

- [ ] **Alert verified**: Audit log shows >100 failed attempts (Step 1)
- [ ] **Threat assessed**: IP reputation checked, attack pattern identified (Step 2)
- [ ] **IP blocked**: iptables/ufw rule added and persistent (Step 3)
- [ ] **Team notified**: Email sent to security@atomic-mcp.com (Step 4)
- [ ] **Lateral movement checked**: No similar IPs in subnet (Step 5)
- [ ] **Zero-trust reviewed**: High-risk sessions investigated (Step 6)
- [ ] **Incident documented**: Report exported to S3 + ticket created (Step 7)
- [ ] **Monitoring enabled**: Alert configured for future incidents (Step 8)

**Success Criteria**: IP blocked at firewall + IntrusionDetectorCapsule + 24-hour TTL

---

## Escalation Criteria

Escalate to **Security Lead** if:
1. **Distributed attack**: >10 IPs from different subnets
2. **Targeted attack**: Attacker knows valid usernames/PIDs
3. **Insider threat**: Attacker uses valid credentials with unusual behavior
4. **Zero-day exploit**: Bypass 18-capsule authentication
5. **Data exfiltration**: Audit log shows successful PID access + large data transfer

**Escalation Contact**: security-lead@atomic-mcp.com (PagerDuty P0 alert)

---

## Post-Incident Review

Within 48 hours, conduct post-incident review:
1. **Root Cause**: Why did attacker target atomic_mcp_server?
2. **Detection Effectiveness**: Did intrusion detector catch it fast enough?
3. **Response Time**: How long did it take to block IP + notify team?
4. **Improvements**: What can we do to prevent future incidents?

**Example Improvements**:
- Add IDS/IPS (Snort, Suricata) for pattern matching
- Implement WebAuthn (FIDO2) to prevent credential theft
- Add geo-blocking (block IPs from high-risk countries)
- Increase Bloom filter size (8KB → 16KB for more IPs)

---

**Runbook Version**: 1.0
**Last Updated**: 2025-11-15
**Owner**: Security Operations Center (SOC)
**Review Frequency**: Quarterly (every 3 months)
