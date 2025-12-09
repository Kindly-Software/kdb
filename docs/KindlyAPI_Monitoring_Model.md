# KindlyAPI Monitoring Model: Tamper-Evident Audit & Intelligent Event Tracking

**Architecture**: Capsule-native monitoring leveraging atomic_ledger_entry (ALE-128), atomic_epoch_tile (ET-1kB), atomic_breaker (ACB-64), plus new intelligent capsules (AIA-1024, AMC-512, AEH-2048)

**Guarantees**:
- **Tamper-evident**: Linear hash chain verification catches any modification
- **Deterministic**: p99 ≈ median latency for health checks
- **Zero-overhead**: Single atomic reads (<5-20ns per check)
- **Crash-safe**: ET-1kB tiles enable instant recovery
- **Intelligence-aware**: Tracks workflow detection, parameter inference, normalization, OAuth refresh

---

## Event Taxonomy

### 0. Intelligence Events (NEW)

**WORKFLOW_DETECTED** - Intelligent endpoint relationship discovered
```rust
AleEvent {
    timestamp: 2025-10-03T14:20:45Z,
    event_type: "WORKFLOW_DETECTED",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    metadata: {
        source_endpoint: "create_customer",
        target_endpoint: "create_subscription",
        relationship_type: "sequential",  // or "parallel", "conditional"
        confidence_score: 0.92,
        usage_pattern: "create_customer → create_subscription (87% of users)",
    },
    prev_hash: "...",
    hash: "..."
}
```

**PARAMETER_INFERRED** - Smart parameter auto-fill
```rust
AleEvent {
    timestamp: 2025-10-03T14:23:41Z,
    event_type: "PARAMETER_INFERRED",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    endpoint: "create_subscription",
    metadata: {
        parameter_name: "customer_id",
        inferred_value: "cus_xyz789",
        source: "previous_call_result",  // or "context", "default", "pattern"
        previous_endpoint: "create_customer",
        confidence: 0.98,
    },
    prev_hash: "...",
    hash: "..."
}
```

**OAUTH_REFRESH** - Automatic OAuth token refresh
```rust
AleEvent {
    timestamp: 2025-10-03T14:25:00Z,
    event_type: "OAUTH_REFRESH",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    metadata: {
        auth_type: "oauth2",
        refresh_trigger: "token_expiring_in_5min",  // or "token_expired", "manual"
        refresh_success: true,
        new_token_expires_at: "2025-10-04T14:25:00Z",
        transparent: true,  // no user intervention required
    },
    prev_hash: "...",
    hash: "..."
}
```

**VERSION_MIGRATED** - Automatic API version migration
```rust
AleEvent {
    timestamp: 2025-10-03T14:30:00Z,
    event_type: "VERSION_MIGRATED",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    metadata: {
        old_endpoint: "POST /v1/charges",
        new_endpoint: "POST /v2/payment_intents",
        migration_reason: "v1_deprecated",
        parameter_mapping: {"amount": "amount_decimal (converted)"},
        automatic: true,
    },
    prev_hash: "...",
    hash: "..."
}
```

**RESPONSE_NORMALIZED** - Schema harmonization applied
```rust
AleEvent {
    timestamp: 2025-10-03T14:23:41Z,
    event_type: "RESPONSE_NORMALIZED",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    endpoint: "create_customer",
    metadata: {
        normalization_rules_applied: 3,
        field_mappings: [
            {"from": "email_address", "to": "email"},
            {"from": "id", "to": "customer_id"},
        ],
        type_coercions: [{"field": "balance", "from": "string", "to": "number"}],
    },
    prev_hash: "...",
    hash: "..."
}
```

**MULTI_API_WORKFLOW** - Cross-API orchestration executed
```rust
AleEvent {
    timestamp: 2025-10-03T14:35:00Z,
    event_type: "MULTI_API_WORKFLOW",
    workflow_id: "wf_abc123",
    metadata: {
        workflow_name: "create_customer_charge_notify",
        steps: [
            {"integration": "stripe", "endpoint": "create_customer", "status": "success"},
            {"integration": "stripe", "endpoint": "create_charge", "status": "success"},
            {"integration": "sendgrid", "endpoint": "send_email", "status": "success"},
            {"integration": "twilio", "endpoint": "send_sms", "status": "success"},
        ],
        atomic: true,
        duration_ms: 1234,
        rollback_executed: false,
    },
    prev_hash: "...",
    hash: "..."
}
```

### 1. Lifecycle Events

**INTEGRATE** - New integration setup (for APIs without official MCP servers)
```rust
AleEvent {
    timestamp: 2025-10-03T14:20:45Z,
    event_type: "INTEGRATE",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    metadata: {
        api_name: "Twilio",
        base_url: "https://api.twilio.com",
        auth_type: "basic",
        endpoints_discovered: 87,
        spec_version: "2010-04-01",
        has_official_mcp: false,  // NEW: Track if API has official MCP server
        source: "kindly_catalog"  // NEW: "kindly_catalog", "custom_spec", "discovered"
    },
    prev_hash: "0000000000000000000000000000000000000000000000000000000000000000", // Genesis
    hash: "4c5d8e9f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c"
}
```

**MCP_OBSERVE** - Start monitoring official MCP server
```rust
AleEvent {
    timestamp: 2025-10-03T14:20:45Z,
    event_type: "MCP_OBSERVE",
    observer_id: "obs_stripe_monitor",
    metadata: {
        mcp_server_name: "stripe",
        monitor_interval_seconds: 60,
        purpose: "health_monitoring"  // observe-only, doesn't execute
    },
    prev_hash: "...",
    hash: "..."
}
```

**EXTEND_MCP** - Auto-generate tools for endpoints missing from official MCP (NEW)
```rust
AleEvent {
    timestamp: 2025-10-03T14:25:00Z,
    event_type: "EXTEND_MCP",
    extension_id: "ext_stripe_advanced",
    metadata: {
        mcp_server_name: "stripe",
        official_endpoints: 23,
        kindlyapi_endpoints: 687,
        total_coverage: 710,
        spec_url: "https://raw.githubusercontent.com/stripe/openapi/master/openapi/spec3.json",
        coverage_improvement: "3.2% → 100%",
        sample_missing_endpoints: ["create_coupon", "create_dispute", "list_refunds"]
    },
    prev_hash: "...",
    hash: "..."
}
```

**DELETE** - Integration removed
```rust
AleEvent {
    timestamp: 2025-10-03T16:45:12Z,
    event_type: "DELETE",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    metadata: {
        reason: "user_requested",
        total_calls: 1234,
        audit_entries_archived: 892
    },
    prev_hash: "...",
    hash: "..."
}
```

---

### 2. Execution Events

**CALL_START** - API call initiated (optional, Pro tier)
```rust
AleEvent {
    timestamp: 2025-10-03T14:23:41.123456Z,
    event_type: "CALL_START",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    endpoint: "POST /v1/charges",
    params_hash: "sha256:a3f9d8e71c2b5f4a9d8e71c2b5f4a9d8e71c2b5f4a",
    idempotency_key: Some("idem_xyz789"),
    retry_attempt: 0,
    prev_hash: "...",
    hash: "..."
}
```

**CALL_SUCCESS** - API call completed successfully
```rust
AleEvent {
    timestamp: 2025-10-03T14:23:41.265890Z,
    event_type: "CALL_SUCCESS",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    endpoint: "POST /v1/charges",
    status_code: 201,
    latency_us: 142456, // 142.456ms
    body_hash: "sha256:7bc4e3a2f9d8e71c2b5f4a9d8e71c2b5f4a9d8e",
    cache_hit: false,
    breaker_level: 0, // L0
    prev_hash: "...",
    hash: "a3f9d8e71c2b5f4a9d8e71c2b5f4a9d8e71c2b5f4a9d8e71c2b5f4a1d2e3c4b"
}
```

**CALL_ERROR** - API call failed
```rust
AleEvent {
    timestamp: 2025-10-03T14:23:35.789012Z,
    event_type: "CALL_ERROR",
    integration_id: "int_x7y8z9a0b1c2d3e4",
    endpoint: "POST /2010-04-01/Accounts/.../Messages",
    status_code: 401,
    error_code: "AUTH_EXPIRED",
    error_message: "Authentication token has expired",
    latency_us: 234123,
    retry_attempt: 0,
    will_retry: false, // Auth error, no retry
    prev_hash: "...",
    hash: "2d5e9c3b7a8f1e2d4c5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b"
}
```

**CALL_RETRY** - Retrying after transient failure
```rust
AleEvent {
    timestamp: 2025-10-03T14:24:05.456789Z,
    event_type: "CALL_RETRY",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    endpoint: "POST /v1/charges",
    retry_attempt: 2,
    backoff_ms: 4000, // Exponential backoff
    original_error: "timeout",
    prev_hash: "...",
    hash: "..."
}
```

---

### 3. Health Events

**BREAKER_FLIP** - Circuit breaker level change
```rust
AleEvent {
    timestamp: 2025-10-03T14:22:15.234567Z,
    event_type: "BREAKER_FLIP",
    integration_id: "int_x7y8z9a0b1c2d3e4",
    from_level: "L0_NORMAL",
    to_level: "L3_PAUSED",
    cause: "AUTH_FAIL_THRESHOLD", // 5 auth errors in 1 minute
    error_count: 5,
    window_ms: 60000,
    dwell_ms: 300000, // Stay paused for 5 minutes
    prev_hash: "...",
    hash: "9f8e7d6c5b4a3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a0b9c"
}
```

**BREAKER_RECOVER** - Breaker level improved
```rust
AleEvent {
    timestamp: 2025-10-03T14:27:15.345678Z,
    event_type: "BREAKER_RECOVER",
    integration_id: "int_x7y8z9a0b1c2d3e4",
    from_level: "L3_PAUSED",
    to_level: "L0_NORMAL",
    trigger: "manual_update", // User ran update_integration
    prev_hash: "...",
    hash: "..."
}
```

**DRIFT_DETECT** - API response differs from spec
```rust
AleEvent {
    timestamp: 2025-10-03T14:21:08.567890Z,
    event_type: "DRIFT_DETECT",
    integration_id: "int_abc123def456",
    endpoint: "GET /repos/{owner}/{repo}/issues",
    drift_type: "field_missing", // Expected field not in response
    expected_field: "assignees[].avatar_url",
    snapshot_current: 1,
    snapshot_fallback: 2, // Trying alternate spec
    prev_hash: "...",
    hash: "6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d"
}
```

**DRIFT_RESOLVE** - Alternate snapshot worked
```rust
AleEvent {
    timestamp: 2025-10-03T14:21:09.123456Z,
    event_type: "DRIFT_RESOLVE",
    integration_id: "int_abc123def456",
    endpoint: "GET /repos/{owner}/{repo}/issues",
    snapshot_used: 2,
    status_code: 200,
    message: "Alternate snapshot successful, updating primary",
    prev_hash: "...",
    hash: "..."
}
```

---

### 4. Configuration Events

**UPDATE_AUTH** - Credentials updated
```rust
AleEvent {
    timestamp: 2025-10-03T14:28:00.000000Z,
    event_type: "UPDATE_AUTH",
    integration_id: "int_x7y8z9a0b1c2d3e4",
    auth_type: "bearer",
    test_result: "success",
    prev_hash: "...",
    hash: "..."
}
```

**UPDATE_POLICY** - Security policy changed
```rust
AleEvent {
    timestamp: 2025-10-03T15:00:00.000000Z,
    event_type: "UPDATE_POLICY",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    changes: {
        allowed_endpoints: "+3", // Added 3 new patterns
        rate_limit_per_minute: "60 → 100"
    },
    prev_hash: "...",
    hash: "..."
}
```

**SPEC_REFRESH** - OpenAPI spec re-fetched
```rust
AleEvent {
    timestamp: 2025-10-03T16:00:00.000000Z,
    event_type: "SPEC_REFRESH",
    integration_id: "int_a1b2c3d4e5f6g7h8",
    old_version: "2024-09-30",
    new_version: "2024-10-01",
    endpoints_added: 2,
    endpoints_removed: 0,
    breaking_changes: false,
    prev_hash: "...",
    hash: "..."
}
```

---

## Capsule Architecture

### AIS-128 (API Integration State)

**Purpose**: Real-time health summary per integration (read by TUI, MCP tools)

**Layout** (128 bits, cache-aligned):
```
W0 (head): commit:1 | ver:8 | health:2 | breaker_level:2 | drift:1 | auth_valid:1 | ...
W1 (body): rate_used_min:24 | rate_limit_min:24 | last_error_ts:32 | last_success_ts:32
```

**Fields**:
- `health`: 0=L0, 1=L1, 2=L2, 3=L3 (maps to ACB-64)
- `breaker_level`: Same as health (redundant for fast access)
- `drift`: 1 if drift detected in last 5 minutes
- `auth_valid`: 1 if last test_auth succeeded
- `rate_used_min`: Calls in current minute (0-16M)
- `rate_limit_min`: Configured limit per minute
- `last_error_ts`: Unix timestamp (seconds since epoch)
- `last_success_ts`: Unix timestamp

**Update Frequency**: Every call_endpoint (single-writer)

**Readers**: TUI dashboard (1s poll), get_health tool (on-demand)

---

### ACR-256 (API Call Result)

**Purpose**: Detailed result from each call_endpoint (ephemeral, not persisted)

**Layout** (256 bits):
```
W0 (head): commit:1 | ver:8 | status_code:16 | error_code:16 | ...
W1: latency_us:32 | breaker_level:2 | cache_hit:1 | retries:4 | drift:1 | ...
W2: body_hash:64 (first 64 bits of SHA-256)
W3: params_hash:64 (for idempotency checking)
```

**Lifecycle**: Created per call, written to ALE-128, then discarded

---

### ALE-128 (Audit Ledger Entry)

**Purpose**: Tamper-evident log of all events (persistent, hash-chained)

**Layout** (128 bits = 16 bytes):
```
W0 (high 64): prev_hash:32 | timestamp:32 (seconds since epoch)
W1 (low 64):  event_type:8 | integration_id:32 | metadata:24
```

**Storage**: Append-only file, one entry per line (16 bytes binary or 32 hex chars)

**Verification**: Linear scan, recompute hash chain, O(n) but fast (1M entries/sec)

---

### ET-1kB (Epoch Tile)

**Purpose**: Crash-safe checkpoint of system state (every 60s)

**Layout** (1024 bytes):
```
Bytes 0-63:    Header (timestamp, tile_seq, prev_tile_hash, integrations_count)
Bytes 64-319:  Integration summaries (AIS-128 snapshot × 16 integrations)
Bytes 320-575: Rate limit windows (per-minute, per-hour, per-day counters × 16)
Bytes 576-831: Last ALE-128 hashes per integration (64 bytes × 4 integrations)
Bytes 832-1023: Reserved (future use)
```

**Update Frequency**: Every 60s by background writer

**On Startup**: Read latest ET-1kB, restore AIS-128 capsules, resume ALE-128 chain

---

## Retention Policies

### Free Tier (24h retention)
- **ALE-128**: Keep last 24h of events (~2-10K entries depending on usage)
- **ET-1kB**: Keep last 24 tiles (24 hours at 60s intervals)
- **ACR-256**: Not persisted (ephemeral)

**Cleanup**: Daily job deletes entries older than 24h

### Pro Tier (7d retention)
- **ALE-128**: Keep last 7d (~10-50K entries)
- **ET-1kB**: Keep last 168 tiles (7 days)
- **Hosted Dashboard**: Stream events to cloud for visualization

**Cleanup**: Weekly job deletes entries older than 7d

### Enterprise Tier (90d+ retention)
- **ALE-128**: Configurable retention (90d, 1y, indefinite)
- **ET-1kB**: Configurable checkpoint frequency (10s, 60s, 5m)
- **Compliance**: Immutable storage, WORM file systems

**Cleanup**: Policy-driven (legal hold, compliance rules)

---

## Friendly Error Templates

### AUTH_EXPIRED

**Template**:
```
❌ Authentication Expired

Integration: {api_name} ({integration_id})
Last Success: {last_success_time} ago

What happened:
  Your API token has expired or been revoked. This is common for
  tokens with 24-hour lifespans (Twilio, OpenAI, etc.).

How to fix:
  1. Get a new token from {api_name} dashboard
  2. Run: update_integration("{integration_id}", auth: {{ token: "new_token" }})
  3. Verify: test_auth("{integration_id}")

Circuit breaker:
  Integration paused (L3) to prevent further failures.
  Will auto-resume after auth update.

📚 Docs: https://docs.kindly.api/errors/auth-expired
```

---

### RATE_LIMIT_EXCEEDED

**Template**:
```
⚠️ Rate Limit Exceeded

Integration: {api_name}
Rate Used: {used} / {limit} requests per {window}
Resets At: {reset_time} ({seconds_remaining}s remaining)

What happened:
  You've reached the rate limit for this integration.
  KindlyAPI's circuit breaker reduced quality to L1 (degraded).

What's happening now:
  - New calls are being queued with backoff
  - Lower-priority calls may be dropped
  - High-priority calls continue with delays

How to fix:
  1. Wait {seconds_remaining}s for rate window to reset (automatic)
  2. Increase rate budget: update_integration(..., rate_budget: {{ per_minute: 100 }})
  3. Upgrade {api_name} plan for higher limits

📊 View rate status: get_rate_status("{integration_id}")
```

---

### DRIFT_DETECTED

**Template**:
```
🔄 API Drift Detected

Integration: {api_name}
Endpoint: {endpoint}
Issue: Expected field '{field_name}' not in response

What happened:
  The API response doesn't match the OpenAPI spec. This could mean:
  - API was updated without version change
  - Spec is outdated
  - Response varies by region/tier

What KindlyAPI did:
  ✓ Automatically tried alternate spec snapshot #{snapshot_num}
  {status} {result_message}

Next steps:
  - If successful: Primary spec will be updated automatically
  - If failed: Run spec_refresh to fetch latest OpenAPI spec

Circuit breaker: No impact (L0 maintained)

📚 Docs: https://docs.kindly.api/errors/drift-detected
```

---

### BREAKER_PAUSED

**Template**:
```
🛑 Circuit Breaker Paused

Integration: {api_name}
Level: L3 (PAUSED)
Cause: {cause_description}
Since: {duration} ago
Auto-recovery: {eta} (if applicable)

What happened:
  KindlyAPI detected {error_count} consecutive errors within {window_ms}ms
  and paused this integration to prevent cascading failures.

Current state:
  - All calls to {api_name} are blocked
  - Integration health monitoring continues
  - Cached responses may be served (if available)

How to fix:
  1. Check integration health: get_health("{integration_id}")
  2. Address root cause: {suggested_action}
  3. Manual recovery: update_integration(...) or test_auth(...)
  4. Breaker will auto-recover after {dwell_time}

📊 View breaker history: get_call_history("{integration_id}", filter: "BREAKER")
```

---

### TIMEOUT

**Template**:
```
⏱️ Request Timeout

Integration: {api_name}
Endpoint: {endpoint}
Timeout: {timeout_ms}ms
Actual: Request did not complete

What happened:
  The API did not respond within the configured timeout period.
  This could indicate:
  - API is overloaded
  - Network connectivity issues
  - Endpoint is processing a large dataset

What KindlyAPI did:
  {retry_status}

How to fix:
  1. Retry with longer timeout: call_endpoint(..., options: {{ timeout_ms: 60000 }})
  2. Check API status: {api_status_url}
  3. Adjust default timeout: update_integration(..., options: {{ timeout_ms: 30000 }})

Circuit breaker: {breaker_action}

⚡ Current timeout: {current_timeout_ms}ms | Suggested: {suggested_timeout_ms}ms
```

---

## Monitoring Queries

### Get Health (Real-Time)

**Input**: `integration_id`

**Capsule Reads**:
1. Load AIS-128 (single atomic read, ~5-20ns)
2. Load ACB-64 (breaker state, ~5-20ns)
3. Total: <100ns for complete health check

**Output**:
```json
{
  "health": "L0_NORMAL",
  "breaker_level": 0,
  "rate_status": {
    "used_per_minute": 45,
    "limit_per_minute": 60,
    "remaining": 15
  },
  "last_error": null,
  "last_success": "2025-10-03T14:23:41Z",
  "drift_detected": false
}
```

---

### Get Call History (Audit Trail)

**Input**: `integration_id`, `limit`, `filter`

**Storage Reads**:
1. Scan ALE-128 append-only file (sequential read)
2. Filter by integration_id and event_type
3. Return last N entries
4. Verify hash chain integrity

**Output**:
```json
{
  "entries": [
    { "timestamp": "...", "event": "CALL_SUCCESS", "hash": "...", ... },
    { "timestamp": "...", "event": "CALL_ERROR", "hash": "...", ... }
  ],
  "total": 1234,
  "chain_valid": true
}
```

---

### Verify Audit Chain (Integrity Check)

**Algorithm**:
```rust
fn verify_chain(entries: &[AleEvent]) -> Result<(), ChainMismatch> {
    let mut prev_hash = GENESIS_HASH; // All zeros

    for (i, entry) in entries.iter().enumerate() {
        // Verify prev_hash matches
        if entry.prev_hash != prev_hash {
            return Err(ChainMismatch { index: i, expected: prev_hash, actual: entry.prev_hash });
        }

        // Recompute hash
        let computed_hash = compute_event_hash(entry);
        if computed_hash != entry.hash {
            return Err(ChainMismatch { index: i, expected: computed_hash, actual: entry.hash });
        }

        prev_hash = entry.hash;
    }

    Ok(())
}
```

**Performance**: ~1M entries/sec on modern hardware (sequential scan + SHA-256)

---

## Alerting (Pro Tier)

### Alert Triggers

**Breaker Flip to L2/L3**:
- Trigger: ACB-64 level change event
- Notify: Webhook, email, Slack
- Message: Include integration, cause, suggested fix

**High Error Rate**:
- Trigger: >10% error rate over 5 minutes
- Notify: Warning (not critical)
- Message: Include top error codes

**Drift Detected**:
- Trigger: DRIFT_DETECT event
- Notify: Info (automatically handled)
- Message: Include snapshot rotation status

**Rate Limit Approaching**:
- Trigger: >90% of rate limit used
- Notify: Warning
- Message: Include reset time, suggest increase

---

### Alert Channels

**Webhook** (HTTP POST):
```json
{
  "alert_type": "BREAKER_FLIP",
  "integration_id": "int_x7y8z9a0b1c2d3e4",
  "api_name": "Twilio",
  "from_level": "L0_NORMAL",
  "to_level": "L3_PAUSED",
  "cause": "AUTH_EXPIRED",
  "timestamp": "2025-10-03T14:22:15Z",
  "suggested_action": "Update credentials and run test_auth"
}
```

**Email** (Transactional):
- Subject: `[KindlyAPI] {alert_type}: {api_name} ({integration_id})`
- Body: Friendly error template (see above)

**Slack** (Bot Message):
```
🛑 Circuit Breaker Paused: Twilio

Integration: Twilio (int_x7y8z9...)
Cause: 5 consecutive auth errors
Action: Update credentials

[View Details] [Fix Now]
```

---

## Performance Characteristics

**Policy Checks** (call_endpoint hot path):
- Endpoint allowlist: <10ns (bitmap check)
- Rate budget: <20ns (atomic compare)
- Auth valid: <10ns (cache-local timestamp check)
- **Total: <100ns (p99)**

**Health Checks** (get_health):
- AIS-128 read: 5-20ns
- ACB-64 read: 5-20ns
- **Total: <50ns (p99)**

**Audit Writes** (per call):
- ALE-128 append: 200-500ns (buffered I/O)
- Non-blocking: Queued to background writer

**Audit Verification** (verify_chain):
- 1M entries: ~1 second (sequential scan + SHA-256)
- 10K entries: ~10ms (typical 24h retention)

**ET-1kB Checkpoints**:
- Write: 1-2ms (1KB sequential write)
- Frequency: Every 60s (low overhead)
- Recovery: <10ms on startup (read latest tile)

---

## Testing Strategy

### Unit Tests
- ALE-128 hash chain verification
- ACB-64 level transitions
- AIS-128 capsule packing/unpacking
- ET-1kB tile serialization

### Integration Tests
- Full event flow: call_endpoint → ACR-256 → ALE-128 → ET-1kB
- Drift detection → snapshot rotation
- Breaker flip → audit trail
- Startup recovery from ET-1kB

### Property Tests
- Hash chain never breaks (tamper detection)
- Timestamps monotonically increase
- Rate counters never exceed limits
- Breaker levels follow state machine (L0→L1→L2→L3, never skip)

### Performance Tests (B32 Framework)
- Policy checks: <100ns (p99)
- Health checks: <50ns (p99)
- Audit writes: <500ns (p99)
- Chain verification: 1M entries/sec

---

## Security Considerations

**Audit Trail Immutability**:
- ALE-128 append-only file (no in-place updates)
- Hash chain makes tampering detectable
- File permissions: 0600 (owner read/write only)
- Optional: Write to WORM storage (Enterprise)

**Secrets in Events**:
- Never log full auth tokens (hash only)
- Truncate sensitive params (credit cards, SSNs)
- Configurable PII redaction rules

**Chain Verification**:
- Run full verification daily (background job)
- Alert on any chain mismatch
- Include verification result in health checks

**Clock Skew**:
- Require monotonic timestamps in ALE-128
- Detect clock rewind on startup
- Use `clock_gettime(CLOCK_MONOTONIC)` for ordering

---

## Compliance & Regulations

**GDPR (Data Protection)**:
- User can request audit log export (MCP tool: `export_audit_log`)
- User can request deletion (MCP tool: `delete_integration`)
- Audit deletion events (GDPR_DELETE) are logged but data is purged

**SOC 2 (Security Controls)**:
- Tamper-evident audit trail (ALE-128 hash chain)
- Regular integrity verification
- Immutable storage (append-only files)
- Access controls (file permissions, encryption at rest)

**HIPAA (Healthcare)**:
- Audit trail of all PHI access (call_endpoint events)
- Retention: 6 years (configurable via Enterprise tier)
- Encryption: At rest (OS keychain) and in transit (TLS)

**PCI-DSS (Payment Card)**:
- No storage of full card numbers (hash only)
- Audit trail of payment API calls
- Retention: 1 year minimum (default for Pro/Enterprise)

---

## Future Enhancements

**Distributed Monitoring** (Post-MVP):
- Aggregate ALE-128 streams from multiple hosts
- Distributed ET-1kB consensus (RAFT or similar)
- Global health dashboard (all integrations, all hosts)

**AI-Powered Insights** (Pro/Enterprise):
- Anomaly detection (unusual latency, error patterns)
- Auto-tuning rate budgets based on usage
- Predictive breaker flips (trend analysis)

**Advanced Alerting**:
- PagerDuty integration
- Custom alert rules (user-defined thresholds)
- Alert suppression (avoid fatigue)

**Metric Aggregation**:
- Prometheus exporter (expose AIS-128 as metrics)
- Grafana dashboard templates
- InfluxDB integration for long-term storage
