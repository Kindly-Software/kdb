# KindlyAPI v1: Execution Runtime Product Specification

**Version**: 1.0
**Date**: 2025-10-03
**Status**: MVP Specification
**Positioning**: Zapier for LLMs - Universal API adapter that connects the 99.9% of APIs without MCP servers + extends the 0.1% that do

---

## Executive Summary

**What**: An MCP server that acts as a universal adapter for APIs with **intelligent generation** that makes APIs feel **native to LLMs** - not just parsing specs, but understanding workflows, inferring parameters, and orchestrating multi-API operations.

**Not**: A code generator, API documentation search tool, or replacement for official MCP servers.

**Core Innovation - Intelligent MCP Generation (~7,000 lines)**:
KindlyAPI doesn't just parse OpenAPI specs - it **understands** APIs through 10 sophisticated features:
1. **Intelligent Endpoint Relationships**: Auto-detects "create_customer → create_subscription" dependencies
2. **Smart Parameter Inference**: "You just created customer X, I'll use that customer_id"
3. **Multi-API Orchestration**: Coordinates Stripe + SendGrid + Twilio in one atomic operation
4. **Automatic Error Recovery**: OAuth token refresh, endpoint migration, intelligent retries
5. **Response Normalization**: Stripe `{id: "cus_123"}` vs PayPal `{customer_id: "123"}` → consistent format
6. **Intelligent Caching**: POST /customers/{id} invalidates GET /customers/{id}
7. **OAuth Flow Automation**: "Click to authorize" browser flow with auto-refresh
8. **API Version Migration**: Auto-migrates deprecated endpoints to new versions
9. **Composite Tool Generation**: High-level business operations from low-level endpoints
10. **Cross-API Intelligence**: "People who use Stripe also use SendGrid" suggestions

**Foundation - Capsule Runtime (Already Built)**:
The execution runtime leverages the existing Primitives workspace (28 working crates: ACB-64, ALE-128, ET-1kB, AIS-128, etc.) for bulletproof reliability:
- Deterministic execution (p99≈median)
- Tamper-evident logs
- Zero-overhead security (<100ns)

**Three-Tier Coverage**:
1. **Tier 1 - Official MCP Servers (Vendor-maintained)**: Use Stripe's official MCP for 23 core endpoints
2. **Tier 2 - MCP Server Extensions (Fill the gaps)**: KindlyAPI extends Stripe with 687 missing endpoints + intelligent features
3. **Tier 3 - Long Tail (No official MCP)**: 100,000+ APIs without any MCP support + intelligent orchestration

**The Incomplete MCP Server Problem**: Even APIs WITH official MCP servers only expose 10-30% of their total API surface. Example: Stripe official MCP has ~23 endpoints, but Stripe's actual REST API has 710+ endpoints. Gap: 687 endpoints (97%) NOT accessible via official MCP.

**Positioning**: Like Zapier connects apps without native integrations (7M users, $5B valuation), KindlyAPI connects APIs without MCP servers AND extends incomplete MCP servers with **LLM-native intelligence**. Same playbook, AI era.

**Differentiator**:
- **Intelligence over reliability**: Other tools just parse specs. KindlyAPI makes APIs feel native to LLMs.
- **Complement, don't compete**: Stripe has official MCP? Great, use it for core operations. KindlyAPI extends it with 687 missing endpoints + intelligent workflows.
- **Extension value**: Official MCP servers cover 10-30% → KindlyAPI gives you 100% coverage + smart orchestration
- **Network effects**: More APIs integrated = more valuable (aiming for 1,000+ community catalog like Zapier's 7,000 apps)
- **Enterprise wedge**: Big companies have 500+ internal APIs, zero MCP servers. KindlyAPI ingests all specs, generates intelligent tools.
- **Foundation that enables magic**: Capsule runtime (already built, reliable) makes intelligent features bulletproof

---

## User Journey

### Developer Setup (One-Time, 2 Minutes)
```
1. Install: `npm install -g @kindly/mcp-server` or `cargo install kindly-mcp`
2. Configure: Add to Claude Desktop / Cursor MCP config
3. Auth: Store API keys in local vault (encrypted at rest)
```

### LLM Workflow (Every Interaction)
```
Developer: "Create a Stripe subscription for user@example.com"

Claude (via MCP):
  1. search_apis("payment processing") → [stripe, paypal, square]
  2. integrate_api("stripe", auth: ENV["STRIPE_KEY"]) → integration_id
  3. call_endpoint(integration_id, "create_subscription", {
       customer_email: "user@example.com",
       price_id: "price_123",
       payment_method: "pm_456"
     })
  4. [RUNTIME: ACB-64 breaker check → ACI-512 intent capsule → Execute → ACR-256 result → ALE-128 audit]
  5. → Success: {subscription_id: "sub_789", status: "active"}

Claude to User: "✓ Subscription created (sub_789). Next billing: 2025-11-03."
```

**No code generation. No manual client setup. Direct execution with built-in guarantees.**

---

## Core Architecture

### Capsule-Based Execution Flow

```
┌─────────────────────────────────────────────────────────────┐
│ LLM (Claude/ChatGPT) via MCP                                │
└─────────────────┬───────────────────────────────────────────┘
                  │ call_endpoint(integration_id, endpoint, params)
                  ▼
┌─────────────────────────────────────────────────────────────┐
│ KindlyAPI Runtime (Rust, lockfree)                          │
├─────────────────────────────────────────────────────────────┤
│ 1. Load AIS-128 (API Integration State)                     │
│    ├─ health:2 (L0-L3), rate_used:24, last_error:16         │
│    └─ drift_detected:1, auth_valid:1                        │
│                                                              │
│ 2. Check ACB-64 (Circuit Breaker) → L0-L3                   │
│    └─ If L3 (pause): Return cached/degraded response        │
│                                                              │
│ 3. Policy Check (<100ns)                                    │
│    ├─ Endpoint allowed? (security allowlist)                │
│    ├─ Rate budget OK? (rate_used < rate_limit)              │
│    └─ Auth valid? (not expired)                             │
│                                                              │
│ 4. Build ACI-512 (API Call Intent)                          │
│    └─ endpoint_id, params_hash, retry_policy, timeout_ms    │
│                                                              │
│ 5. Execute HTTP Call (with retry/backoff)                   │
│    └─ On failure: Try alternate spec snapshot               │
│                                                              │
│ 6. Publish ACR-256 (API Call Result)                        │
│    └─ status, latency_us, body_hash, error_code             │
│                                                              │
│ 7. Write ALE-128 (Audit Ledger Entry)                       │
│    └─ Tamper-evident log: prev_hash → event → new_hash      │
│                                                              │
│ 8. Update AIS-128 + ET-1kB (health checkpoint)              │
│    └─ Increment rate_used, update last_error if needed      │
└─────────────────┬───────────────────────────────────────────┘
                  │ Result (success/error + friendly message)
                  ▼
┌─────────────────────────────────────────────────────────────┐
│ LLM receives structured result + actionable error message   │
└─────────────────────────────────────────────────────────────┘
```

## Intelligent MCP Generation (Primary Innovation)

**The Magic: ~7,000 Lines of Sophisticated Logic**

### 1. Intelligent Endpoint Relationships (~800 lines)
- **Auto-detect dependencies**: Analyzes OpenAPI spec to find "create_customer requires email" → "create_subscription requires customer_id"
- **Generate composite tools**: `create_customer_and_subscribe(email, plan_id)` combines both operations atomically
- **Workflow pattern recognition**: Detects common sequences (create → update → delete) and suggests optimizations

### 2. Smart Parameter Inference (~600 lines)
- **Context-aware auto-fill**: "You just created customer X (cus_123), I'll use that customer_id for the subscription"
- **Test/production mode detection**: Automatically uses test API keys when in development
- **Smart defaults**: Infers common parameters (currency: "usd", timeout: 30s) from API patterns

### 3. Multi-API Orchestration (~1000 lines)
- **Cross-API workflows**: `create_customer(Stripe) → charge(Stripe) → send_receipt(SendGrid) → sms(Twilio)` as single command
- **Atomic multi-step operations**: All-or-nothing execution with automatic rollback on failure
- **Distributed transaction coordination**: Ensures consistency across multiple API calls

### 4. Automatic Error Recovery (~500 lines)
- **OAuth token refresh**: Transparent refresh when tokens expire (no user intervention)
- **Endpoint deprecation migration**: "API v1 deprecated → auto-migrate to v2"
- **Intelligent retry strategies**: Different backoff per error type (auth: no retry, rate limit: exponential backoff)

### 5. Response Normalization (~400 lines)
- **Schema harmonization**: Stripe `{id: "cus_123"}` vs PayPal `{customer_id: "123"}` → consistent `{id: "...", provider: "..."}`
- **Type coercion**: String "123" vs number 123 → normalized to expected type
- **Field mapping**: Automatically maps `email_address` vs `email` vs `userEmail` to standard schema

### 6. Intelligent Caching (~700 lines)
- **Smart invalidation rules**: POST /customers/{id} invalidates GET /customers/{id}
- **Cross-endpoint invalidation**: POST /customers/{id}/cards invalidates GET /customers/{id}
- **LRU cache with freshness guarantees**: Configurable staleness tolerance per endpoint

### 7. OAuth Flow Automation (~900 lines)
- **Browser-based authorization**: "Click to authorize" flow with local callback server
- **PKCE support**: Secure OAuth 2.0 with Proof Key for Code Exchange
- **Automatic token refresh**: Background refresh before expiration (no user interruption)

### 8. API Version Migration (~600 lines)
- **Auto-migrate deprecated endpoints**: "POST /v1/charges deprecated → use POST /v2/payment_intents"
- **Parameter mapping across versions**: Old `amount` (cents) vs new `amount_decimal` (string) → automatic conversion
- **Breaking change detection**: Alerts when migration is unsafe

### 9. Composite Tool Generation (~800 lines)
- **High-level business operations**: `setup_subscription_business` = create_customer + create_product + create_price + create_subscription + setup_billing
- **Workflow templates**: Pre-built sequences for common use cases (e-commerce checkout, user onboarding, etc.)
- **Customizable orchestration**: User-defined composite tools from primitive endpoints

### 10. Cross-API Intelligence (~500 lines)
- **Usage pattern analysis**: "90% of users who integrate Stripe also integrate SendGrid within 7 days"
- **Multi-API workflow suggestions**: "You're using Stripe for payments → consider Twilio for SMS notifications"
- **Compatibility detection**: Warns when APIs have conflicting requirements (OAuth scopes, rate limits, etc.)

---

## Capsule Runtime (Foundation - Already Built)

**The Reliability Layer: Leverages Existing Primitives Workspace (28 Crates)**

This runtime is the **foundation that makes intelligence bulletproof** - deterministic, tamper-evident, and zero-overhead.

### Novel Capsules (Built on Atomic Capsule v1.1)

**ACI-512 (API Call Intent)** - Single-writer request capsule
- `endpoint_id:64 | params_hash:64 | auth_method:8 | retry_policy:8`
- `timeout_ms:16 | priority:4 | idempotency_key:128 | workflow_id:64 | ...`

**ACR-256 (API Call Result)** - Single-writer response capsule
- `status_code:16 | latency_us:20 | breaker_level:2 | body_hash:64`
- `error_code:16 | cache_hit:1 | retries:4 | drift_detected:1 | normalized:1 | ...`

**AIS-128 (API Integration State)** - Health summary per API
- `health:2 | rate_used:24 | rate_limit:24 | last_error_ts:32`
- `drift_detected:1 | auth_valid:1 | last_success_ts:32 | workflow_count:16 | ...`

**AIA-1024 (API Integration Analytics)** - NEW: Per-endpoint metrics
- `endpoint_id:64 | call_count:32 | error_count:32 | p50_latency:20 | p99_latency:20`
- `cache_hit_rate:16 | normalization_applied:1 | workflow_invocations:32 | ...`

**AMC-512 (API Marketplace Catalog Entry)** - NEW: Community catalog metadata
- `api_id:64 | popularity_score:32 | quality_score:32 | community_rating:16`
- `has_workflows:1 | oauth_required:1 | normalization_rules:16 | ...`

**AEH-2048 (API Extension Heuristics)** - NEW: Gap analysis for MCP extensions
- `official_endpoint_count:16 | total_endpoint_count:16 | gap_percentage:16`
- `workflow_coverage:16 | normalization_rules:32 | composite_tools:32 | ...`

**Reused from Primitives:**
- **ACB-64**: Circuit breaker (L0 normal → L3 pause)
- **ALE-128**: Tamper-evident audit ledger
- **ET-1kB**: Crash-safe health snapshots (24h/7d retention)

---

## 18 MCP Tools (Phased Rollout)

### Phase 1: Core Execution (MVP - Week 2)
1. `integrate_api` - Setup API integration with auth (for APIs WITHOUT official MCP servers)
2. `call_endpoint` - Execute API call with guarantees
3. `get_health` - Current health + breaker state
4. `get_call_history` - Recent audit trail

### Phase 2: Reliability (Week 3)
5. `list_integrations` - All configured APIs
6. `get_integration_info` - Endpoints, schema, rate limits
7. `validate_request` - Pre-flight check (dry-run)
8. `explain_error` - Friendly error + suggested fix
9. `test_auth` - Validate credentials
10. `update_integration` - Change config/auth

### Phase 3: Advanced (Week 4+)
11. `search_api_catalog` - Browse 100+ pre-integrated APIs (community catalog)
12. `batch_call` - Efficient multi-request execution
13. `get_rate_status` - Usage vs budget
14. `delete_integration` - Cleanup

### Phase 4: Monitoring & Pro Features (Post-MVP)
15. `observe_mcp_server` - Monitor official MCP servers (Stripe, GitHub, etc.) for health
16. `suggest_alternatives` - "Stripe is down, switch to PayPal?" (failover suggestions)
17. `ai_discover` - "Find endpoints to manage subscriptions" (Sonnet 4.5)
18. `optimize_calls` - Batching/caching suggestions
19. `export_client` - Generate code if needed (optional)
20. `create_template` - Reusable integration configs
21. `extend_mcp_server` - Auto-generate tools for endpoints missing from official MCP servers (NEW)

---

## Success Metrics (SLOs)

**Time-to-First-Working-Call (TTFWC)**: ≤ 2 minutes
- Measured: `integrate_api` → `call_endpoint` → success

**Spec Fetch Accuracy**: ≥ 98%
- Triangulation + probing selects working spec on first try

**MCP Extension Coverage**: ≥ 90%
- Percentage of total API surface accessible after extension (NEW)
- Example: Stripe 23 official + 687 KindlyAPI = 710/710 (100%)

**Execution Reliability**: p99 ≈ median latency
- Deterministic runtime via capsule architecture

**Security Overhead**: <100ns policy checks
- Single-read allowlist/rate budget validation

**Audit Trail**: 100% tamper-evident
- Every call logged in ALE-128 hash chain

**Integration Health**: <1min drift detection
- Automatic snapshot failover, friendly error messages

---

## Security Model

**Zero-Overhead Policy Enforcement:**
- Allowlist: Endpoint in approved set? (single bitmap check)
- Rate Budget: `rate_used < rate_limit`? (single atomic compare)
- Auth: Token valid & not expired? (cache-local check)

**All checks: One cache-line read (~5-20ns per check).**

**Secrets Management:**
- Encrypted at rest (local vault, OS keychain)
- Never leave process boundary
- Rotation: Update via `update_integration`

**Audit:**
- Every call → ALE-128 entry (prev_hash, event, new_hash)
- ET-1kB snapshots every 60s for recovery
- Linear verification catches tampering

---

## Free vs. Pro vs. Enterprise

| Feature | Free | Pro | Enterprise |
|---------|------|-----|------------|
| Integrations | 3 active | Unlimited | Unlimited |
| call_endpoint | ✓ | ✓ | ✓ |
| Circuit breaker | Basic (L0-L3) | Advanced + auto-tune | Custom policies |
| Audit retention | 24h | 7d | 90d |
| AI features | - | ai_discover, optimize_calls | + Custom AI agents |
| Monitoring | CLI/TUI | Dashboard + alerts | Private deployment + SLA |
| Rate limits | Shared pool | Dedicated | Custom |
| Support | Community | Email + docs | Dedicated + on-call |

**Pricing Intent:**
- Free: $0 (self-hosted MCP server)
- Pro: $20/mo per developer
- Enterprise: Custom (starts $500/mo for teams)

---

## MVP Acceptance Criteria (Week 4)

**Functional:**
- [ ] `integrate_api` + `call_endpoint` working for 5 real APIs (Stripe, GitHub, OpenAI, Twilio, SendGrid)
- [ ] ACB-64 breaker flips to L1-L3 under error/rate conditions
- [ ] ALE-128 audit trail verifiable (linear hash chain check)
- [ ] Friendly error messages for common failures (auth, rate limit, drift)

**Performance:**
- [ ] TTFWC ≤ 2min (integrate → first successful call)
- [ ] Policy checks <100ns (95th percentile)
- [ ] p99 latency ≈ 1.2x p50 (deterministic tails)

**Quality:**
- [ ] Zero warnings (cargo build --release)
- [ ] 90%+ test coverage (unit + integration)
- [ ] B32 fair benchmarks vs direct HTTP client

**Docs:**
- [ ] MCP setup guide (Claude Desktop + Cursor)
- [ ] API reference for 18 tools
- [ ] TUI user guide

---

## Non-Goals (Avoid Scope Creep)

- ❌ Visual workflow builder (stay code-native)
- ❌ Public API marketplace (focus on execution runtime)
- ❌ Long-running gateways (zero-overhead = on-path only)
- ❌ GraphQL/gRPC in v1 (OpenAPI/REST only for MVP)

---

## Technology Stack

**Runtime:** Rust (nightly, leveraging existing Primitives workspace)
**Capsules:** atomic_breaker, atomic_ledger_entry, atomic_epoch_tile (reuse)
**MCP:** stdio transport (JSON-RPC)
**TUI:** ratatui + crossterm (optional, Phase 3)
**HTTP:** reqwest (with connection pooling)
**Spec Parsing:** openapiv3, serde_json
**Auth Storage:** OS keychain (keyring-rs)

---

## Risk Mitigation

**Risk: OpenAPI specs are broken/incomplete**
→ Mitigation: Triangulation (fetch 3+ sources) + probing + snapshot fallback

**Risk: APIs change without notice (drift)**
→ Mitigation: Checksum validation + automatic snapshot rotation + friendly errors

**Risk: Rate limiting causes breaker flips**
→ Mitigation: Adaptive backoff + rate budget tracking + L1-L3 degradation

**Risk: LLMs pass malicious params**
→ Mitigation: Schema validation + allowlist enforcement + audit trail

---

## Relationship with Official MCP Servers

**Philosophy**: Complement, don't compete. Three-tier value proposition.

**Tier 1: Official MCP Servers (Best, when available)**:
- **Example**: Stripe official MCP (23 core endpoints: charges, customers, subscriptions)
- **Use them**: Vendor-maintained, always up-to-date
- **KindlyAPI role**: Monitor health, observe-only

**Tier 2: MCP Server Extensions (Fill the gaps)** - NEW:
- **Problem**: Official MCP servers only expose 10-30% of total API surface
- **Example**: Stripe official MCP has 23 endpoints, but Stripe REST API has 710+ endpoints
- **Gap**: 687 endpoints NOT accessible (coupons, disputes, refunds, balance, payouts, radar, tax, etc.)
- **Solution**: KindlyAPI auto-generates tools for missing endpoints by parsing OpenAPI spec
- **Result**: Users get BOTH official MCP (23 endpoints) AND KindlyAPI extension (687 endpoints) = 100% coverage

**Tier 3: Long Tail (No official MCP)**:
- **Example**: Twilio, SendGrid, Shopify (87-200 endpoints each)
- **Problem**: 100,000+ APIs without any MCP support
- **Solution**: KindlyAPI auto-generates tools from OpenAPI spec
- **Result**: LLMs can call any API, not just the 10-20 with official MCP servers

**Example Workflow**:
```
✓ Stripe* (Official MCP): 23 endpoints (charges, customers, subscriptions) - use for common operations
✓ Stripe+ (KindlyAPI Extension): 687 endpoints (coupons, disputes, refunds, etc.) - for advanced features
✓ Twilio (Long Tail): 87 endpoints - no official MCP, KindlyAPI is the ONLY option
✓ SendGrid (Long Tail): 200 endpoints - no official MCP
✓ Internal CRM API: KindlyAPI ingests company's OpenAPI spec
```

**Before/After**:
- **Before KindlyAPI**: Claude can call 23 Stripe endpoints (official MCP only)
- **After KindlyAPI**: Claude can call 710 Stripe endpoints (23 official + 687 KindlyAPI extension)

**Result**: LLMs get 100% API coverage, not just 10-30%.

---

## Competitive Positioning

| Competitor | Model | Limitation | KindlyAPI Advantage |
|------------|-------|------------|---------------------|
| Zapier/n8n | No-code workflows | Not LLM-native, manual setup | MCP-native, auto-generates from OpenAPI |
| Official MCP servers | Vendor-maintained | Only 10-20 APIs covered | 100,000+ APIs via auto-generation |
| OpenAPI generators | Static code gen | Manual updates, no monitoring | Dynamic runtime, auto-drift handling |
| Kong/Apigee | API gateway (proxy) | Adds latency, requires proxy | Zero-overhead on-path checks |
| Postman/Insomnia | Manual testing | Not automated | LLM-driven execution |

**Bottom Line**: KindlyAPI is "Zapier for LLMs" - connecting the long tail of APIs that don't have native MCP support.

**Zapier Parallel**:
- Zapier: 7,000 app integrations, 7M users, $5B valuation
- KindlyAPI: Target 1,000+ API integrations (community catalog), same network effects playbook
