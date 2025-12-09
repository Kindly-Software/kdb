# KindlyAPI MVP Plan: 4-Week Intelligent Generation Roadmap

**Goal:** Ship "Intelligent MCP Generation" - make APIs feel native to LLMs through smart workflows, parameter inference, and multi-API orchestration

**Philosophy:** **Intelligence over reliability** - capsule runtime (already built, 28 crates) is the foundation; intelligent generation (~7,000 lines) is the innovation

**Target:**
- **Week 1-2**: Intelligent endpoint relationships + smart parameter inference (~1,400 lines)
- **Week 2**: OAuth automation + intelligent caching + error recovery (~2,100 lines)
- **Week 3**: Multi-API orchestration + composite tools (~1,800 lines)
- **Week 4**: Cross-API intelligence + polish + documentation (~1,700 lines)
- **Total**: ~7,000 lines of intelligent generation logic

**Foundation (Already Built):**
- Capsule runtime integration is trivial (existing Primitives workspace: ACB-64, ALE-128, ET-1kB, AIS-128)
- Add 3 new capsules: AIA-1024 (analytics), AMC-512 (catalog), AEH-2048 (extension heuristics)

---

## Week 1: Intelligent Endpoint Relationships (~800 lines) + Smart Parameter Inference (~600 lines)

### Milestone: Context-aware API calls with auto-detected workflows

**Deliverables:**
1. MCP server scaffold (stdio transport, JSON-RPC)
2. OpenAPI spec parser with dependency analysis (openapiv3 crate + custom relationship detection)
3. `integrate_api` tool with workflow detection
4. `call_endpoint` tool with smart parameter inference from context
5. **Endpoint relationship graph**: Auto-detect "create_customer → create_subscription" dependencies
6. **Parameter inference engine**: "You just created customer X (cus_123), I'll use that customer_id"
7. Basic capsule integration (AIS-128, ACR-256, **AIA-1024** for workflow metrics)
8. 1 API validated (Stripe with workflow detection)

---

### Day 1: Project Scaffold + MCP Server

**Tasks:**
- [ ] Create `kindly_api` crate in Primitives workspace
- [ ] Add to `Cargo.toml` workspace members
- [ ] Depend on: atomic_breaker, atomic_ledger_entry, atomic_epoch_tile
- [ ] Implement MCP stdio transport (read JSON-RPC from stdin, write to stdout)
- [ ] Skeleton for 18 MCP tools (stubs only)
- [ ] Basic logging (tracing crate)

**Files:**
```
kindly_api/
├── Cargo.toml
├── src/
│   ├── main.rs              # MCP server entry point
│   ├── lib.rs               # Core library
│   ├── mcp/
│   │   ├── mod.rs           # MCP protocol handler
│   │   ├── transport.rs     # stdio JSON-RPC
│   │   └── tools/           # 18 MCP tool implementations (stubs)
│   │       ├── integrate_api.rs
│   │       ├── call_endpoint.rs
│   │       ├── get_health.rs
│   │       └── ... (15 more)
│   ├── runtime/
│   │   ├── mod.rs           # Capsule runtime (black box)
│   │   └── capsules.rs      # AIS-128, ACR-256, ACI-512
│   └── api/
│       ├── mod.rs
│       ├── spec.rs          # OpenAPI spec parsing
│       └── executor.rs      # HTTP client wrapper
└── tests/
    └── integration_tests.rs
```

**Acceptance:**
- [X] `cargo run --bin kindly-mcp` starts MCP server
- [X] Responds to `{"method": "tools/list"}` with 18 tool stubs
- [X] Exits gracefully on Ctrl+C

---

### Day 2: OpenAPI Spec Parsing

**Tasks:**
- [ ] Implement spec fetching (HTTP GET via reqwest)
- [ ] Parse OpenAPI 3.0/3.1 (openapiv3 crate)
- [ ] Extract endpoints (path, method, operation_id, parameters)
- [ ] Basic spec validation (required fields present)
- [ ] Store spec in local cache (~/.kindly-api/specs/)

**Files:**
```rust
// src/api/spec.rs
pub struct ApiSpec {
    pub base_url: String,
    pub version: String,
    pub endpoints: Vec<Endpoint>,
}

pub struct Endpoint {
    pub path: String,
    pub method: HttpMethod,
    pub operation_id: Option<String>,
    pub parameters: Vec<Parameter>,
    pub request_body: Option<RequestBody>,
}

pub fn fetch_spec(url: &str) -> Result<ApiSpec, SpecError>;
pub fn parse_openapi(content: &str) -> Result<ApiSpec, SpecError>;
```

**Test:**
- [X] Fetch Stripe OpenAPI spec (https://raw.githubusercontent.com/stripe/openapi/master/openapi/spec3.json)
- [X] Parse 342 endpoints correctly
- [X] Extract parameters for POST /v1/charges

**Acceptance:**
- [X] Parse real Stripe OpenAPI spec without errors
- [X] Extract >300 endpoints

---

### Day 3: `integrate_api` Tool

**Tasks:**
- [ ] Implement `integrate_api` MCP tool
- [ ] Generate unique integration_id (aid_96 pattern: time | node | counter | class)
- [ ] Store integration config (auth, base_url, endpoints)
- [ ] Encrypt auth credentials (keyring-rs for OS keychain)
- [ ] Initialize AIS-128 capsule (health: L0, rate_used: 0)
- [ ] Write INTEGRATE event to ALE-128

**Files:**
```rust
// src/mcp/tools/integrate_api.rs
pub async fn integrate_api(params: IntegrateApiParams) -> Result<IntegrateApiResult, McpError> {
    // 1. Fetch spec (or lookup known API)
    let spec = fetch_spec(&params.api_identifier)?;

    // 2. Generate integration_id
    let integration_id = generate_integration_id();

    // 3. Store config
    let config = IntegrationConfig {
        integration_id: integration_id.clone(),
        api_name: spec.api_name,
        base_url: spec.base_url,
        auth: params.auth,
        endpoints: spec.endpoints,
        created_at: Utc::now(),
    };
    store_config(&config)?;

    // 4. Encrypt auth
    encrypt_auth(&integration_id, &params.auth)?;

    // 5. Initialize AIS-128
    let ais = AIS128::new(integration_id.clone(), HealthLevel::L0);
    ais.publish();

    // 6. Write ALE-128
    write_audit_event(AleEvent::Integrate {
        integration_id: integration_id.clone(),
        api_name: spec.api_name,
        endpoints_discovered: spec.endpoints.len(),
    })?;

    Ok(IntegrateApiResult {
        integration_id,
        api_name: spec.api_name,
        base_url: spec.base_url,
        endpoints_discovered: spec.endpoints.len(),
        health: HealthLevel::L0,
    })
}
```

**Test:**
- [X] Call `integrate_api("stripe", auth: {...})`
- [X] Verify integration_id generated
- [X] Verify auth stored in OS keychain (encrypted)
- [X] Verify AIS-128 capsule created (health: L0)
- [X] Verify ALE-128 INTEGRATE event written

**Acceptance:**
- [X] `integrate_api` returns integration_id
- [X] Auth stored securely (OS keychain)
- [X] AIS-128 readable via atomic_breaker patterns

---

### Day 4-5: `call_endpoint` Tool (HTTP Execution)

**Tasks:**
- [ ] Implement `call_endpoint` MCP tool
- [ ] Load integration config + decrypt auth
- [ ] Read AIS-128 for health check (skip if L3)
- [ ] Check ACB-64 breaker (L0-L3 decision)
- [ ] Validate endpoint exists in spec
- [ ] Build HTTP request (method, URL, headers, body)
- [ ] Execute via reqwest (with timeout)
- [ ] Parse response (status, body, headers)
- [ ] Write ACR-256 result capsule
- [ ] Write ALE-128 audit event (CALL_SUCCESS or CALL_ERROR)
- [ ] Update AIS-128 (rate_used++, last_success/error_ts)

**Files:**
```rust
// src/mcp/tools/call_endpoint.rs
pub async fn call_endpoint(params: CallEndpointParams) -> Result<CallEndpointResult, McpError> {
    // 1. Load integration config
    let config = load_config(&params.integration_id)?;

    // 2. Read AIS-128 (health check)
    let ais = AIS128::read(&params.integration_id)?;
    if ais.health == HealthLevel::L3 {
        return Err(McpError::BreakerPaused { retry_after: ais.dwell_remaining() });
    }

    // 3. Check ACB-64 breaker
    let breaker = ACB64::read(&params.integration_id)?;
    if breaker.level >= 3 {
        return Err(McpError::BreakerPaused { retry_after: breaker.dwell_ms });
    }

    // 4. Find endpoint in spec
    let endpoint = config.find_endpoint(&params.endpoint)?;

    // 5. Build HTTP request
    let req = build_request(&config, &endpoint, &params.params)?;

    // 6. Execute (with timeout)
    let start = Instant::now();
    let resp = execute_http(req, params.options.timeout_ms).await?;
    let latency_us = start.elapsed().as_micros() as u64;

    // 7. Parse response
    let status = resp.status().as_u16();
    let body = resp.bytes().await?;
    let body_hash = sha256_first_64(&body);

    // 8. Write ACR-256
    let acr = ACR256::new(status, latency_us, body_hash, breaker.level);
    acr.publish();

    // 9. Write ALE-128
    if status >= 200 && status < 300 {
        write_audit_event(AleEvent::CallSuccess {
            integration_id: params.integration_id.clone(),
            endpoint: params.endpoint,
            status,
            latency_us,
        })?;
    } else {
        write_audit_event(AleEvent::CallError {
            integration_id: params.integration_id.clone(),
            endpoint: params.endpoint,
            status,
            error_code: derive_error_code(status),
        })?;
    }

    // 10. Update AIS-128
    ais.increment_rate_used();
    if status >= 200 && status < 300 {
        ais.update_last_success(Utc::now());
    } else {
        ais.update_last_error(Utc::now(), derive_error_code(status));
    }
    ais.publish();

    Ok(CallEndpointResult {
        success: status >= 200 && status < 300,
        status_code: status,
        body: serde_json::from_slice(&body).ok(),
        metadata: Metadata {
            latency_us,
            breaker_level: breaker.level,
            retries: 0,
            cache_hit: false,
            audit_hash: acr.hash(),
        },
    })
}
```

**Test:**
- [X] Call `call_endpoint` for Stripe POST /v1/charges
- [X] Verify HTTP request sent correctly
- [X] Verify response parsed
- [X] Verify ACR-256 capsule written
- [X] Verify ALE-128 CALL_SUCCESS event
- [X] Verify AIS-128 updated (rate_used++, last_success)

**Acceptance:**
- [X] End-to-end: integrate_api → call_endpoint → success
- [X] 5 successful Stripe API calls in a row
- [X] All capsules written correctly (AIS-128, ACR-256, ALE-128)

---

### Day 6-7: Testing + Polish

**Tasks:**
- [ ] Integration test: Stripe end-to-end (integrate → call 5 endpoints → verify audit)
- [ ] Unit tests: Spec parsing, endpoint matching, HTTP building
- [ ] Error handling: Network timeout, auth failure, 4xx/5xx responses
- [ ] Friendly error messages (template system)
- [ ] Basic logging (tracing subscriber)

**Acceptance Tests:**
- [X] T1: integrate_api("stripe") succeeds
- [X] T2: call_endpoint("create_charge") returns 201
- [X] T3: call_endpoint with bad auth returns AUTH_INVALID error
- [X] T4: call_endpoint with network timeout returns TIMEOUT error
- [X] T5: ALE-128 chain verifies correctly (5 events)

**Week 1 Exit Criteria:**
- [X] 1 API (Stripe) working end-to-end
- [X] integrate_api + call_endpoint tools functional
- [X] Basic capsule integration (AIS-128, ACR-256, ALE-128)
- [X] Zero warnings (cargo build --release)
- [X] 10+ integration tests passing

---

## Week 2: OAuth Automation (~900 lines) + Intelligent Caching (~700 lines) + Error Recovery (~500 lines)

### Milestone: Zero-friction OAuth + smart cache invalidation + transparent error handling

**Deliverables:**
1. **OAuth flow automation**: "Click to authorize" browser flow with PKCE support
2. **Automatic token refresh**: Background refresh before expiration (transparent to user)
3. **Intelligent caching**: POST /customers/{id} invalidates GET /customers/{id}
4. **Cross-endpoint invalidation**: POST /customers/{id}/cards invalidates GET /customers/{id}
5. **Smart error recovery**: OAuth refresh, endpoint migration, intelligent retries
6. ACB-64 circuit breaker integration (foundation - already built)
7. Security policy enforcement (foundation - already built)
8. 2 more APIs validated (GitHub with OAuth, OpenAI)
9. `get_health`, `get_call_history` tools

---

### Day 8: Circuit Breaker (ACB-64)

**Tasks:**
- [ ] Integrate atomic_breaker crate
- [ ] Initialize ACB-64 per integration (L0 default)
- [ ] Auto-flip on error thresholds:
  - 5 errors in 1 minute → L1 (degraded)
  - 10 errors in 1 minute → L2 (limited)
  - 15 errors in 1 minute → L3 (paused)
- [ ] Check breaker before each call_endpoint
- [ ] Write BREAKER_FLIP events to ALE-128
- [ ] Auto-recovery after dwell time (5 minutes for L3)

**Files:**
```rust
// src/runtime/breaker.rs
pub struct BreakerManager {
    breakers: HashMap<IntegrationId, ACB64>,
}

impl BreakerManager {
    pub fn check(&self, integration_id: &str) -> Result<(), BreakerError> {
        let breaker = self.breakers.get(integration_id)?;
        match breaker.level() {
            0 => Ok(()),
            1 => Ok(()), // Degraded, but allow
            2 => Ok(()), // Limited, but allow
            3 => Err(BreakerError::Paused { dwell_ms: breaker.dwell_remaining() }),
        }
    }

    pub fn record_success(&mut self, integration_id: &str) {
        let breaker = self.breakers.get_mut(integration_id).unwrap();
        breaker.record_success();
        // Check if level drops (L1→L0, L2→L1, etc.)
    }

    pub fn record_error(&mut self, integration_id: &str, error_code: &str) {
        let breaker = self.breakers.get_mut(integration_id).unwrap();
        breaker.record_error();

        // Check thresholds
        let error_count = breaker.error_count_last_minute();
        let new_level = if error_count >= 15 {
            3 // Paused
        } else if error_count >= 10 {
            2 // Limited
        } else if error_count >= 5 {
            1 // Degraded
        } else {
            0 // Normal
        };

        if new_level != breaker.level() {
            write_audit_event(AleEvent::BreakerFlip {
                integration_id: integration_id.to_string(),
                from_level: breaker.level(),
                to_level: new_level,
                cause: error_code.to_string(),
            }).ok();
            breaker.set_level(new_level);
        }
    }
}
```

**Test:**
- [X] Trigger 5 errors → breaker flips to L1
- [X] Trigger 15 errors → breaker flips to L3
- [X] call_endpoint when L3 returns BreakerPaused error
- [X] Wait 5 minutes → breaker auto-recovers to L0

**Acceptance:**
- [X] ACB-64 auto-flips based on error thresholds
- [X] L3 paused state blocks new calls
- [X] BREAKER_FLIP events logged to ALE-128

---

### Day 9: Security Policy Checks

**Tasks:**
- [ ] Implement security policy system
- [ ] Allowlist: Endpoint must match regex patterns
- [ ] Rate budget: Check rate_used < rate_limit (per minute)
- [ ] Blocked methods: Deny DELETE by default
- [ ] Policy checks <100ns (benchmark with criterion)

**Files:**
```rust
// src/runtime/policy.rs
pub struct SecurityPolicy {
    pub allowed_endpoints: Vec<Regex>,
    pub blocked_methods: HashSet<HttpMethod>,
    pub rate_budget: RateBudget,
}

impl SecurityPolicy {
    pub fn check_endpoint(&self, endpoint: &str, method: &HttpMethod) -> Result<(), PolicyError> {
        // 1. Check blocked methods (<10ns, hashset lookup)
        if self.blocked_methods.contains(method) {
            return Err(PolicyError::MethodBlocked);
        }

        // 2. Check allowlist (<50ns, regex match)
        if !self.allowed_endpoints.iter().any(|re| re.is_match(endpoint)) {
            return Err(PolicyError::EndpointBlocked);
        }

        Ok(())
    }

    pub fn check_rate_budget(&self, ais: &AIS128) -> Result<(), PolicyError> {
        // Single atomic read (<20ns)
        if ais.rate_used() >= self.rate_budget.per_minute {
            return Err(PolicyError::RateLimitExceeded {
                used: ais.rate_used(),
                limit: self.rate_budget.per_minute,
            });
        }

        Ok(())
    }
}
```

**Test:**
- [X] Block DELETE endpoint → PolicyError::MethodBlocked
- [X] Block non-allowlisted endpoint → PolicyError::EndpointBlocked
- [X] Exceed rate limit → PolicyError::RateLimitExceeded
- [X] Benchmark: policy checks <100ns (p99)

**Acceptance:**
- [X] Security policy enforced on every call_endpoint
- [X] Policy checks <100ns (measured via criterion)
- [X] Friendly error messages for policy violations

---

### Day 10: Retry Logic

**Tasks:**
- [ ] Implement retry policy (basic, exponential, adaptive)
- [ ] Retry on transient errors: 429, 500, 503, timeout
- [ ] Do NOT retry on: 401, 403, 404 (permanent errors)
- [ ] Write CALL_RETRY events to ALE-128
- [ ] Exponential backoff: 1s, 2s, 4s, 8s, 16s (max 5 attempts)

**Files:**
```rust
// src/api/executor.rs
pub async fn execute_with_retry(
    req: HttpRequest,
    retry_policy: RetryPolicy,
) -> Result<HttpResponse, ExecutionError> {
    let mut attempt = 0;
    let mut backoff_ms = 1000; // Start at 1 second

    loop {
        attempt += 1;

        match execute_http(req.clone()).await {
            Ok(resp) if resp.status().is_success() => return Ok(resp),
            Ok(resp) if is_retryable(resp.status()) && attempt < 5 => {
                // Retry on 429, 500, 503
                write_audit_event(AleEvent::CallRetry {
                    attempt,
                    backoff_ms,
                    original_error: format!("HTTP {}", resp.status()),
                }).ok();

                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms *= 2; // Exponential backoff
                continue;
            }
            Ok(resp) => return Err(ExecutionError::HttpError(resp.status())),
            Err(e) if is_retryable_error(&e) && attempt < 5 => {
                // Retry on timeout
                write_audit_event(AleEvent::CallRetry {
                    attempt,
                    backoff_ms,
                    original_error: e.to_string(),
                }).ok();

                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms *= 2;
                continue;
            }
            Err(e) => return Err(ExecutionError::NetworkError(e)),
        }
    }
}
```

**Test:**
- [X] Retry on 503 (service unavailable) → succeeds on 2nd attempt
- [X] Do NOT retry on 401 (auth error)
- [X] Exponential backoff: 1s, 2s, 4s verified
- [X] CALL_RETRY events logged

**Acceptance:**
- [X] Transient errors retry automatically (up to 5 attempts)
- [X] Exponential backoff implemented correctly
- [X] Permanent errors fail immediately

---

### Day 11-12: MCP Server Extension + Add 2 More APIs

**Tasks:**
- [ ] Implement `extend_mcp_server` tool (NEW)
- [ ] Detect official MCP server endpoints (via introspection or hardcoded list for Stripe/GitHub)
- [ ] Fetch OpenAPI spec for that API
- [ ] Compare: Which endpoints are NOT in official MCP?
- [ ] Auto-generate tools for missing endpoints
- [ ] Namespace: `stripe_advanced` to distinguish from `stripe` (official)
- [ ] Validate OpenAI API (https://api.openai.com/v1)
- [ ] Validate Twilio API (https://api.twilio.com)
- [ ] Write integration tests for each
- [ ] Document known quirks (auth headers, base URLs)

**Example:**
```rust
// Stripe official MCP has: create_charge, get_customer, create_subscription (23 total)
// KindlyAPI detects: Stripe OpenAPI has 710 endpoints
// Missing: 687 endpoints (create_coupon, create_dispute, list_refunds, etc.)
// Auto-generate: stripe_advanced.create_coupon, stripe_advanced.create_dispute, etc.
```

**Test:**
- [X] Extend Stripe official MCP
- [X] Verify: 23 official endpoints + 687 KindlyAPI endpoints = 710 total
- [X] Call `stripe_advanced.create_coupon` successfully
- [X] Write EXTEND_MCP event to ALE-128

**Acceptance:**
- [X] `extend_mcp_server("stripe")` returns extension_id
- [X] LLM can call both official MCP + KindlyAPI extension
- [X] TUI shows: "Stripe* (23) + Stripe+ (687)"

**Tests per API:**
- [X] T1: integrate_api succeeds
- [X] T2: call_endpoint (read operation) succeeds
- [X] T3: call_endpoint (write operation) succeeds
- [X] T4: Breaker triggers on repeated errors
- [X] T5: Rate limit respected

**Acceptance:**
- [X] 4 APIs total (Stripe + Stripe extension, OpenAI, Twilio) working end-to-end
- [X] `extend_mcp_server` tool functional
- [X] 25+ integration tests passing

---

### Day 13: Health & History Tools

**Tasks:**
- [ ] Implement `get_health` tool (read AIS-128 + ACB-64)
- [ ] Implement `get_call_history` tool (read ALE-128, filter by integration_id)
- [ ] ALE-128 chain verification (linear scan + SHA-256)

**Files:**
```rust
// src/mcp/tools/get_health.rs
pub fn get_health(params: GetHealthParams) -> Result<GetHealthResult, McpError> {
    let ais = AIS128::read(&params.integration_id)?;
    let breaker = ACB64::read(&params.integration_id)?;

    Ok(GetHealthResult {
        integration_id: params.integration_id,
        health: ais.health(),
        breaker_state: BreakerState {
            level: breaker.level(),
            cause: breaker.cause(),
            since: breaker.since(),
            error_count: breaker.error_count(),
        },
        rate_status: RateStatus {
            used_per_minute: ais.rate_used(),
            limit_per_minute: ais.rate_limit(),
        },
        last_success: ais.last_success_ts(),
        drift_detected: ais.drift_detected(),
    })
}
```

```rust
// src/mcp/tools/get_call_history.rs
pub fn get_call_history(params: GetCallHistoryParams) -> Result<GetCallHistoryResult, McpError> {
    let mut entries = Vec::new();
    let file = File::open("~/.kindly-api/audit.ale128")?;

    for line in BufReader::new(file).lines() {
        let entry: AleEvent = bincode::deserialize(&hex::decode(line?)?)?;
        if entry.integration_id == params.integration_id {
            entries.push(entry);
        }
        if entries.len() >= params.limit {
            break;
        }
    }

    // Verify chain
    let chain_valid = verify_chain(&entries).is_ok();

    Ok(GetCallHistoryResult {
        entries,
        total: entries.len(),
        chain_valid,
    })
}
```

**Test:**
- [X] get_health returns correct breaker level
- [X] get_call_history returns last 100 events
- [X] Chain verification detects tampering (modify 1 byte → chain_valid=false)

**Acceptance:**
- [X] get_health <50ns (measured)
- [X] get_call_history processes 10K entries in <10ms
- [X] Chain verification working

---

### Day 14: Week 2 Testing + Polish

**Tasks:**
- [ ] End-to-end test: 4 APIs, 100 calls, verify audit trail
- [ ] Stress test: Trigger breaker flips, verify L0→L3→L0 cycle
- [ ] Performance test: Policy checks <100ns (B32 framework)
- [ ] Error message templates (friendly errors)

**Acceptance Tests:**
- [X] T1: 100 successful calls across 4 APIs
- [X] T2: Breaker flips correctly (L0→L1→L2→L3)
- [X] T3: Policy checks <100ns (p99)
- [X] T4: Retry logic works (3 retries on 503)
- [X] T5: ALE-128 chain valid (100 events)

**Week 2 Exit Criteria:**
- [X] 4 APIs working (Stripe, GitHub, OpenAI, Twilio)
- [X] Circuit breaker functional (ACB-64 auto-flips)
- [X] Security policy enforced (<100ns checks)
- [X] Retry logic working (exponential backoff)
- [X] get_health + get_call_history tools working
- [X] 30+ integration tests passing
- [X] Zero warnings

---

## Week 3: Multi-API Orchestration (~1000 lines) + Composite Tools (~800 lines)

### Milestone: Cross-API workflows + high-level business operations

**Deliverables:**
1. **Multi-API orchestration engine**: Coordinate Stripe + SendGrid + Twilio atomically
2. **Workflow templates**: "create_customer_charge_notify" = Stripe customer → charge → SendGrid receipt → Twilio SMS
3. **Atomic multi-step operations**: All-or-nothing execution with rollback
4. **Composite tool generation**: `setup_subscription_business` combines 5+ API calls
5. **Workflow pattern recognition**: Detects common sequences and suggests optimizations
6. TUI dashboard with **workflow visualization** (relationship graph, parameter flow)
7. API Catalog screen with **workflow templates** (browse 100+ pre-integrated APIs)
8. `observe_mcp_server` tool (monitor official MCP servers)
9. ET-1kB checkpoint system (foundation - already built)
10. 1 more API (Twilio for multi-API workflows)
11. Response normalization (~400 lines): Stripe `{id: "cus_123"}` vs PayPal `{customer_id: "123"}` → consistent format

---

### Day 15-17: TUI Dashboard

**Tasks:**
- [ ] Scaffold TUI app (ratatui + crossterm)
- [ ] Main dashboard screen (integration list, health bars, recent activity)
- [ ] Integration details screen (endpoints, rate limits, errors)
- [ ] Audit log screen (ALE-128 chain with verification)
- [ ] Mouse support (click rows, buttons, tabs)
- [ ] Real-time updates (1s refresh, poll AIS-128)

**Files:**
```
kindly_api/
├── src/
│   ├── bin/
│   │   ├── kindly-mcp.rs     # MCP server
│   │   └── kindly-tui.rs     # TUI dashboard (NEW)
│   └── tui/
│       ├── mod.rs
│       ├── app.rs            # App state machine
│       ├── ui.rs             # Screen rendering
│       ├── events.rs         # Keyboard/mouse handling
│       └── screens/
│           ├── dashboard.rs  # Main screen
│           ├── details.rs    # Integration details
│           └── audit.rs      # Audit log
```

**Acceptance:**
- [X] TUI launches: `kindly-tui`
- [X] Dashboard shows 4 integrations with health bars
- [X] Mouse clicks navigate to details screen
- [X] Audit log shows last 100 events
- [X] Real-time updates (health changes reflect in <2s)

---

### Day 18: ET-1kB Checkpoints

**Tasks:**
- [ ] Integrate atomic_epoch_tile crate
- [ ] Write ET-1kB tile every 60s (background thread)
- [ ] Include: AIS-128 snapshots (all integrations), ALE-128 last hashes
- [ ] Store tiles in ~/.kindly-api/checkpoints/
- [ ] On startup: Read latest ET-1kB, restore capsules

**Files:**
```rust
// src/runtime/checkpoints.rs
pub struct CheckpointManager {
    tile_path: PathBuf,
    writer: TileWriter,
}

impl CheckpointManager {
    pub fn start(interval: Duration) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                Self::write_checkpoint().await;
            }
        });
    }

    pub async fn write_checkpoint() {
        let tile = ET1kB::new();

        // Snapshot all AIS-128 capsules
        for integration in list_integrations() {
            let ais = AIS128::read(&integration.id).unwrap();
            tile.add_ais_snapshot(&integration.id, ais);
        }

        // Store last ALE-128 hashes
        for integration in list_integrations() {
            let last_hash = get_last_audit_hash(&integration.id);
            tile.add_ale_hash(&integration.id, last_hash);
        }

        tile.write("~/.kindly-api/checkpoints/latest.et1kb").await;
    }

    pub fn restore_from_checkpoint() -> Result<(), CheckpointError> {
        let tile = ET1kB::read("~/.kindly-api/checkpoints/latest.et1kb")?;

        // Restore AIS-128 capsules
        for (integration_id, ais_snapshot) in tile.ais_snapshots() {
            let ais = AIS128::from_snapshot(integration_id, ais_snapshot);
            ais.publish();
        }

        Ok(())
    }
}
```

**Test:**
- [X] Write checkpoint after 4 integrations created
- [X] Kill process, restart, verify capsules restored
- [X] ALE-128 chain continuous (no gaps after restart)

**Acceptance:**
- [X] ET-1kB written every 60s
- [X] Startup recovery <10ms
- [X] No data loss on restart

---

### Day 19: Remaining Core Tools

**Tasks:**
- [ ] Implement `list_integrations`
- [ ] Implement `get_integration_info`
- [ ] Implement `validate_request` (dry-run, no execution)
- [ ] Implement `explain_error` (friendly error templates)
- [ ] Implement `test_auth` (execute test endpoint)
- [ ] Implement `update_integration`
- [ ] Implement `delete_integration`

**Acceptance:**
- [X] All 14 core tools working
- [X] Integration tests for each tool

---

### Day 20: Add SendGrid API

**Tasks:**
- [ ] Validate SendGrid API (https://api.sendgrid.com/v3)
- [ ] Integration test suite

**Acceptance:**
- [X] 5 APIs total (Stripe, GitHub, OpenAI, Twilio, SendGrid)
- [X] 40+ integration tests passing

---

### Day 21: Week 3 Testing + Polish

**Tasks:**
- [ ] End-to-end test: TUI + MCP server running concurrently
- [ ] Verify ET-1kB recovery (kill + restart + verify continuity)
- [ ] Polish error messages (friendly templates)
- [ ] Performance validation: TTFWC ≤ 2min (integrate → first call)

**Acceptance Tests:**
- [X] T1: TUI shows real-time health across 5 integrations
- [X] T2: ET-1kB recovery restores all capsules
- [X] T3: TTFWC ≤ 2min (measured for all 5 APIs)
- [X] T4: Policy checks <100ns (p99)
- [X] T5: get_health <50ns (p99)

**Week 3 Exit Criteria:**
- [X] TUI dashboard functional (all 6 screens)
- [X] ET-1kB checkpoints working (60s interval)
- [X] Startup recovery <10ms
- [X] 5 APIs working end-to-end
- [X] 14 core MCP tools implemented
- [X] 50+ integration tests passing
- [X] Zero warnings

---

## Week 4: Cross-API Intelligence (~500 lines) + API Version Migration (~600 lines) + Polish + Launch

### Milestone: Production-ready intelligent MCP generation + public launch

**Deliverables:**
1. **Cross-API intelligence**: "90% of users who integrate Stripe also integrate SendGrid within 7 days"
2. **Multi-API workflow suggestions**: "You're using Stripe for payments → consider Twilio for SMS notifications"
3. **API version migration**: Auto-migrate "POST /v1/charges" (deprecated) → "POST /v2/payment_intents"
4. **Parameter mapping across versions**: Old `amount` (cents) vs new `amount_decimal` (string) → automatic conversion
5. **Breaking change detection**: Alerts when migration is unsafe
6. Complete documentation emphasizing **intelligent features** (parameter inference, workflows, OAuth, normalization)
7. Landing page (kindly.api) with **"Intelligence over reliability"** positioning
8. Free tier enforcement (3 integrations + **3 workflow templates**)
9. CI/CD (GitHub Actions)
10. Packaging (npm, cargo, binaries)
11. Public launch with **intelligent MCP generation** pitch (not just reliability)

---

### Day 22-23: Documentation

**Tasks:**
- [ ] Write MCP setup guide (Claude Desktop, Cursor)
- [ ] API reference for all 18 tools (OpenAPI schema)
- [ ] Architecture docs (capsule runtime, black-box guarantees)
- [ ] FAQ (10 common questions)
- [ ] Troubleshooting guide

**Deliverables:**
- [X] docs.kindly.api site (MkDocs or Docusaurus)
- [X] 5+ guides (setup, quickstart, API reference, architecture, FAQ)

---

### Day 24: Free Tier Enforcement

**Tasks:**
- [ ] Enforce 3 integration limit (soft delete, not hard delete)
- [ ] Enforce 24h audit retention (background cleanup job)
- [ ] Shared rate pool (60 calls/min default)
- [ ] Upgrade prompts in TUI

**Acceptance:**
- [X] 4th integration blocked with upgrade prompt
- [X] Audit logs older than 24h deleted
- [X] Rate limits enforced

---

### Day 25: CI/CD + Packaging

**Tasks:**
- [ ] GitHub Actions workflow (build + test on push)
- [ ] Publish to crates.io (cargo install kindly-mcp)
- [ ] Publish to npm (@kindly/mcp-server)
- [ ] Pre-built binaries (Linux, macOS, Windows via GitHub Releases)

**Acceptance:**
- [X] cargo install kindly-mcp works
- [X] npm install -g @kindly/mcp-server works
- [X] Binaries downloadable from GitHub

---

### Day 26: Landing Page

**Tasks:**
- [ ] Deploy kindly.api (Vercel or Cloudflare Pages)
- [ ] Implement hero section + 4 benefits
- [ ] Pricing table (Free vs Pro vs Enterprise)
- [ ] Installation instructions
- [ ] Analytics (PostHog or Plausible)

**Acceptance:**
- [X] Landing page live at kindly.api
- [X] CTA: "Add to Claude in 60 seconds"

---

### Day 27: Final Testing

**Tasks:**
- [ ] Full regression test suite (all 50+ tests)
- [ ] Performance validation (B32 benchmarks)
- [ ] Security audit (ASSUM framework)
- [ ] User testing (3 beta testers)

**Acceptance:**
- [X] All tests passing
- [X] Benchmarks meet targets (<100ns policy checks, <50ns health)
- [X] Zero critical security issues

---

### Day 28: Launch

**Tasks:**
- [ ] Post on Hacker News ("Show HN: KindlyAPI - Zapier for LLMs, connect any API")
- [ ] Tweet launch announcement (emphasize "99.9% of APIs don't have MCP servers")
- [ ] Post in Claude Discord, r/programming, r/rust
- [ ] Email early access list
- [ ] Monitor feedback (Discord, GitHub issues)

**Launch Message (Hacker News):**
"**Zapier for LLMs: Connect any API + extend any MCP server to 100% coverage**

**The Problem:** Only ~10-20 APIs have official MCP servers. Even those only expose 10-30% of their API.

**Example:** Stripe official MCP has 23 endpoints, but Stripe API has 710 endpoints. Gap: 687 endpoints (97%) not accessible to LLMs.

**Solution:** KindlyAPI provides three-tier coverage:
1. **Use official MCP** for core operations (vendor-maintained)
2. **Extend with KindlyAPI** for missing endpoints (auto-generated from OpenAPI spec)
3. **Long tail coverage** for 100,000+ APIs without any official MCP

**Result:** Stripe (official) 23 + Stripe Advanced (KindlyAPI) 687 = 710 total (100% coverage)

Like Zapier connects apps without integrations (7M users, $5B valuation), we connect APIs without MCP servers AND extend incomplete ones.

Free tier: 3 integrations + 1 MCP extension + unlimited monitoring. $20/mo Pro for unlimited extensions + API Marketplace."

**Launch Checklist:**
- [X] Landing page live (kindly.api with three-tier value proposition)
- [X] MCP server installable (npm, cargo, binary)
- [X] Docs complete (emphasize extension + complementary relationship)
- [X] 5 APIs validated:
  - [X] Stripe official MCP (23 endpoints) + Stripe Advanced (687 endpoints) = 710 total
  - [X] 3 long tail via KindlyAPI (OpenAI, Twilio, SendGrid)
- [X] `extend_mcp_server` tool working
- [X] API Catalog UI (browse 100+ APIs)
- [X] Free tier functional (3 integrations + 1 MCP extension)
- [X] Zero critical bugs
- [X] Acceptance test: "Verify Stripe official MCP + Stripe Advanced (extension) + Twilio (long tail) all work together"

**Week 4 Exit Criteria:**
- [X] Production-ready MVP shipped
- [X] Public launch complete
- [X] 10+ users installed
- [X] Positive feedback from beta testers

---

## MVP Success Metrics (End of Week 4)

**Functional:**
- [X] 5 real APIs working (Stripe, GitHub, OpenAI, Twilio, SendGrid)
- [X] 18 MCP tools (14 core + 4 advanced stubs)
- [X] Circuit breaker (ACB-64 auto-flips)
- [X] Security policy (<100ns checks)
- [X] Audit trail (ALE-128 tamper-evident)
- [X] TUI dashboard (6 screens)
- [X] ET-1kB checkpoints (crash-safe)

**Performance:**
- [X] TTFWC ≤ 2min (integrate → first call)
- [X] Policy checks <100ns (p99)
- [X] Health checks <50ns (p99)
- [X] p99 latency ≈ 1.2x p50 (deterministic)

**Quality:**
- [X] 60+ integration tests passing
- [X] Zero warnings (cargo build --release)
- [X] Zero critical security issues
- [X] B32 benchmarks passing

**Docs:**
- [X] MCP setup guide
- [X] API reference (18 tools)
- [X] Architecture docs
- [X] FAQ + troubleshooting

**Launch:**
- [X] Landing page live
- [X] Installable (npm, cargo, binary)
- [X] Public announcement
- [X] 10+ users

---

## Post-MVP Roadmap (Weeks 5-8)

### Week 5: Advanced Features (Pro Tier Prep)
- [ ] `batch_call` (parallel execution)
- [ ] `search_apis` (basic directory)
- [ ] `get_rate_status` (detailed rate info)
- [ ] Pro tier infrastructure (Stripe billing)

### Week 6: AI Features (Sonnet 4.5 Integration)
- [ ] `ai_discover` (endpoint discovery)
- [ ] `optimize_calls` (batching suggestions)
- [ ] API spec repair (malformed OpenAPI)
- [ ] Pro tier launch ($20/mo)

### Week 7: Web Dashboard (Pro Tier)
- [ ] Dashboard frontend (Next.js + shadcn/ui)
- [ ] Real-time health charts
- [ ] Hosted backend (Rust + Axum)
- [ ] Email + webhook alerts

### Week 8: Enterprise Prep
- [ ] Private deployment docs (Docker, Kubernetes)
- [ ] RBAC system
- [ ] SSO integration (SAML, OAuth)
- [ ] Compliance certifications (SOC 2 prep)

---

## Risk Mitigation

### Risk 1: OpenAPI specs are broken
**Mitigation:**
- Triangulate 3+ sources (official, GitHub, APIDocs.dev)
- Probe endpoints to validate
- Automatic snapshot rotation on drift
- **Fallback:** AI spec repair (Pro tier)

### Risk 2: Capsule architecture is too complex
**Mitigation:**
- Start simple (AIS-128, ACR-256, ALE-128 only)
- Black-box the runtime (hide complexity)
- Progressive enhancement (add capsules as needed)
- **Fallback:** Use standard HashMap if atomics too complex

### Risk 3: Performance targets unrealistic
**Mitigation:**
- Benchmark early (Day 9, Day 14, Day 21, Day 27)
- B32 framework for honest measurement
- Profile hot paths (perf, flamegraphs)
- **Fallback:** Adjust targets if needed (100ns → 200ns still excellent)

### Risk 4: MCP adoption is slow
**Mitigation:**
- Focus on Claude users (largest MCP user base)
- Make installation frictionless (60 seconds)
- Provide value in Free tier (not just trial)
- **Fallback:** Direct API (not just MCP) if needed

### Risk 5: User retention is low
**Mitigation:**
- Make Free tier genuinely useful (3 integrations is enough for most)
- Sticky features: TUI, audit trail, self-healing
- Community building (Discord, GitHub)
- **Fallback:** Adjust pricing/limits based on feedback

---

## Resource Requirements

**Team:**
- 1 senior Rust engineer (full-time, 4 weeks)
- OR: 1 founder (full-time, 6 weeks with learning curve)

**Infrastructure:**
- GitHub (free)
- Vercel or Cloudflare Pages (free tier for landing page)
- Domain: kindly.api (~$10/year)
- CI/CD: GitHub Actions (free for public repos)

**Tools:**
- Rust toolchain (nightly)
- Existing Primitives workspace (atomic_breaker, atomic_ledger_entry, etc.)
- MCP protocol (stdio, no server needed)

**Budget:**
- $0-50 total (domain + maybe hosting)
- All tooling is free/open-source

---

## Definition of Done (MVP)

**Functional Requirements:**
- [X] 5 APIs working end-to-end (Stripe, GitHub, OpenAI, Twilio, SendGrid)
- [X] 18 MCP tools (14 core implemented, 4 advanced stubs)
- [X] Circuit breaker (ACB-64) auto-flips L0→L3
- [X] Security policy (<100ns checks)
- [X] Audit trail (ALE-128 tamper-evident, 24h retention)
- [X] TUI dashboard (real-time health, 6 screens)
- [X] ET-1kB checkpoints (crash-safe recovery)

**Non-Functional Requirements:**
- [X] Performance: TTFWC ≤ 2min, policy checks <100ns, health <50ns
- [X] Quality: 60+ tests, zero warnings, zero critical bugs
- [X] Docs: Setup guide, API reference, architecture, FAQ
- [X] Packaging: npm, cargo, binaries

**Launch Requirements:**
- [X] Landing page live (kindly.api)
- [X] Installable in 60 seconds (Claude Desktop, Cursor)
- [X] Public announcement (Hacker News, Twitter, Discord)
- [X] 10+ beta users

**When all boxes checked: MVP is DONE. Ship it!** 🚀
