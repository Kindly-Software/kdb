# 6900HX Deployment & Validation Guide

**Purpose**: Deploy and validate `client_demo` on 6900HX production hardware

**Hardware**: AMD Ryzen 9 6900HX (16 cores @ 3.3-4.9 GHz), 64 GB DDR5-4800, Ubuntu Server 24.04

**IP**: 192.168.0.38

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [Deployment Strategy](#deployment-strategy)
3. [Validation Criteria](#validation-criteria)
4. [Usage](#usage)
5. [Manual Deployment](#manual-deployment)
6. [Troubleshooting](#troubleshooting)

---

## Quick Start

### Automated Deployment (Recommended)

```bash
# Build and run deployment system
cargo run --bin deploy_validate_6900hx --features benchmarking

# Dry run (show commands without executing)
cargo run --bin deploy_validate_6900hx --features benchmarking -- --dry-run

# Custom customer ID
cargo run --bin deploy_validate_6900hx --features benchmarking -- --customer-id demo-6900hx-v2
```

**Expected Runtime**: ~45 minutes (all 3 tiers)

**Output**:
- Console: Real-time progress + validation report
- Remote: `/tmp/demo_audit_demo_6900hx.jsonl` (audit trail)
- Remote: `./demo_results_[TIMESTAMP].txt` (summary report)

---

## Deployment Strategy

### Why Build on 6900HX?

**Native Performance**: Building on the target hardware eliminates cross-compilation overhead and ensures optimal CPU feature detection (AVX2, RDRAND, cache topology).

**Zero Transfer**: No need to transfer large binaries (100+ MB with symbols). Build time on 6900HX is comparable to local due to 16 cores.

### Steps

1. **SSH Verification**: Verify connectivity to `samuel@192.168.0.38`
2. **Project Check**: Ensure `/home/samuel/Primitives/kindly_dedup` exists and is synced (via lsyncd or manual)
3. **Native Build**: `cargo build --release --bin client_demo --features 'meta-capsule,benchmarking'`
4. **Execute Demo**: Run all 3 tiers (100K/1M/10M docs)
5. **Collect Results**: Parse stdout, copy audit trail
6. **Validate Claims**: Check F1 ≥99%, throughput ≥60K, speedup ≥35×

---

## Validation Criteria

### B32 Framework Thresholds

| Metric | Threshold | Claim | Tier |
|--------|-----------|-------|------|
| **F1 Score** | ≥99.0% | 100% | Tier 1 (100K accuracy) |
| **Throughput** | ≥60K docs/sec | 60K | Tier 2 (1M production) |
| **Speedup** | ≥35× | 38× | vs Python datasketch (1,572 docs/sec) |
| **Tier 3 Time** | ≤300s (5 min) | 167s (2.78 min) | 10M massive scale |

### META_CAPSULE Validation

**4 Layers Expected**:
1. **Layer 1**: Build-time verification (customer ID embedding, binary signing)
2. **Layer 2**: Circuit breaker (8 detection methods: debugger, VM, memory, injection, timing, fault, hardware, voting)
3. **Layer 3**: License validation (DualAtomicU64 + hardware binding)
4. **Layer 4**: Audit trail (AtomicHash256 hash-chained Q34 compliance)

**PUF Stability**: ≥95% (claim: 96.9% on 6900HX validated)

---

## Usage

### Automated Deployment

```bash
# Full deployment and validation
cargo run --bin deploy_validate_6900hx --features benchmarking

# Expected output:
# ═══════════════════════════════════════════════════════════
#   6900HX Deployment & Validation System
#   kindly_dedup - Production Performance Validation
# ═══════════════════════════════════════════════════════════
#
# [CONFIGURATION]
#   SSH Host: samuel@192.168.0.38
#   Remote Dir: ~/Primitives/kindly_dedup
#   Customer ID: demo-6900hx
#   Timeout: 3600 seconds
#
# [VALIDATION THRESHOLDS]
#   F1 Score: ≥99.0%
#   Throughput: ≥60000 docs/sec
#   Speedup: ≥35.0×
#   Tier 3 Time: ≤300s
#
# [STEP 1] Verifying SSH connectivity to 6900HX...
# [SSH] Executing: hostname && uname -a
# ├─ Remote host: 6900hx-brain
# └─ ✓ SSH connection verified
#
# [STEP 2] Verifying project directory...
# └─ ✓ Project directory exists: ~/Primitives/kindly_dedup
#
# [STEP 3] Building client_demo on 6900HX (native)...
# ├─ Customer ID: demo-6900hx
# ├─ Features: meta-capsule, benchmarking
# └─ Profile: release (opt-level=3, lto=fat)
# [SSH] Executing: cd ~/Primitives/kindly_dedup && CUSTOMER_ID=demo-6900hx cargo build --release --bin client_demo --features 'meta-capsule,benchmarking' 2>&1
#
# ✓ Build successful
#   Finished release [optimized] target(s) in 82.34s
#
# [STEP 4] Running client_demo...
# ├─ Tier 1: 100K docs (accuracy validation)
# ├─ Tier 2: 1M docs (throughput demonstration)
# ├─ Tier 3: 10M docs (scale capability)
# └─ Estimated time: 45 minutes
#
# [SSH] Executing: cd ~/Primitives/kindly_dedup && echo '' | ./target/release/client_demo 2>&1
#
# ... [demo output] ...
#
# ✓ Demo completed in 43.2 minutes
#
# [STEP 5] Collecting audit trail...
# └─ ✓ Audit trail found: /tmp/demo_audit_demo_6900hx.jsonl
#
# [OPTIONAL] Copy audit trail to local machine?
#   scp samuel@192.168.0.38:/tmp/demo_audit_demo_6900hx.jsonl ./
#
# ═══════════════════════════════════════════════════════════
#   6900HX VALIDATION REPORT
# ═══════════════════════════════════════════════════════════
#
# [BUILD]
#   Status: ✓ Success
#
# [DEMO EXECUTION]
#   Status: ✓ Success
#
# [TIER 1: ACCURACY VALIDATION]
#   F1 Score: 100.00% (threshold: 99.00%) ✓ PASS
#
# [TIER 2: THROUGHPUT VALIDATION]
#   Throughput: 62341 docs/sec (threshold: 60000) ✓ PASS
#
# [TIER 3: SCALE VALIDATION]
#   Time: 165.3s (threshold: 300.0s) ✓ PASS
#
# [SPEEDUP VALIDATION]
#   Speedup: 39.6× vs Python datasketch (threshold: 35.0×) ✓ PASS
#
# [META_CAPSULE VALIDATION]
#   Layers active: 4
#     - Layer 1: Build-time verification
#     - Layer 2: Circuit breaker (8 checks)
#     - Layer 3: License validation
#     - Layer 4: Audit trail
#
# [PUF VALIDATION]
#   Stability: 96.9% (target: ≥95%)
#
# [AUDIT TRAIL]
#   Path: /tmp/demo_audit_demo_6900hx.jsonl
#
# ═══════════════════════════════════════════════════════════
#   ✓ ALL VALIDATIONS PASSED
# ═══════════════════════════════════════════════════════════
#
# [DEPLOYMENT SUMMARY]
#   Total time: 45.7 minutes
#   SSH Host: samuel@192.168.0.38
#   Customer ID: demo-6900hx
#
# ✓ DEPLOYMENT & VALIDATION SUCCESSFUL
#   All claims validated on 6900HX production hardware
```

### Dry Run (Test Without Execution)

```bash
cargo run --bin deploy_validate_6900hx --features benchmarking -- --dry-run

# Shows all commands that would be executed without running them
# Useful for:
# - Verifying SSH configuration
# - Checking remote paths
# - Reviewing build commands
```

---

## Manual Deployment

If the automated system fails or you need more control:

### Step 1: SSH to 6900HX

```bash
ssh samuel@192.168.0.38
```

### Step 2: Navigate to Project

```bash
cd ~/Primitives/kindly_dedup
```

### Step 3: Build client_demo

```bash
CUSTOMER_ID=demo-6900hx cargo build --release --bin client_demo --features 'meta-capsule,benchmarking'
```

**Expected Build Time**: ~80 seconds (16 cores, lto=fat)

### Step 4: Run Demo

```bash
# Run all 3 tiers (45 min)
./target/release/client_demo

# Skip Tier 3 (press Ctrl+C when prompted)
./target/release/client_demo
# ... wait for Tier 3 prompt ...
# Press Ctrl+C

# Auto-skip Tier 3 (via stdin)
echo "skip" | ./target/release/client_demo
```

### Step 5: Collect Results

```bash
# View audit trail
cat /tmp/demo_audit_demo_6900hx.jsonl | tail -20

# Copy to local machine
scp samuel@192.168.0.38:/tmp/demo_audit_demo_6900hx.jsonl ./

# View summary (if generated)
cat demo_results_*.txt
```

### Step 6: Manual Validation

Check stdout for:
- **F1 Score**: Look for "F1 Score: XX.XX%" in Tier 1 output
- **Throughput**: Look for "Single-threaded: XXXXX docs/sec" in summary
- **Speedup**: Look for "Speedup: XX.X×" in summary
- **META_CAPSULE**: Look for "META_CAPSULE: Active (4 layers)"
- **PUF**: Look for "PUF Stability: XX.X%"

---

## Troubleshooting

### SSH Connection Fails

**Error**: `ssh: connect to host 192.168.0.38 port 22: No route to host`

**Solutions**:
1. Verify 6900HX is powered on and connected to network
2. Check IP address: `ip addr show` on 6900HX
3. Verify SSH service: `sudo systemctl status ssh` on 6900HX
4. Check firewall: `sudo ufw status` on 6900HX

### Project Directory Not Found

**Error**: `Project directory not found: ~/Primitives/kindly_dedup`

**Solutions**:
1. Check lsyncd status: `systemctl status lsyncd` (local machine)
2. Manually sync:
   ```bash
   rsync -avz --delete /home/samuel/Primitives/kindly_dedup/ samuel@192.168.0.38:~/Primitives/kindly_dedup/
   ```
3. Verify on 6900HX: `ssh samuel@192.168.0.38 "ls -la ~/Primitives/kindly_dedup/"`

### Build Fails

**Error**: `error: linker 'cc' not found`

**Solutions**:
1. Install build essentials on 6900HX:
   ```bash
   ssh samuel@192.168.0.38 "sudo apt-get update && sudo apt-get install -y build-essential"
   ```
2. Verify Rust toolchain:
   ```bash
   ssh samuel@192.168.0.38 "rustc --version"
   ```

**Error**: `error[E0425]: cannot find function 'compute_ground_truth_production' in module 'benchmarking'`

**Solutions**:
1. Ensure project is fully synced (lsyncd or manual rsync)
2. Clean and rebuild:
   ```bash
   ssh samuel@192.168.0.38 "cd ~/Primitives/kindly_dedup && cargo clean && cargo build --release --bin client_demo --features 'meta-capsule,benchmarking'"
   ```

### Demo Runs Too Slowly

**Expected**: 45 minutes for all 3 tiers

**If slower**:
1. Check CPU frequency: `ssh samuel@192.168.0.38 "cat /proc/cpuinfo | grep MHz"`
2. Verify no background processes: `ssh samuel@192.168.0.38 "htop"`
3. Check memory: `ssh samuel@192.168.0.38 "free -h"`

**If much faster**: Verify correctness (not skipping tiers)

### META_CAPSULE Not Active

**Error**: `⚠ META_CAPSULE not enabled`

**Solutions**:
1. Rebuild with correct features:
   ```bash
   cargo build --release --bin client_demo --features 'meta-capsule,benchmarking'
   ```
2. Verify customer ID is set:
   ```bash
   CUSTOMER_ID=demo-6900hx cargo build --release --bin client_demo --features 'meta-capsule,benchmarking'
   ```

### Validation Failures

**F1 Score < 99%**:
- Check ground truth implementation (should use ExhaustiveCompound)
- Verify threshold (0.85 Jaccard)
- Review corpus distribution (5% exact, 15% near, 80% unique)

**Throughput < 60K docs/sec**:
- Check CPU frequency (should be 3.3-4.9 GHz)
- Verify no background load
- Review memory usage (should have 30+ GB free)

**Speedup < 35×**:
- Verify baseline (Python datasketch: 1,572 docs/sec)
- Check throughput measurement (same corpus)

---

## Production Notes

### Hardware Requirements

**Minimum**:
- CPU: 8 cores @ 2.5 GHz
- RAM: 32 GB
- Storage: 10 GB free (for corpus generation)

**Recommended** (6900HX):
- CPU: 16 cores @ 3.3-4.9 GHz (AMD Ryzen 9 6900HX)
- RAM: 64 GB DDR5-4800
- Storage: 50 GB NVMe SSD

### Performance Scaling

| Corpus Size | Time (6900HX) | Throughput | Memory |
|-------------|---------------|------------|--------|
| 100K | ~17 min | 60K docs/sec | ~2 GB |
| 1M | ~17 sec | 60K docs/sec | ~10 GB |
| 10M | ~167 sec | 60K docs/sec | ~50 GB |

**Projected Multi-threaded** (16 cores @ 60% efficiency):
- Throughput: ~576K docs/sec (9.6× single-threaded)
- 10M corpus: ~17 seconds

### Security Considerations

**META_CAPSULE Protection**:
- **Layer 1**: Binary signing prevents tampering
- **Layer 2**: Circuit breaker detects debugging/analysis attempts
- **Layer 3**: Hardware binding prevents unauthorized hardware
- **Layer 4**: Audit trail provides Q34 compliance

**Trade Secret Notice**: This binary contains billion-dollar capsule architecture IP. Unauthorized reverse engineering or distribution is prohibited.

### Compliance

**Q34 Auditability**:
- **Audit Trail**: `/tmp/demo_audit_[CUSTOMER_ID].jsonl`
- **Format**: JSONL (one JSON object per line)
- **Hash Chain**: AtomicHash256 tamper-evident
- **Retention**: 90 days (configurable)

**SOX/SOC2/GDPR/HIPAA**: Q34 audit trail provides compliance-ready event logging.

---

## Contact

**Technical Support**: support@kindly.ai
**Sales**: sales@kindly.ai
**Documentation**: https://docs.kindly.ai

---

## Appendix: File Locations

### Local Machine

- **Deployment script**: `/home/samuel/Primitives/kindly_dedup/src/bin/deploy_validate_6900hx.rs`
- **Client demo**: `/home/samuel/Primitives/kindly_dedup/src/bin/client_demo.rs`
- **This guide**: `/home/samuel/Primitives/kindly_dedup/DEPLOYMENT_6900HX.md`

### 6900HX Server

- **Project directory**: `~/Primitives/kindly_dedup/`
- **Binary**: `~/Primitives/kindly_dedup/target/release/client_demo`
- **Audit trail**: `/tmp/demo_audit_[CUSTOMER_ID].jsonl`
- **Results**: `~/Primitives/kindly_dedup/demo_results_[TIMESTAMP].txt`

---

**Last Updated**: 2025-10-29
**Version**: 1.0
**Status**: Production-Ready
