# END USER LICENSE AGREEMENT (EULA)

**kindly_dedup - LLM Training Dataset Deduplication Software**

**Version:** 1.0
**Effective Date:** [DATE]
**Last Updated:** [DATE]

---

## 1. ACCEPTANCE OF TERMS

BY INSTALLING, COPYING, OR OTHERWISE USING THIS SOFTWARE ("kindly_dedup"), YOU ("LICENSEE") AGREE TO BE BOUND BY THE TERMS OF THIS END USER LICENSE AGREEMENT ("AGREEMENT"). IF YOU DO NOT AGREE TO THESE TERMS, DO NOT INSTALL OR USE THE SOFTWARE.

This Agreement is a legal contract between you and [COMPANY NAME] ("LICENSOR") governing your use of kindly_dedup software.

---

## 2. GRANT OF LICENSE

### 2.1 License Tiers

Subject to your compliance with this Agreement and payment of applicable fees, Licensor grants you a non-exclusive, non-transferable, revocable license to use kindly_dedup according to your purchased tier:

| Tier | Document Limit | Use Case | Price |
|------|----------------|----------|-------|
| **Demo** | 1,000 documents | Evaluation, testing | Free (30-day trial) |
| **Basic** | 100,000 documents | Small datasets, research | [PRICE] |
| **Pro** | 10,000,000 documents | Production LLM training | [PRICE] |
| **Enterprise** | Unlimited | Large-scale ML operations | [PRICE] (contact sales) |

### 2.2 Scope of License

You may:
- Install and use the software on hardware you own or control
- Process documents up to your tier's document limit
- Use the software for internal business purposes only
- Create backups for disaster recovery

### 2.3 License Restrictions

You may NOT:
- Exceed your licensed document limit without upgrading
- Use the software for service bureau or time-sharing purposes
- Distribute, sublicense, rent, lease, or lend the software
- Use the software to provide services to third parties
- Remove or modify any proprietary notices or labels

---

## 3. INTELLECTUAL PROPERTY RIGHTS

### 3.1 Ownership

kindly_dedup software, including but not limited to:
- The computational capsule architecture (T0-T11 tier system)
- Lockfree coordination primitives
- SIMD-accelerated MinHash algorithms
- GPU acceleration kernels (T7 Heterogeneous tier)
- Persistent deduplication architecture (T9+T10)
- Adaptive GPU/CPU pipeline orchestration
- All source code, object code, documentation, and trade secrets

...are the exclusive property of Licensor and are protected by copyright, trade secret, and patent laws.

### 3.2 Trade Secrets

The software contains proprietary algorithms and computational capsule implementations that constitute trade secrets under applicable law. You acknowledge that:
- The software's internal architecture is confidential
- Reverse engineering is strictly prohibited (see Section 4)
- Unauthorized disclosure may result in irreparable harm to Licensor

### 3.3 No Transfer of Rights

This Agreement grants only a limited license to use the software. No ownership rights, title, or intellectual property rights are transferred to you.

---

## 4. RESTRICTIONS

### 4.1 Reverse Engineering

You may NOT:
- Reverse engineer, decompile, or disassemble the software
- Attempt to derive source code from compiled binaries
- Analyze the software's internal algorithms or data structures
- Create derivative works based on the software's architecture
- Benchmark the software against competing products without prior written consent

**Exception:** You may reverse engineer to the extent permitted by applicable law, provided you first contact Licensor to request the necessary information.

### 4.2 Redistribution

You may NOT:
- Distribute copies of the software to third parties
- Make the software available over a network where multiple users can access it
- Include the software in software-as-a-service (SaaS) offerings
- Embed the software in products or services sold to third parties

### 4.3 Modifications

You may NOT:
- Modify, adapt, or create derivative works of the software
- Merge the software with other programs
- Translate the software into other languages or formats

### 4.4 License Key Protection

You agree to:
- Keep your license key confidential
- Not share your license key with unauthorized parties
- Notify Licensor immediately if your license key is compromised
- Use hardware-bound licensing (PUF validation) where applicable

---

## 5. HARDWARE FINGERPRINTING AND LICENSE VERIFICATION

### 5.1 Local Verification

The software uses local hardware fingerprinting (Physical Unclonable Function - PUF) to:
- Bind your license to specific hardware
- Prevent unauthorized license transfers
- Verify license tier limits (document count)

### 5.2 No Cloud Verification

All license verification is performed **locally on your hardware**. No hardware fingerprints or license data are transmitted to Licensor's servers.

### 5.3 Hardware Changes

If you upgrade or replace hardware, contact Licensor to:
- Deactivate the license on old hardware
- Activate the license on new hardware
- Limited to [NUMBER] hardware changes per year

---

## 6. DATA PROCESSING AND PRIVACY

### 6.1 Local Processing

**ALL DATA PROCESSING IS LOCAL.** The software:
- Does NOT upload your documents to cloud servers
- Does NOT transmit your data to Licensor or third parties
- Processes all documents entirely on your local hardware
- Stores all results (signatures, LSH buckets, audit trails) locally

### 6.2 No Telemetry by Default

The software does NOT collect or transmit:
- Usage statistics
- Performance metrics
- Document content or metadata
- Error reports or crash dumps

**Exception:** If you explicitly enable opt-in analytics (disabled by default), minimal anonymized metrics may be collected. See PRIVACY_POLICY.md for details.

### 6.3 Audit Trails (Q34 Compliance)

The software generates cryptographic audit trails (hash-chained logs) for compliance purposes (SOX, SOC2, GDPR, HIPAA). These audit trails:
- Are stored **locally only** (no cloud upload)
- Contain NO personally identifiable information (PII)
- Contain NO document content
- Record only: timestamps, operation types, hash chains

You retain full ownership and control of all audit trail data.

### 6.4 Your Responsibilities

You are responsible for:
- Ensuring your use of the software complies with applicable data protection laws (GDPR, CCPA, etc.)
- Obtaining necessary consents from data subjects (if processing personal data)
- Implementing appropriate security measures for your data
- Maintaining backups of your processed data

---

## 7. TERM AND TERMINATION

### 7.1 Term

This Agreement is effective upon installation and continues until terminated.

### 7.2 Termination by Licensee

You may terminate this Agreement at any time by:
- Uninstalling the software
- Destroying all copies of the software
- Ceasing all use of the software

### 7.3 Termination by Licensor

Licensor may terminate this Agreement immediately if you:
- Violate any term of this Agreement
- Fail to pay applicable license fees
- Engage in reverse engineering or unauthorized redistribution
- Exceed your licensed document limit

### 7.4 Effect of Termination

Upon termination:
- Your license to use the software immediately ceases
- You must uninstall and destroy all copies of the software
- Sections 3 (Intellectual Property), 8 (Warranty Disclaimer), 9 (Limitation of Liability), and 11 (General Provisions) survive termination

---

## 8. WARRANTY DISCLAIMER

### 8.1 AS-IS Software

THE SOFTWARE IS PROVIDED "AS IS" WITHOUT WARRANTY OF ANY KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO:
- MERCHANTABILITY
- FITNESS FOR A PARTICULAR PURPOSE
- NON-INFRINGEMENT
- ACCURACY OR RELIABILITY
- UNINTERRUPTED OR ERROR-FREE OPERATION

### 8.2 Performance Claims

Performance claims (e.g., "60K docs/sec", "38× speedup", "93% memory reduction") are based on internal testing under specific conditions (AMD Ryzen 9 6900HX, 64GB DDR5-4800). Actual performance may vary based on:
- Hardware capabilities (CPU, RAM, disk speed)
- Dataset characteristics (document size, duplication ratio)
- System configuration (OS, other running processes)

**NO GUARANTEE** is made that you will achieve the same performance.

### 8.3 No Support Obligation

Licensor is under no obligation to provide:
- Technical support
- Bug fixes
- Software updates
- Feature enhancements

**Exception:** Enterprise tier licenses may include support services under separate agreement.

### 8.4 Data Loss Risk

You acknowledge that:
- Software bugs may cause data loss or corruption
- You are responsible for maintaining backups
- Licensor is NOT liable for any data loss (see Section 9)

---

## 9. LIMITATION OF LIABILITY

### 9.1 Maximum Liability

TO THE MAXIMUM EXTENT PERMITTED BY LAW, LICENSOR'S TOTAL LIABILITY UNDER THIS AGREEMENT SHALL NOT EXCEED THE AMOUNT YOU PAID FOR THE SOFTWARE IN THE 12 MONTHS PRECEDING THE CLAIM.

### 9.2 Exclusion of Consequential Damages

IN NO EVENT SHALL LICENSOR BE LIABLE FOR:
- LOSS OF PROFITS, REVENUE, OR BUSINESS OPPORTUNITIES
- LOSS OF DATA OR INTERRUPTION OF BUSINESS
- COST OF SUBSTITUTE GOODS OR SERVICES
- INDIRECT, INCIDENTAL, SPECIAL, CONSEQUENTIAL, OR PUNITIVE DAMAGES

...EVEN IF LICENSOR HAS BEEN ADVISED OF THE POSSIBILITY OF SUCH DAMAGES.

### 9.3 Critical Applications

THE SOFTWARE IS NOT DESIGNED FOR USE IN:
- Life-critical medical devices
- Nuclear facilities
- Aviation or aerospace systems
- Military applications
- Any application where software failure could result in death or serious injury

YOU ASSUME ALL RISK IF USING THE SOFTWARE IN SUCH APPLICATIONS.

### 9.4 Data Processing Liability

Licensor is NOT liable for:
- Accuracy of duplicate detection (false positives/negatives)
- Data quality issues in your corpus
- Compliance with your data protection obligations
- Unauthorized access to your data due to your security failures

---

## 10. UPDATES AND UPGRADES

### 10.1 Updates

Licensor may provide software updates (bug fixes, security patches) at its discretion. Updates do NOT constitute new licenses and are subject to this Agreement.

### 10.2 Upgrades

Upgrades (new major versions, new features) may require:
- Separate purchase
- Acceptance of updated EULA terms
- Upgrade fees (at Licensor's discretion)

### 10.3 No Obligation

Licensor is under NO obligation to provide updates or upgrades.

---

## 11. GENERAL PROVISIONS

### 11.1 Governing Law

This Agreement shall be governed by the laws of [JURISDICTION], without regard to conflict of law principles.

### 11.2 Dispute Resolution

Any disputes arising under this Agreement shall be resolved through:
1. Good faith negotiation (30 days)
2. Mediation (if negotiation fails)
3. Binding arbitration under [ARBITRATION RULES]

### 11.3 Entire Agreement

This Agreement, together with PRIVACY_POLICY.md, constitutes the entire agreement between you and Licensor regarding the software and supersedes all prior agreements.

### 11.4 Severability

If any provision of this Agreement is held invalid or unenforceable, the remaining provisions shall remain in full force and effect.

### 11.5 Waiver

Failure to enforce any right under this Agreement does NOT constitute a waiver of that right.

### 11.6 Assignment

You may NOT assign or transfer this Agreement without Licensor's prior written consent. Licensor may assign this Agreement without restriction.

### 11.7 Export Control

You agree to comply with all applicable export control laws and regulations.

### 11.8 Government Use

If you are a government entity, the software is "commercial computer software" and is provided with RESTRICTED RIGHTS as defined in applicable regulations.

### 11.9 Contact Information

For questions about this Agreement, contact:

**[COMPANY NAME]**
Email: [EMAIL]
Address: [ADDRESS]
Website: [WEBSITE]

---

## 12. ACKNOWLEDGMENT

BY INSTALLING OR USING THIS SOFTWARE, YOU ACKNOWLEDGE THAT:
- You have read and understand this Agreement
- You agree to be bound by its terms
- You have authority to accept this Agreement on behalf of your organization (if applicable)
- You understand the software's limitations and warranties

**Version:** 1.0
**Last Updated:** [DATE]

---

## APPENDIX A: DEFINITIONS

- **Document**: A single text unit (e.g., web page, article, dataset row) processed by the software
- **Duplicate**: Two documents with Jaccard similarity ≥ threshold (default 85%)
- **Computational Capsule**: Proprietary lockfree data structure (trade secret)
- **Tier**: License level determining document processing limits
- **Hardware Fingerprint**: Physical Unclonable Function (PUF) derived from CPU characteristics
- **Audit Trail**: Q34-compliant hash-chained log of operations (local storage only)

---

## APPENDIX B: OPEN SOURCE COMPONENTS

kindly_dedup uses the following open source components (full licenses in `LICENSE-THIRD-PARTY.md`):
- Rust standard library (MIT/Apache-2.0)
- wgpu (MIT/Apache-2.0) - GPU acceleration
- criterion (MIT/Apache-2.0) - benchmarking
- atomic_capsule (proprietary, internal use only)

All open source components are used in compliance with their respective licenses.

---

**END OF AGREEMENT**
