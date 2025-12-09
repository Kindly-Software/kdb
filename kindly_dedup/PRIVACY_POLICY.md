# PRIVACY POLICY

**kindly_dedup - LLM Training Dataset Deduplication Software**

**Version:** 1.0
**Effective Date:** [DATE]
**Last Updated:** [DATE]

---

## 1. INTRODUCTION

This Privacy Policy describes how kindly_dedup ("Software", "we", "our") processes data when you use our LLM training dataset deduplication software.

**KEY PRINCIPLE: ALL DATA PROCESSING IS LOCAL. WE DO NOT UPLOAD YOUR DATA TO THE CLOUD.**

This policy is designed to comply with:
- **GDPR** (General Data Protection Regulation - EU)
- **CCPA** (California Consumer Privacy Act - USA)
- **SOX** (Sarbanes-Oxley Act - Financial compliance)
- **SOC2** (Service Organization Control 2 - Security)
- **HIPAA** (Health Insurance Portability and Accountability Act - if applicable)

---

## 2. DATA PROCESSING OVERVIEW

### 2.1 What We Process

The Software processes:
- **Your Documents**: Text documents you provide for deduplication (e.g., web pages, articles, training data)
- **MinHash Signatures**: Compact fingerprints derived from your documents (128 bytes per document)
- **LSH Buckets**: Index structures for finding similar documents
- **Duplicate Clusters**: Groups of similar documents identified by the software
- **Audit Trails**: Hash-chained logs of operations for compliance (Q34 compliance)

### 2.2 What We DO NOT Process

The Software does NOT process:
- Personal data (names, email addresses, phone numbers) **unless present in your input documents**
- Payment information (handled by third-party processors only)
- Biometric data
- Health information (unless present in your input documents)

### 2.3 Processing Location

**100% LOCAL PROCESSING.** All data processing occurs:
- **On your hardware** (your servers, workstations, or cloud instances you control)
- **Never on our servers** (we do not operate cloud infrastructure for data processing)
- **Never transmitted** to us or third parties (except as described in Section 4)

---

## 3. LOCAL-ONLY ARCHITECTURE

### 3.1 No Cloud Upload

The Software is designed with a **local-first architecture**:

```
┌─────────────────────────────────────────────┐
│  YOUR HARDWARE                              │
│  ┌─────────────┐                            │
│  │ Documents   │ ──┐                        │
│  └─────────────┘   │                        │
│                    ▼                        │
│  ┌───────────────────────────────────────┐ │
│  │ kindly_dedup Processing Pipeline      │ │
│  │ • Tokenization                        │ │
│  │ • MinHash signatures                  │ │
│  │ • LSH bucketing                       │ │
│  │ • Duplicate detection                 │ │
│  │ • Union-Find clustering               │ │
│  └───────────────────────────────────────┘ │
│                    │                        │
│                    ▼                        │
│  ┌─────────────────────────────────────┐   │
│  │ Local Storage (mmap files)          │   │
│  │ • Signatures.mmap                   │   │
│  │ • LSH_buckets.mmap                  │   │
│  │ • Audit_trail.jsonl                 │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  NO INTERNET CONNECTION REQUIRED            │
└─────────────────────────────────────────────┘
```

### 3.2 Offline Capability

The Software:
- **Works fully offline** (no internet connection required for core functionality)
- **Stores all data locally** (mmap files on your disk)
- **Generates results locally** (duplicate clusters, signatures, audit trails)

**Exception:** License verification may require internet connection on first activation (one-time only).

### 3.3 Data Ownership

You retain **100% ownership** of:
- Your input documents
- Generated MinHash signatures
- LSH bucket indices
- Duplicate detection results
- Audit trail logs

We have **ZERO access** to your data.

---

## 4. DATA COLLECTION (Minimal by Design)

### 4.1 What We Collect

The Software collects **MINIMAL data** for license verification and optional analytics:

| Data Type | Purpose | Collection | Storage | Transmission |
|-----------|---------|------------|---------|--------------|
| **Hardware Fingerprint** | License binding (PUF) | Automatic | Local only | One-time (activation) |
| **License Key** | Tier verification | User-provided | Local only | One-time (activation) |
| **Document Count** | Tier limit enforcement | Automatic | Local only | Never |
| **Audit Trails** | Q34 compliance | Automatic | Local only | Never |
| **Telemetry** (opt-in) | Performance metrics | Opt-in only | Local cache | Opt-in only |

### 4.2 Hardware Fingerprint (PUF)

The Software uses **Physical Unclonable Function (PUF)** to:
- Derive a unique fingerprint from your CPU's physical characteristics
- Bind your license to specific hardware (prevent unauthorized transfers)
- Verify license tier limits (document count)

**Privacy Properties:**
- **Generated locally** (never transmitted except during activation)
- **Cannot be reversed** to identify you personally
- **Stored locally only** after activation (in encrypted license file)
- **Not linked to personal data** (name, email, address)

**Activation Process:**
1. You purchase a license (via third-party payment processor)
2. You receive a license key (via email)
3. First run: Software generates hardware fingerprint locally
4. Software sends: `{license_key: "XXX", hardware_fingerprint: "YYY"}` to activation server
5. Activation server validates and returns: `{status: "activated"}`
6. Software stores encrypted license locally
7. **No further internet connection required** (all subsequent verifications are local)

### 4.3 Document Metadata

The Software processes:
- **Document ID** (integer index, e.g., 0, 1, 2, ...)
- **Document text** (your content)
- **MinHash signature** (128-byte fingerprint derived from text)

**What we DO NOT collect:**
- Document content (never transmitted)
- Document metadata (titles, authors, timestamps)
- Original file paths or filenames

### 4.4 Audit Trails (Q34 Compliance)

The Software generates cryptographic audit trails for compliance (SOX, SOC2, GDPR, HIPAA):

```json
{
  "timestamp": "2025-11-25T12:34:56.789Z",
  "operation": "add_document",
  "document_id": 12345,
  "signature_hash": "sha256:abc123...",
  "previous_hash": "sha256:def456...",
  "chain_hash": "sha256:ghi789..."
}
```

**Privacy Properties:**
- **No document content** (only operation type and hash)
- **No personal data** (only document IDs)
- **Stored locally only** (never transmitted)
- **Tamper-evident** (hash-chained for integrity)

You can:
- Disable audit trails (set `audit_trail: false` in config)
- Delete audit logs at any time
- Export audit logs for compliance purposes

---

## 5. TELEMETRY (OPT-IN ONLY)

### 5.1 Default Behavior

By default, the Software collects **ZERO telemetry**:
- No usage statistics
- No performance metrics
- No error reports
- No crash dumps

### 5.2 Opt-In Analytics

If you **explicitly enable** telemetry (disabled by default), the Software may collect:

| Metric | Example | Purpose |
|--------|---------|---------|
| **Throughput** | 60,000 docs/sec | Performance optimization |
| **Hardware** | "AMD Ryzen 9, 64GB RAM" | Hardware compatibility |
| **Tier Usage** | "Pro tier, 1M docs" | Product analytics |
| **Errors** | "Out of memory at 10M docs" | Bug fixes |

**What we DO NOT collect even with opt-in:**
- Document content or text
- Document metadata (titles, authors)
- Personal data (names, emails, IPs)
- Full system information (OS version, installed software)

### 5.3 How to Enable/Disable Telemetry

```bash
# Disable telemetry (default)
kindly_dedup --telemetry=false

# Enable telemetry (opt-in)
kindly_dedup --telemetry=true

# Check current setting
kindly_dedup --show-telemetry-status
```

**Note:** Telemetry setting is stored locally in `~/.kindly_dedup/config.toml` and can be changed at any time.

### 5.4 Data Retention (Telemetry)

If telemetry is enabled:
- **Local cache:** 30 days (then auto-deleted)
- **Transmission frequency:** Weekly batch (if internet available)
- **Server retention:** 90 days (aggregated statistics only)
- **Deletion:** You can request deletion at any time

---

## 6. LICENSE VERIFICATION

### 6.1 How It Works

The Software verifies your license tier to enforce document limits:

| Tier | Limit | Verification Method |
|------|-------|---------------------|
| **Demo** | 1,000 docs | Local counter (no server check) |
| **Basic** | 100,000 docs | Local counter (no server check) |
| **Pro** | 10,000,000 docs | Local counter (no server check) |
| **Enterprise** | Unlimited | Local counter (no server check) |

**Verification Process:**
1. **Activation** (one-time, requires internet):
   - You enter license key
   - Software generates hardware fingerprint (PUF)
   - Software sends `{license_key, hardware_fingerprint}` to activation server
   - Server validates and returns tier information
   - Software stores encrypted license locally
2. **Runtime** (offline, local only):
   - Software loads license from local file
   - Software verifies hardware fingerprint matches (PUF check)
   - Software enforces document limit (local counter)
   - **No server communication**

### 6.2 What We Store

**On activation server** (one-time):
- License key (hashed)
- Hardware fingerprint (hashed)
- Activation timestamp
- Tier purchased

**Locally on your machine**:
- Encrypted license file (`~/.kindly_dedup/license.enc`)
- Hardware fingerprint (for verification)
- Current document count (for tier limit enforcement)

**What we DO NOT store:**
- Your personal information (name, email, address) after activation
- Your document content
- Your processing history

### 6.3 Data Transmission (Activation Only)

**One-time transmission** (HTTPS encrypted):
```json
{
  "license_key": "XXXX-XXXX-XXXX-XXXX",
  "hardware_fingerprint": "sha256:abc123...",
  "software_version": "3.0.0"
}
```

**Server response:**
```json
{
  "status": "activated",
  "tier": "Pro",
  "document_limit": 10000000,
  "expires": "2026-11-25"
}
```

**After activation:** No further transmission (all verifications are local).

### 6.4 License Renewal

Annual renewals:
- **Automatic renewal** (via payment processor, no data transmission to us)
- **Manual renewal** (purchase new license key, re-activate)

You can disable automatic renewal at any time via the payment processor's portal.

---

## 7. THIRD-PARTY SERVICES

### 7.1 Payment Processing

We use **[PAYMENT PROCESSOR NAME]** (e.g., Stripe, PayPal) to process payments. Their privacy policies apply:
- Stripe: https://stripe.com/privacy
- PayPal: https://www.paypal.com/privacy

**What we share with payment processor:**
- Your email (for license delivery)
- Payment amount
- Billing address (if required)

**What we DO NOT share:**
- Document content or metadata
- Hardware fingerprint
- Processing history

### 7.2 Activation Server

Our activation server (operated by us or trusted third-party):
- **Purpose:** License validation (one-time)
- **Data collected:** License key (hashed), hardware fingerprint (hashed), tier
- **Data retention:** 5 years (for audit/fraud prevention)
- **Location:** [SERVER LOCATION, e.g., "USA (AWS us-east-1)"]
- **Security:** TLS 1.3 encryption, SOC2 Type II certified

### 7.3 Analytics (Opt-In Only)

If telemetry is enabled, we may use **[ANALYTICS SERVICE]** (e.g., Plausible, self-hosted):
- **Purpose:** Aggregated usage statistics
- **Data collected:** Hardware specs, throughput metrics, error types
- **Data retention:** 90 days
- **Privacy:** No personal data, no tracking cookies, GDPR compliant

### 7.4 No Other Third Parties

We do NOT share your data with:
- Advertising networks
- Data brokers
- Social media platforms
- Government agencies (except where legally required)

---

## 8. DATA SECURITY

### 8.1 Encryption

**In Transit:**
- License activation: TLS 1.3 (HTTPS)
- Telemetry (opt-in): TLS 1.3 (HTTPS)

**At Rest:**
- License file: AES-256 encryption (local)
- Your data: **Your responsibility** (we recommend full-disk encryption)

**Note:** Your document data is stored in plaintext mmap files locally. You should:
- Enable full-disk encryption (e.g., BitLocker, FileVault, LUKS)
- Set appropriate file permissions (chmod 600)
- Store data on encrypted volumes

### 8.2 Access Control

**Who has access to your data:**
- **You:** Full access (you control the files)
- **Us:** **ZERO access** (we cannot see your data)

**Who has access to activation server data:**
- **Us:** Minimal access (license keys, hardware fingerprints, tiers)
- **Third parties:** ZERO access (no sharing)

### 8.3 Data Breach Protocol

If our activation server is breached:
1. **Immediate notification** (within 72 hours, GDPR requirement)
2. **Impact assessment** (what data was accessed)
3. **Remediation** (password resets, key rotation)
4. **Regulatory notification** (if required by law)

**Your data is NOT at risk** because:
- Your documents are stored locally (not on our servers)
- Hardware fingerprints are hashed (cannot be reversed)
- License keys are hashed (cannot be used directly)

### 8.4 Your Responsibilities

You are responsible for:
- Securing your local machine (firewall, antivirus, updates)
- Encrypting your data at rest (full-disk encryption)
- Managing access control (user permissions, file permissions)
- Backing up your data (we are not responsible for data loss)

---

## 9. YOUR RIGHTS (GDPR/CCPA)

### 9.1 Right to Access

You have the right to request:
- What data we hold about you (license key, hardware fingerprint, tier)
- How we use your data (license verification only)

**How to request:** Email [EMAIL] with subject "Data Access Request"

### 9.2 Right to Rectification

If your data is inaccurate (e.g., wrong tier assigned), you can request correction.

**How to request:** Email [EMAIL] with subject "Data Correction Request"

### 9.3 Right to Erasure ("Right to be Forgotten")

You can request deletion of:
- Your license key (will deactivate your license)
- Your hardware fingerprint (will require re-activation)
- Your telemetry data (if opt-in enabled)

**How to request:** Email [EMAIL] with subject "Data Deletion Request"

**Note:** We may retain minimal data for legal compliance (e.g., financial records for tax purposes).

### 9.4 Right to Data Portability

You can request:
- Export of your license data (JSON format)
- Export of your telemetry data (if opt-in enabled)

**How to request:** Email [EMAIL] with subject "Data Export Request"

### 9.5 Right to Object

You can object to:
- Telemetry collection (disable in settings)
- Marketing emails (unsubscribe link)
- License verification (will prevent software use)

### 9.6 Right to Withdraw Consent

You can withdraw consent for:
- Telemetry (disable in settings, takes effect immediately)
- Marketing emails (unsubscribe, takes effect within 10 days)

### 9.7 Response Time

We will respond to all requests within:
- **30 days** (GDPR requirement)
- **45 days** (CCPA requirement, extendable to 90 days if complex)

---

## 10. CHILDREN'S PRIVACY

The Software is NOT intended for children under 13 (USA) or 16 (EU). We do not knowingly collect data from children.

If you believe we have collected data from a child, contact us immediately at [EMAIL].

---

## 11. INTERNATIONAL DATA TRANSFERS

### 11.1 Your Data Stays Local

Since all data processing is local, **no international data transfers occur** for your documents.

### 11.2 Activation Server (One-Time Transfer)

If you are located outside [SERVER LOCATION]:
- License activation data (license key, hardware fingerprint) may be transferred to [SERVER LOCATION]
- Transfer is protected by:
  - **Standard Contractual Clauses (SCCs)** (GDPR Article 46)
  - **TLS 1.3 encryption**
  - **SOC2 Type II certification**

### 11.3 Your Rights

If you are an EU resident and your data is transferred outside the EU:
- You have the right to object (see Section 9.5)
- You can request details of safeguards (see Section 9.1)

---

## 12. DATA RETENTION

| Data Type | Retention Period | Deletion |
|-----------|------------------|----------|
| **Your Documents** | Indefinite (you control) | Delete files manually |
| **License Activation** | 5 years (fraud prevention) | Request deletion (Section 9.3) |
| **Telemetry** (opt-in) | 90 days (aggregated stats) | Auto-deleted or on request |
| **Payment Records** | 7 years (tax law) | Cannot be deleted (legal requirement) |
| **Audit Trails** | Indefinite (you control) | Delete files manually |

---

## 13. COMPLIANCE CERTIFICATIONS

The Software is designed to support compliance with:

| Standard | Compliance Feature | Your Responsibility |
|----------|-------------------|---------------------|
| **GDPR** | Local processing, minimal data collection, user rights | Data controller obligations (consent, notices) |
| **CCPA** | No sale of data, opt-out rights, data portability | Privacy notices, consumer requests |
| **SOX** | Q34 audit trails (hash-chained logs) | Implement controls, retain logs |
| **SOC2** | Cryptographic integrity, tamper detection | Access control, monitoring |
| **HIPAA** | Local processing (no PHI transmission) | BAA required, encryption at rest |

**Note:** We provide **tools** for compliance. You are responsible for implementing appropriate controls and policies.

---

## 14. CHANGES TO THIS POLICY

### 14.1 Notification

We may update this Privacy Policy. Changes will be notified via:
- **Email** (to license holders)
- **In-app notification** (on software update)
- **Website** (posted 30 days before effective date)

### 14.2 Material Changes

For material changes (e.g., new data collection, third-party sharing):
- **60 days advance notice**
- **Opt-in consent required** (you can decline and continue using old version)

### 14.3 Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | [DATE] | Initial release |

---

## 15. CONTACT INFORMATION

For privacy questions or requests:

**Data Controller:**
[COMPANY NAME]
Email: [EMAIL]
Address: [ADDRESS]

**Data Protection Officer (if applicable):**
Email: [DPO_EMAIL]

**Response Time:** Within 30 days (GDPR) or 45 days (CCPA)

---

## 16. COMPLAINTS

### 16.1 Internal Complaints

If you have a privacy concern:
1. Email [EMAIL] with subject "Privacy Complaint"
2. We will respond within 30 days
3. We will investigate and resolve the issue

### 16.2 Regulatory Complaints

You have the right to lodge a complaint with:
- **EU:** Your local Data Protection Authority (https://edpb.europa.eu/about-edpb/board/members_en)
- **USA:** Federal Trade Commission (https://www.ftc.gov/complaint)
- **California:** California Attorney General (https://oag.ca.gov/privacy)

---

## 17. ACKNOWLEDGMENT

BY USING THIS SOFTWARE, YOU ACKNOWLEDGE THAT:
- You have read and understand this Privacy Policy
- You consent to the data processing described herein
- You understand your rights under GDPR/CCPA
- You understand the Software's local-only architecture

**Version:** 1.0
**Last Updated:** [DATE]

---

## APPENDIX: TECHNICAL DETAILS

### Data Flow Diagram

```
┌─────────────────────────────────────────────────────┐
│  YOUR LOCAL MACHINE                                 │
│                                                     │
│  1. Documents (Input) ──┐                          │
│                         │                          │
│                         ▼                          │
│  2. kindly_dedup Processing (100% Local)           │
│     • Tokenization                                 │
│     • MinHash (128-byte signatures)                │
│     • LSH (bucket indexing)                        │
│     • Duplicate detection                          │
│     • Union-Find clustering                        │
│                         │                          │
│                         ▼                          │
│  3. Local Storage (mmap files)                     │
│     • signatures.mmap                              │
│     • lsh_buckets.mmap                             │
│     • audit_trail.jsonl (Q34)                      │
│                         │                          │
│                         ▼                          │
│  4. Results (Output)                               │
│     • Duplicate clusters                           │
│     • Deduplicated dataset                         │
│                                                     │
│  NO DATA LEAVES YOUR MACHINE                        │
└─────────────────────────────────────────────────────┘

EXCEPTION: License Activation (One-Time Only)
┌─────────────┐  HTTPS (TLS 1.3)  ┌─────────────────┐
│ Your Machine│ ─────────────────> │ Activation      │
│             │ {license_key,      │ Server          │
│             │  hw_fingerprint}   │                 │
│             │ <───────────────── │                 │
│             │ {status, tier}     │                 │
└─────────────┘                    └─────────────────┘
```

### Q34 Audit Trail Example

```json
{
  "version": "1.0",
  "software": "kindly_dedup",
  "entries": [
    {
      "timestamp": "2025-11-25T12:34:56.789Z",
      "operation": "add_document",
      "document_id": 0,
      "signature_hash": "sha256:a1b2c3d4e5f6...",
      "previous_hash": "sha256:0000000000000000...",
      "chain_hash": "sha256:f1e2d3c4b5a6..."
    },
    {
      "timestamp": "2025-11-25T12:34:56.790Z",
      "operation": "add_document",
      "document_id": 1,
      "signature_hash": "sha256:b2c3d4e5f6a1...",
      "previous_hash": "sha256:f1e2d3c4b5a6...",
      "chain_hash": "sha256:c3d4e5f6a1b2..."
    }
  ],
  "integrity": {
    "chain_valid": true,
    "total_documents": 2,
    "total_operations": 2
  }
}
```

**Privacy Properties:**
- **No document content** (only hashes)
- **No personal data** (only document IDs)
- **Tamper-evident** (hash chain breaks if modified)
- **Local storage** (never transmitted)

---

**END OF PRIVACY POLICY**
