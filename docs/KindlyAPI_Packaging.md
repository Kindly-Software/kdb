# KindlyAPI Packaging: Free → Pro → Enterprise

**Pricing Philosophy**: "Intelligent MCP generation for LLMs" - freemium with network effects

**Value Ladder**:
- **Free**: Basic intelligent features (3 workflow templates, smart caching, parameter inference for simple cases)
- **Pro**: All 10 intelligent features (unlimited workflows, multi-API orchestration, OAuth automation, response normalization)
- **Enterprise**: Custom workflow training + multi-tenant orchestration + private intelligent catalog

**Zapier Parallel**: Zapier charges $20/mo (750 tasks), grew to $140M ARR. KindlyAPI: Same freemium playbook, AI era.

---

## Tier Comparison Matrix

| Feature | Free | Pro | Enterprise |
|---------|------|-----|------------|
| **Intelligent MCP Generation** | | | |
| Endpoint relationship detection | ✓ (basic) | ✓ (advanced) | ✓ (custom training) |
| Smart parameter inference | ✓ (simple) | ✓ (full context) | ✓ (cross-session) |
| Multi-API orchestration | 1 workflow | Unlimited workflows | Custom workflows + training |
| Automatic error recovery | Basic retry | OAuth refresh + migration | Custom recovery rules |
| Response normalization | ✓ (basic) | ✓ (full) | ✓ (custom schemas) |
| Intelligent caching | ✓ (basic LRU) | ✓ (smart invalidation) | ✓ (custom rules) |
| OAuth flow automation | Manual | Automatic refresh | PKCE + custom flows |
| API version migration | Manual | Automatic | Automatic + custom mapping |
| Composite tool generation | 3 templates | Unlimited templates | Custom + training |
| Cross-API intelligence | Browse | Suggestions | Custom recommendations |
| **Core Execution (Foundation)** | | | |
| `call_endpoint` with guarantees | ✓ | ✓ | ✓ |
| Circuit breaker (ACB-64) | Basic (L0-L3) | Advanced + auto-tune | Custom policies |
| Deterministic runtime (p99≈median) | ✓ | ✓ | ✓ |
| Zero-overhead security (<100ns) | ✓ | ✓ | ✓ |
| **Integrations** | | | |
| KindlyAPI integrations (APIs without official MCP) | 3 | Unlimited | Unlimited |
| Official MCP monitoring (observe-only) | Unlimited | Unlimited | Unlimited |
| MCP server extensions (fill gaps in official MCPs) | 1 | Unlimited | Unlimited |
| New integrations per week | 3 | Unlimited | Unlimited |
| Spec triangulation + probing | ✓ | ✓ | ✓ |
| Automatic drift detection | ✓ | ✓ | ✓ |
| Snapshot rotation | Manual | Automatic | Automatic + AI repair |
| API Catalog access (community integrations) | Browse only | ✓ (1000+ APIs) | ✓ + Private catalog |
| **Monitoring & Audit** | | | |
| Audit trail (ALE-128 hash chain) | ✓ | ✓ | ✓ |
| Retention period | 24h | 7d | 90d+ (configurable) |
| ET-1kB checkpoints | 60s | 60s | 10s / 60s / 5m (configurable) |
| Real-time health (AIS-128) | ✓ | ✓ | ✓ |
| TUI dashboard | ✓ | ✓ | ✓ + white-label |
| Web dashboard | - | ✓ | ✓ |
| Export audit logs | JSON | JSON, CSV | JSON, CSV, Parquet |
| Chain verification | On-demand | Automatic daily | Automatic + alerts |
| **MCP Tools** | | | |
| `integrate_api`, `call_endpoint` | ✓ | ✓ | ✓ |
| `get_health`, `get_call_history` | ✓ | ✓ | ✓ |
| `list_integrations`, `validate_request` | ✓ | ✓ | ✓ |
| `test_auth`, `update_integration` | ✓ | ✓ | ✓ |
| `explain_error`, `get_rate_status` | ✓ | ✓ | ✓ |
| `delete_integration` | ✓ | ✓ | ✓ |
| `search_apis` (basic) | ✓ | ✓ | ✓ |
| `batch_call` (parallel execution) | - | ✓ | ✓ |
| `ai_discover` (Sonnet 4.5) | - | ✓ | ✓ |
| `optimize_calls` (AI suggestions) | - | ✓ | ✓ |
| `export_client` (code generation) | - | ✓ | ✓ |
| `create_template` (reusable configs) | - | - | ✓ |
| **Rate Limits** | | | |
| Rate budget enforcement | Shared pool | Dedicated | Custom |
| Default calls per minute | 60 | 600 | Custom |
| Burst allowance | 1.5x | 2x | Custom |
| **Alerting** | | | |
| Email alerts | - | ✓ | ✓ |
| Webhook alerts | - | ✓ | ✓ |
| Slack integration | - | ✓ | ✓ |
| PagerDuty integration | - | - | ✓ |
| Custom alert rules | - | - | ✓ |
| **AI Features (Sonnet 4.5)** | | | |
| AI-powered endpoint discovery | - | ✓ | ✓ |
| Call optimization suggestions | - | ✓ | ✓ |
| API spec repair (malformed docs) | - | ✓ | ✓ |
| Natural language queries | - | ✓ | ✓ |
| Custom AI agents | - | - | ✓ |
| **Deployment** | | | |
| Self-hosted MCP server | ✓ | ✓ | ✓ |
| Hosted MCP server | - | ✓ (single-tenant) | ✓ (single-tenant) |
| Private deployment | - | - | ✓ (VPC, on-prem) |
| High availability | - | - | ✓ (multi-region) |
| **Security** | | | |
| Local secret storage (OS keychain) | ✓ | ✓ | ✓ |
| Encrypted audit logs | ✓ | ✓ | ✓ |
| RBAC (role-based access) | - | - | ✓ |
| SSO (SAML, OAuth) | - | - | ✓ |
| Compliance certifications | - | - | SOC 2, HIPAA, PCI-DSS |
| Custom security policies | - | - | ✓ |
| **Support** | | | |
| Community support (GitHub) | ✓ | ✓ | ✓ |
| Email support | - | ✓ (48h response) | ✓ (4h response) |
| Video onboarding | - | - | ✓ |
| Dedicated Slack channel | - | - | ✓ |
| SLA guarantee | - | - | ✓ (99.9% uptime) |
| On-call support | - | - | ✓ (optional add-on) |
| **Pricing** | | | |
| Monthly cost | **$0** | **$20/dev** | **Custom** (starts $500/mo) |
| Annual discount | - | 2 months free | Negotiable |

---

## Free Tier Deep Dive

### What You Get
- **3 active KindlyAPI integrations** (for APIs without official MCP servers)
- **Unlimited official MCP monitoring** (observe-only, e.g., track Stripe health)
- **1 MCP server extension** (NEW: extend Stripe OR GitHub OR one other official MCP with missing endpoints)
- **3 new integrations per week** (create/delete counts as "new")
- **All core MCP tools** (15/21 tools, excluding AI/marketplace features)
- **Circuit breaker** (basic L0-L3 levels)
- **Audit trail** (24h retention, ALE-128 hash chain)
- **TUI dashboard** (real-time health monitoring - official MCP + extensions + KindlyAPI)
- **Zero-overhead security** (<100ns policy checks)
- **Deterministic runtime** (p99≈median latency)
- **API Catalog browsing** (view 1000+ pre-integrated APIs, can't add without Pro)

### What You Don't Get
- AI-powered features (`ai_discover`, `optimize_calls`)
- API Marketplace access (`search_api_catalog` add function)
- Batch execution (`batch_call`)
- Code generation (`export_client`)
- Observability for official MCP servers (`observe_mcp_server`, `suggest_alternatives`)
- Web dashboard (TUI only)
- Extended audit retention (24h limit)
- Email/webhook alerts

### Ideal For
- Solo developers building side projects
- Small teams evaluating KindlyAPI
- Low-volume API usage (< 10K calls/month)
- Projects with < 3 API integrations

### Upgrade Triggers
- Need > 3 integrations
- Need more than 1 MCP extension (e.g., extend both Stripe AND GitHub)
- Want AI-powered endpoint discovery
- Need audit logs beyond 24 hours
- Want hosted dashboard for team visibility

### Limitations
**Hard Limits:**
- 3 integrations (enforced)
- 3 new integrations/week (rate limit)
- 24h audit retention (auto-delete after 24h)
- TUI only (no web dashboard access)

**Soft Limits (shared pool):**
- 60 calls/minute per integration (best effort)
- 1,000 calls/hour per integration
- 10,000 calls/day per integration

**Breaker:** If you exceed soft limits, ACB-64 flips to L1 (degraded) with backoff. No hard cutoff.

---

## Pro Tier Deep Dive

### What You Get (Everything in Free +)
- **Unlimited KindlyAPI integrations** (no 3-integration cap for long tail APIs)
- **Unlimited MCP server extensions** (NEW: extend Stripe + GitHub + all others with missing endpoints)
- **API Marketplace** (Pro tier exclusive)
  - `search_api_catalog`: Browse + add from 1,000+ pre-integrated APIs
  - Community contributions (like Zapier's 7,000 app catalog)
  - One-click integration from catalog
- **Official MCP observability**
  - `observe_mcp_server`: Monitor official MCP servers (Stripe, GitHub, etc.)
  - `suggest_alternatives`: "Stripe down? Switch to PayPal" (failover suggestions)
  - Unified health dashboard (all APIs, official MCP + extensions + KindlyAPI)
- **AI-powered features** (Sonnet 4.5)
  - `ai_discover`: "Find endpoints to manage subscriptions"
  - `optimize_calls`: Batching/caching suggestions
  - API spec repair (malformed OpenAPI docs)
- **Batch execution** (`batch_call` for parallel requests)
- **Code generation** (`export_client` for Rust/Python/TypeScript/Go)
- **7-day audit retention** (vs 24h in Free)
- **Web dashboard** (hosted, accessible from browser)
- **Email + webhook alerts** (breaker flips, high error rates, MCP server down)
- **Automatic spec refresh** (daily check for API changes)
- **Dedicated rate budget** (not shared pool)
- **Email support** (48h response time)

### Pricing
- **$20/month per developer**
- **Annual:** $200/year (save $40, ~2 months free)
- **Teams:** 5+ devs get 10% discount

### Ideal For
- Professional developers with 5-10 API integrations
- Agencies managing multiple client APIs
- Teams that want AI-powered insights
- Projects needing audit compliance (7d retention)
- Users who prefer web dashboard over TUI

### Rate Limits
- **600 calls/minute** per integration (10x Free)
- **10,000 calls/hour** per integration
- **200,000 calls/day** per integration
- **Burst:** 2x rate (1,200 calls in 1 minute, then throttle)

### AI Features Usage
- **ai_discover**: 100 queries/month (resets monthly)
- **optimize_calls**: 50 analyses/month
- **API spec repair**: 20 repairs/month

**Overage:** $0.50 per additional AI query (billed monthly)

### Web Dashboard Features
- Real-time health across all integrations
- Historical latency/error charts (7d)
- Breaker flip timeline
- Drift detection history
- Export reports (PDF, CSV)
- Shareable read-only links (for clients)

### Upgrade Triggers (Free → Pro)
- Need > 3 integrations
- Need more than 1 MCP extension (extend multiple official MCPs)
- Want AI endpoint discovery ("find endpoints for X")
- Need audit logs beyond 24h (compliance)
- Want web dashboard for team visibility
- Need batch execution for efficiency
- Want code generation fallback

---

## Enterprise Tier Deep Dive

### What You Get (Everything in Pro +)
- **Private MCP deployment** (your VPC, on-prem, or Kubernetes)
- **Private API Catalog** (NEW - Enterprise game-changer)
  - Ingest company's 500+ internal APIs (OpenAPI specs)
  - Auto-generate MCP tools for all internal services
  - Team-wide catalog (no public exposure)
  - White-label dashboard
- **Custom security policies** (beyond default allowlist/rate limits)
- **SSO integration** (SAML, OAuth, Okta, Auth0)
- **RBAC** (role-based access control for teams)
- **90-day+ audit retention** (configurable, up to indefinite)
- **Compliance certifications** (SOC 2, HIPAA, PCI-DSS ready)
- **Custom breaker policies** (beyond L0-L3, custom thresholds)
- **High availability** (multi-region, 99.9% SLA)
- **PagerDuty integration** + custom alert rules
- **Reusable templates** (`create_template` for team sharing)
- **Custom AI agents** (trained on your internal APIs)
- **Dedicated support** (4h response, video onboarding, Slack channel)
- **Optional on-call** ($500/mo add-on)

### Pricing
- **Starts at $500/month** (5-seat minimum)
- **Volume discounts:**
  - 10-25 devs: 15% off
  - 25-50 devs: 25% off
  - 50+ devs: Custom pricing
- **Annual commitment:** 10% discount (1 month free)

### Ideal For
- Engineering teams (10+ developers)
- Fintech, healthcare, e-commerce (compliance requirements)
- Enterprises with internal API ecosystems
- Companies needing private deployment
- Teams requiring SSO + RBAC
- High-volume API usage (10M+ calls/month)

### Rate Limits
- **Custom** (no hard limits, negotiate based on usage)
- **Default:** 6,000 calls/minute per integration (100x Free)
- **Burst:** 3x rate (18,000 calls in 1 minute)

### Deployment Options

**1. Self-Hosted (Private MCP Server)**
- Deploy in your VPC (AWS, GCP, Azure)
- On-premises (bare metal, VMware)
- Kubernetes cluster (Helm chart provided)
- Docker Compose (single-server)

**2. Hosted (Single-Tenant)**
- KindlyAPI manages infrastructure
- Your data stays in your region
- Dedicated resources (no noisy neighbors)
- 99.9% SLA

**3. Hybrid**
- MCP server in your VPC
- Dashboard hosted by KindlyAPI
- Encrypted audit logs synced to cloud

### Security Add-Ons

**Custom Security Policies** ($100/mo):
- Conditional allowlists (by time, IP, user role)
- Advanced rate limiting (per endpoint, per user)
- Request/response filtering (PII redaction)
- Custom error handling rules

**Compliance Audit** ($5,000 one-time):
- SOC 2 Type II readiness assessment
- HIPAA compliance validation
- PCI-DSS audit support
- Includes remediation recommendations

**Dedicated HSM** ($300/mo):
- Hardware security module for key storage
- FIPS 140-2 Level 3 certified
- Required for highest-security environments

### Support SLA

**Standard (Included):**
- Email support: 4h response (business hours)
- Video onboarding: 1 session
- Dedicated Slack channel
- Quarterly business reviews

**Premium (Add-On, $500/mo):**
- 24/7 on-call support
- 1h critical response time
- Monthly architecture reviews
- Direct engineering escalation

### Custom Features (Negotiable)

**Custom AI Agents:**
- Train Sonnet 4.5 on your internal API docs
- Custom prompt templates for your domain
- Private model fine-tuning (separate pricing)

**Advanced Analytics:**
- Prometheus exporter (real-time metrics)
- Grafana dashboard templates
- Custom data warehouse integration

**Multi-Tenancy:**
- Manage API integrations for your customers
- White-label dashboard (your branding)
- Billing API (charge your customers)

**On-Prem Appliance:**
- Pre-configured hardware appliance
- Air-gapped operation (no internet required)
- Annual maintenance contract

---

## Upgrade Paths

### Free → Pro
**Trigger:** User hits integration limit or wants AI features

**Flow:**
1. User tries to create 4th integration → "Upgrade to Pro"
2. Or: User runs `ai_discover` → "Pro feature - upgrade to unlock"
3. Click "Upgrade" → Stripe checkout
4. Instant activation (no downtime)
5. Retain all existing integrations + audit history

**Price:** $20/mo per developer (billed monthly)

---

### Pro → Enterprise
**Trigger:** Team needs private deployment or compliance

**Flow:**
1. User clicks "Contact Sales" in dashboard
2. Discovery call (30 min) to understand requirements
3. Custom proposal with pricing
4. Contract signing + onboarding kickoff
5. 2-4 week implementation (private deployment)
6. Go-live with dedicated support

**Price:** Custom quote (starts $500/mo for 5 seats)

---

### Downgrade (Pro → Free)
**Policy:** Allowed, but with data loss warnings

**Flow:**
1. User clicks "Downgrade to Free"
2. Warning: "You have 12 integrations. Free tier allows 3. Which 3 do you want to keep?"
3. User selects 3 integrations to keep
4. Remaining 9 integrations archived (not deleted)
5. Audit logs truncated to 24h (older logs exported as CSV)
6. AI features disabled

**Grace Period:** 30 days to re-upgrade (archived integrations restored)

---

## Packaging Strategy

### Anchor Pricing
- **Free:** $0 (anchor for virality)
- **Pro:** $20/mo (impulse purchase for professionals)
- **Enterprise:** $500+/mo (premium positioning)

### Value Perception
- Free tier is genuinely useful (not a "trial")
- Pro tier unlocks AI (clear value prop)
- Enterprise tier is for teams + compliance (custom)

### Expansion Revenue
- Start Free → upgrade to Pro when scaling
- Pro users naturally hit Enterprise triggers (team size, compliance)

### Land-and-Expand
- Developers start with Free (personal projects)
- Bring to work → Pro tier for team
- Company adopts → Enterprise for organization

---

## Competitive Pricing Comparison

| Competitor | Model | Price | KindlyAPI Advantage |
|------------|-------|-------|---------------------|
| Zapier | Per-task pricing | $20/mo (750 tasks) | Unlimited calls (Pro: 200K/day) |
| Postman | Per-seat + usage | $14/user/mo | Includes AI features in Pro |
| Kong | Gateway license | $2,500/mo (enterprise) | Zero-overhead, no gateway |
| OpenAPI Generator | Open source | Free | Runtime vs static code |
| Make (Integromat) | Per-operation | $9/mo (1,000 ops) | Deterministic guarantees |

**Bottom Line:** KindlyAPI Pro ($20/mo) is 2-5x cheaper for developers building LLM-powered apps.

---

## Revenue Projections (Year 1)

**Assumptions:**
- 1,000 Free users (80% stay free, 15% upgrade Pro, 5% churn)
- 150 Pro users (10% upgrade Enterprise annually)
- 15 Enterprise customers (avg $2,000/mo)

**Monthly Recurring Revenue (MRR) by Month 12:**
- Free: $0
- Pro: 150 users × $20 = $3,000/mo
- Enterprise: 15 customers × $2,000 = $30,000/mo
- **Total MRR: $33,000/mo**

**Annual Recurring Revenue (ARR) by Year 1:**
- $33,000 × 12 = **$396,000 ARR**

**Year 2 Target:**
- 5,000 Free users → 750 Pro → 75 Enterprise
- **$2.4M ARR** (6x growth)

---

## Positioning Summary

**Free:** "Try KindlyAPI risk-free. 3 integrations, deterministic runtime, 24h audit trail. No credit card required."

**Pro:** "Unlock unlimited integrations + AI-powered insights. Perfect for professional developers managing 5-10 APIs."

**Enterprise:** "Private deployment, custom security, and compliance certifications for teams that demand the highest standards."

**Tagline:** "Start free, scale with confidence, enterprise-ready from day one."
