# kdb-mcp Public Beta Test Plan

**Version**: 1.0.0
**Date**: 2025-12-04
**Target Release**: Q1 2025
**Test Duration**: 4 weeks (2 weeks internal, 2 weeks public beta)

---

## Executive Summary

Comprehensive test plan for public deployment of kdb-mcp, the T6 Mixed MCP debugging server with <10us latency. This plan covers security penetration testing, performance load testing, compliance verification, and user acceptance testing to ensure production readiness.

**Go/No-Go Criteria**:
- All P0 security tests PASS
- <10us P95 latency under 1000 concurrent clients
- Q34 audit trail integrity verified
- Zero critical vulnerabilities in penetration testing
- 95%+ user satisfaction in UAT

---

## 1. Security Penetration Testing

### 1.1 Authentication Bypass Tests

| Test ID | Description | Method | Expected Result | Priority |
|---------|-------------|--------|-----------------|----------|
| SEC-001 | JWT signature forgery | Tamper with JWT payload | Reject with 401 | P0 |
| SEC-002 | JWT algorithm confusion | HS256 vs RS256 attack | Reject with 401 | P0 |
| SEC-003 | Expired JWT replay | Replay expired token | Reject with 401 | P0 |
| SEC-004 | TOTP brute force | 10,000 TOTP attempts | Rate limit after 10 | P0 |
| SEC-005 | TOTP time skew attack | +/- 5 minute window | Accept only +/- 1 step | P1 |
| SEC-006 | Session fixation | Pre-set session ID | Generate new session | P0 |
| SEC-007 | Session hijacking | Stolen session cookie | Zero-trust detects IP change | P1 |
| SEC-008 | API key enumeration | Timing attack on lookup | Constant-time comparison | P0 |

**Test Script**:
```bash
#!/bin/bash
# SEC-001: JWT signature forgery
FORGED_JWT=$(echo '{"sub":"admin","exp":9999999999}' | base64).$(echo '{"alg":"none"}' | base64).
curl -X POST https://debug.kindly.dev/mcp \
  -H "Authorization: Bearer $FORGED_JWT" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"debugger/attach","params":{"pid":12345}}'
# Expected: 401 Unauthorized
```

### 1.2 PID Allowlist Enforcement Tests

| Test ID | Description | Method | Expected Result | Priority |
|---------|-------------|--------|-----------------|----------|
| PID-001 | Debug PID 1 (init) | Attach to systemd | Reject with 403 | P0 |
| PID-002 | Debug kernel thread | Attach to kworker | Reject with 403 | P0 |
| PID-003 | Debug unauthorized PID | Random non-whitelisted PID | Reject with 403 | P0 |
| PID-004 | PID race condition | Rapid add/check/remove | TOCTOU prevented | P0 |
| PID-005 | Bloom filter collision | Craft colliding PID | Hash table fallback | P1 |
| PID-006 | Negative PID injection | PID = -1 or MAX_INT | Input validation | P0 |

**Test Script**:
```bash
#!/bin/bash
# PID-001: Attempt to debug init process
curl -X POST https://debug.kindly.dev/mcp \
  -H "Authorization: Bearer $VALID_JWT" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"debugger/attach","params":{"pid":1}}'
# Expected: {"jsonrpc":"2.0","id":1,"error":{"code":-32001,"message":"PID 1 not whitelisted"}}
```

### 1.3 Rate Limiting Tests

| Test ID | Description | Method | Expected Result | Priority |
|---------|-------------|--------|-----------------|----------|
| RATE-001 | Global rate limit | 2000 req/sec burst | Throttle after 1000 | P0 |
| RATE-002 | Per-client rate limit | 20 req/sec single client | Throttle after 10 | P0 |
| RATE-003 | Distributed attack | 100 IPs x 50 req/sec | Fair quota per client | P1 |
| RATE-004 | Rate limit bypass | X-Forwarded-For spoofing | Use real IP from nginx | P0 |
| RATE-005 | Slowloris attack | Slow HTTP requests | Timeout after 30s | P1 |

**Test Script**:
```bash
#!/bin/bash
# RATE-001: Global rate limit stress test
for i in {1..2000}; do
  curl -X POST https://debug.kindly.dev/mcp \
    -H "Authorization: Bearer $VALID_JWT" \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","id":'$i',"method":"debugger/get_stack_trace"}' &
done
wait
# Expected: First 1000 succeed, remaining 1000 get 429 Too Many Requests
```

### 1.4 Audit Trail Integrity Tests

| Test ID | Description | Method | Expected Result | Priority |
|---------|-------------|--------|-----------------|----------|
| AUDIT-001 | Hash chain verification | Verify CRC64 chain | All events chain correctly | P0 |
| AUDIT-002 | Tamper detection | Modify audit entry | Hash mismatch detected | P0 |
| AUDIT-003 | Event ordering | Concurrent events | Monotonic timestamps | P0 |
| AUDIT-004 | Export integrity | JSON/CSV export | Hash chain preserved | P1 |
| AUDIT-005 | Retention compliance | 7-year simulation | No data loss | P1 |

**Test Script**:
```bash
#!/bin/bash
# AUDIT-001: Verify hash chain integrity
cargo run --release --bin audit_verify -- \
  --audit-dir /var/log/kdb-mcp \
  --verify-hash-chain
# Expected: "Hash chain verified: 65,536 events, 0 integrity violations"
```

### 1.5 Time-Travel Debugging Consistency Tests

| Test ID | Description | Method | Expected Result | Priority |
|---------|-------------|--------|-----------------|----------|
| TTD-001 | Snapshot order | Forward/backward steps | Consistent state replay | P0 |
| TTD-002 | Snapshot capacity | 2048 snapshots | FIFO eviction works | P1 |
| TTD-003 | Concurrent snapshots | Multi-client replay | Isolated per session | P0 |
| TTD-004 | Memory corruption | Invalid snapshot ID | Graceful error | P0 |
| TTD-005 | Snapshot timing | 6-8ns capture target | Within SLA | P1 |

---

## 2. Performance Load Testing

### 2.1 Latency Targets

| Metric | Target | Measurement Method | Priority |
|--------|--------|-------------------|----------|
| P50 latency | <5us | Criterion benchmark | P0 |
| P95 latency | <10us | Criterion benchmark | P0 |
| P99 latency | <20us | Criterion benchmark | P1 |
| P99.9 latency | <50us | Criterion benchmark | P1 |

### 2.2 Load Test Scenarios

| Scenario | Clients | Req/sec | Duration | Success Criteria |
|----------|---------|---------|----------|------------------|
| Baseline | 1 | 100 | 60s | <5us P50 |
| Normal load | 100 | 1,000 | 300s | <10us P95 |
| Peak load | 500 | 5,000 | 300s | <20us P99 |
| Stress test | 1,000 | 10,000 | 600s | <50us P99.9 |
| Sustained | 100 | 1,000 | 86,400s (24h) | No degradation |
| Spike | 100->1000 | Burst | 60s | Recovery <1s |

**Load Test Script**:
```bash
#!/bin/bash
# B32 Benchmark: 1000 clients, 10,000 req/sec
cargo run --release --bin stress_test -- \
  --clients 1000 \
  --requests-per-client 10000 \
  --target-rps 10000 \
  --duration 600 \
  --output results.json

# Expected output:
# P50:  4.2us
# P95:  8.7us
# P99:  15.3us
# P99.9: 42.1us
# Throughput: 773,000 req/sec
```

### 2.3 Resource Utilization

| Resource | Target | Alert Threshold |
|----------|--------|-----------------|
| CPU | <50% at peak | >80% |
| Memory | <100MB baseline | >500MB |
| Network | <100Mbps | >500Mbps |
| Disk I/O | <10MB/s (audit) | >50MB/s |

---

## 3. Compliance Testing

### 3.1 Q34 Audit Trail Verification

| Test ID | Requirement | Verification | Status |
|---------|-------------|--------------|--------|
| Q34-001 | All auth events logged | Count auth vs audit entries | [ ] |
| Q34-002 | Hash-chain integrity | CRC64 verification | [ ] |
| Q34-003 | Tamper-evident | Modify entry, detect | [ ] |
| Q34-004 | 7-year retention | S3 lifecycle policy | [ ] |
| Q34-005 | Export formats | JSON/CSV/binary | [ ] |

### 3.2 SOX/SOC2/GDPR/HIPAA Checklist

| Standard | Requirement | Implementation | Verified |
|----------|-------------|----------------|----------|
| **SOX** | Audit trail retention | 7-year S3 archive | [ ] |
| **SOX** | Access control logging | All operations logged | [ ] |
| **SOC2** | Security controls | 18-capsule defense | [ ] |
| **SOC2** | Availability (99.9%) | HA deployment | [ ] |
| **GDPR** | Data protection | ChaCha20 encryption | [ ] |
| **GDPR** | Right to erasure | PII deletion API | [ ] |
| **HIPAA** | Access logging | Q34 audit trail | [ ] |
| **HIPAA** | Transmission security | TLS 1.3 only | [ ] |

---

## 4. User Acceptance Testing (UAT)

### 4.1 Beta Tester Recruitment

| Tier | Count | Selection Criteria |
|------|-------|-------------------|
| Alpha | 10 | Internal developers |
| Beta (Early) | 50 | Enterprise customers |
| Beta (Public) | 500 | Developer community |

### 4.2 UAT Scenarios

| Scenario | Description | Success Criteria |
|----------|-------------|------------------|
| UAT-001 | First-time setup | <5 min to first debug session |
| UAT-002 | Claude Code integration | All 9 tools work |
| UAT-003 | Time-travel debugging | Forward/backward works |
| UAT-004 | Multi-process debugging | 10 PIDs simultaneously |
| UAT-005 | License activation | <30s activation |
| UAT-006 | Error recovery | Graceful error messages |
| UAT-007 | Documentation clarity | 80%+ can self-serve |

### 4.3 Feedback Collection

| Channel | Method | Frequency |
|---------|--------|-----------|
| In-app | NPS survey | After each session |
| Email | Weekly digest | Weekly |
| GitHub | Issue tracker | Continuous |
| Discord | Community channel | Real-time |

### 4.4 Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Net Promoter Score (NPS) | >50 | In-app survey |
| Setup success rate | >95% | Telemetry |
| Feature adoption | >70% use time-travel | Analytics |
| Support tickets | <10/day | Help desk |
| Retention (day 7) | >60% | Cohort analysis |
| Retention (day 30) | >40% | Cohort analysis |

---

## 5. Test Schedule

### Week 1-2: Internal Testing

| Day | Activity | Owner | Status |
|-----|----------|-------|--------|
| 1-2 | Security penetration tests (SEC-*) | Security Team | [ ] |
| 3-4 | PID allowlist tests (PID-*) | Core Team | [ ] |
| 5 | Rate limiting tests (RATE-*) | Core Team | [ ] |
| 6-7 | Audit trail tests (AUDIT-*) | Compliance Team | [ ] |
| 8-9 | Load testing (all scenarios) | Performance Team | [ ] |
| 10 | Compliance checklist | Compliance Team | [ ] |

### Week 3-4: Public Beta

| Day | Activity | Owner | Status |
|-----|----------|-------|--------|
| 15 | Alpha release (10 users) | Release Team | [ ] |
| 16-17 | Alpha feedback collection | Product Team | [ ] |
| 18 | Bug fixes from alpha | Core Team | [ ] |
| 19 | Early beta release (50 users) | Release Team | [ ] |
| 20-24 | Beta feedback collection | Product Team | [ ] |
| 25 | Public beta release (500 users) | Release Team | [ ] |
| 26-28 | Monitor and iterate | All Teams | [ ] |

---

## 6. Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Security vulnerability discovered | Medium | Critical | Bug bounty, rapid response |
| Performance degradation at scale | Low | High | Horizontal scaling ready |
| Compliance gap identified | Low | High | External audit scheduled |
| Low beta adoption | Medium | Medium | Developer outreach program |
| Integration issues with Claude | Low | High | Anthropic partnership |

---

## 7. Go/No-Go Criteria

### Must Pass (P0)

- [ ] All SEC-* tests pass
- [ ] All PID-* tests pass
- [ ] All RATE-* tests pass
- [ ] All AUDIT-* tests pass
- [ ] <10us P95 latency at 1000 clients
- [ ] Zero critical security vulnerabilities
- [ ] Q34 audit trail integrity verified

### Should Pass (P1)

- [ ] All TTD-* tests pass
- [ ] NPS > 50 in beta
- [ ] >95% setup success rate
- [ ] <20us P99 latency at peak

### Nice to Have (P2)

- [ ] >70% feature adoption
- [ ] <5 support tickets/day
- [ ] External security audit complete

---

## 8. Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Engineering Lead | | | |
| Security Lead | | | |
| Compliance Lead | | | |
| Product Lead | | | |
| CEO | | | |

---

**Document Maintained By**: Release Team
**Last Updated**: 2025-12-04
**Review Cadence**: Weekly during beta
