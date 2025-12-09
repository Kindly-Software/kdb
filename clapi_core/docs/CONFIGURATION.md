# Configuration Reference

Complete configuration schema for Clapi Core.

## File Format

Configuration uses TOML format. Default location: `clapi.toml`

**Override via environment variable**:
```bash
export CLAPI_CONFIG=/path/to/custom.toml
clapi
```

**Override via command-line flag**:
```bash
clapi --config /path/to/custom.toml
```

## Complete Example

```toml
[server]
listen_addr = "0.0.0.0:8080"
default_budget_cents = 100_00
max_budget_slots = 1_000_000
admin_api_key = "admin-secret-key-here"

[circuit_breaker]
failure_threshold_bp = 1000
recovery_threshold_bp = 500
cooldown_secs = 60
min_samples = 10

[metrics]
enabled = true
export_interval_secs = 60
retention_days = 90
prometheus_endpoint = "http://localhost:9090"

[alerting]
enabled = true
budget_low_threshold_cents = 100_00
budget_critical_threshold_cents = 10_00
failure_rate_threshold_bp = 1000
webhook_url = "https://hooks.slack.com/services/..."
pagerduty_key = "pd-..."

[audit]
enabled = true
format = "json"
output_dir = "./audit_logs"
rotate_daily = true
compression = "gzip"

[[providers]]
id = "anthropic"
name = "Anthropic Claude"
api_key = "sk-ant-..."
endpoint = "https://api.anthropic.com/v1/messages"
model = "claude-3-5-sonnet-20241022"
priority = 1
timeout_secs = 60
max_retries = 3

[[providers]]
id = "openai"
name = "OpenAI GPT"
api_key = "sk-..."
endpoint = "https://api.openai.com/v1/chat/completions"
model = "gpt-4-turbo"
priority = 2
timeout_secs = 30
max_retries = 2
```

## Configuration Sections

### [server]

HTTP server configuration.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `listen_addr` | String | `"0.0.0.0:8080"` | Server bind address (IP:port) |
| `default_budget_cents` | u64 | `100_00` | Initial budget in cents ($100.00) |
| `max_budget_slots` | usize | `1_000_000` | Maximum concurrent budgets (1M = 128MB RAM) |
| `admin_api_key` | String | None | Admin API key for /admin/* endpoints |
| `cors_allowed_origins` | Array[String] | `["*"]` | CORS allowed origins |
| `request_timeout_secs` | u64 | `120` | Maximum request processing time |

**Memory calculation**: `max_budget_slots × 128 bytes`
- 1M slots = 128 MB
- 10M slots = 1.28 GB
- 100M slots = 12.8 GB

**Environment overrides**:
```bash
export CLAPI_LISTEN_ADDR="127.0.0.1:8080"
export CLAPI_DEFAULT_BUDGET_CENTS=50000  # $500.00
export CLAPI_ADMIN_API_KEY="secret"
```

### [circuit_breaker]

Circuit breaker configuration for provider health monitoring.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `failure_threshold_bp` | u16 | `1000` | Open circuit at 10% failure (basis points) |
| `recovery_threshold_bp` | u16 | `500` | Close circuit at 5% failure |
| `cooldown_secs` | u64 | `60` | Cooldown before retry (seconds) |
| `min_samples` | u32 | `10` | Minimum requests before evaluation |

**Basis points (bp)**: 1% = 100bp, 10% = 1000bp, 100% = 10000bp

**States**:
- **Closed** (0): Provider healthy (<5% failure)
- **HalfOpen** (1): Monitoring recovery (5-10% failure)
- **Open** (2): Provider failing (>10% failure)

**Example: Aggressive failover (5% threshold)**:
```toml
[circuit_breaker]
failure_threshold_bp = 500     # 5% failure triggers open
recovery_threshold_bp = 250    # 2.5% failure for recovery
cooldown_secs = 30              # Quick retry (30s)
min_samples = 20                # More samples for stability
```

**Example: Tolerant (20% threshold)**:
```toml
[circuit_breaker]
failure_threshold_bp = 2000    # 20% failure triggers open
recovery_threshold_bp = 1000   # 10% failure for recovery
cooldown_secs = 120             # Longer cooldown (2 min)
min_samples = 5                 # Fewer samples (faster response)
```

**Environment overrides**:
```bash
export CLAPI_CIRCUIT_BREAKER_FAILURE_THRESHOLD_BP=500
export CLAPI_CIRCUIT_BREAKER_COOLDOWN_SECS=30
```

### [metrics]

Metrics collection and export configuration.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `enabled` | bool | `true` | Enable metrics collection |
| `export_interval_secs` | u64 | `60` | Export interval (seconds) |
| `retention_days` | u32 | `90` | Metrics retention period |
| `prometheus_endpoint` | String | None | Prometheus push gateway URL |
| `cloudwatch_namespace` | String | None | AWS CloudWatch namespace |
| `statsd_endpoint` | String | None | StatsD endpoint (UDP) |

**Collected metrics**:
- Budget operations (deductions, allocations, failures)
- Circuit breaker state (per-provider)
- Request latency (p50, p90, p95, p99)
- Cost breakdown (per-provider, per-budget)
- Hash chain integrity status

**Example: Prometheus export**:
```toml
[metrics]
enabled = true
export_interval_secs = 15
prometheus_endpoint = "http://prometheus:9091"
```

**Example: CloudWatch export**:
```toml
[metrics]
enabled = true
cloudwatch_namespace = "ClapiCore/Production"
retention_days = 365
```

**Environment overrides**:
```bash
export CLAPI_METRICS_ENABLED=true
export CLAPI_METRICS_PROMETHEUS_ENDPOINT="http://localhost:9090"
```

### [alerting]

Real-time alerting configuration.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `enabled` | bool | `true` | Enable alerting |
| `budget_low_threshold_cents` | u64 | `100_00` | Low budget warning ($100) |
| `budget_critical_threshold_cents` | u64 | `10_00` | Critical budget alert ($10) |
| `failure_rate_threshold_bp` | u16 | `1000` | Alert on >10% failure rate |
| `webhook_url` | String | None | Slack/Teams webhook URL |
| `pagerduty_key` | String | None | PagerDuty integration key |
| `email_recipients` | Array[String] | `[]` | Email recipients |
| `smtp_server` | String | None | SMTP server (for email) |

**Alert priorities**:
- **CRITICAL**: Budget <$10, All providers circuit open, Circuit open >5min
- **WARNING**: Budget <$100, Provider failure >5%, Slot utilization >80%
- **INFO**: Provider recovered, Budget refilled

**Example: Slack webhook**:
```toml
[alerting]
enabled = true
webhook_url = "https://hooks.slack.com/services/T00000000/B00000000/XXXXXXXXXXXX"
budget_low_threshold_cents = 50_00  # $50 warning
budget_critical_threshold_cents = 10_00  # $10 critical
```

**Example: PagerDuty**:
```toml
[alerting]
enabled = true
pagerduty_key = "your-integration-key"
failure_rate_threshold_bp = 500  # Alert on >5% failure
```

**Example: Email**:
```toml
[alerting]
enabled = true
email_recipients = ["ops@company.com", "oncall@company.com"]
smtp_server = "smtp.gmail.com:587"
smtp_username = "alerts@company.com"
smtp_password = "app-specific-password"
```

**Environment overrides**:
```bash
export CLAPI_ALERTING_WEBHOOK_URL="https://hooks.slack.com/..."
export CLAPI_ALERTING_PAGERDUTY_KEY="pd-..."
```

### [audit]

Audit trail configuration (SOX/SOC2/GDPR/HIPAA compliance).

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `enabled` | bool | `true` | Enable audit logging |
| `format` | String | `"json"` | Log format (json/csv/binary) |
| `output_dir` | String | `"./audit_logs"` | Audit log directory |
| `rotate_daily` | bool | `true` | Rotate logs daily |
| `compression` | String | `"gzip"` | Compression (none/gzip/zstd) |
| `retention_days` | u32 | `2555` | Retention period (7 years default) |
| `hash_chain_validation` | bool | `true` | Validate hash chain on startup |

**Logged events**:
- Request received (timestamp, budget_id, provider, model)
- Budget deduction (amount, before, after)
- Provider response (status, tokens, cost)
- Circuit breaker state changes
- Admin operations (budget refills, config changes)

**Example: 7-year retention (SOX compliance)**:
```toml
[audit]
enabled = true
format = "json"
output_dir = "/var/log/clapi/audit"
rotate_daily = true
compression = "zstd"
retention_days = 2555  # 7 years
hash_chain_validation = true
```

**Example: High-performance (binary format)**:
```toml
[audit]
enabled = true
format = "binary"
compression = "none"
rotate_daily = false
```

**Environment overrides**:
```bash
export CLAPI_AUDIT_OUTPUT_DIR="/var/log/clapi"
export CLAPI_AUDIT_RETENTION_DAYS=365
```

### [[providers]]

Provider configuration (repeatable section).

| Setting | Type | Required | Description |
|---------|------|----------|-------------|
| `id` | String | ✅ | Unique provider identifier |
| `name` | String | ✅ | Display name |
| `api_key` | String | ✅ | Provider API key |
| `endpoint` | String | ✅ | API endpoint URL |
| `model` | String | ✅ | Default model name |
| `priority` | u8 | ✅ | Priority (1=highest, 255=lowest) |
| `timeout_secs` | u64 | No (default: 60) | Request timeout |
| `max_retries` | u8 | No (default: 3) | Max retry attempts |
| `rate_limit_rps` | u32 | No | Rate limit (requests/second) |
| `cost_multiplier` | f64 | No (default: 1.0) | Cost adjustment multiplier |

**Priority-based routing**:
- Providers sorted by priority (ascending)
- Requests route to highest priority with Closed circuit
- If Open, fallback to next priority

**Example: Multi-provider setup**:
```toml
[[providers]]
id = "anthropic_primary"
name = "Anthropic Claude (Primary)"
api_key = "sk-ant-primary-..."
endpoint = "https://api.anthropic.com/v1/messages"
model = "claude-3-5-sonnet-20241022"
priority = 1  # Preferred
timeout_secs = 60
max_retries = 3

[[providers]]
id = "anthropic_backup"
name = "Anthropic Claude (Backup)"
api_key = "sk-ant-backup-..."
endpoint = "https://api.anthropic.com/v1/messages"
model = "claude-3-5-sonnet-20241022"
priority = 2  # Fallback
timeout_secs = 30
max_retries = 2

[[providers]]
id = "openai"
name = "OpenAI GPT-4"
api_key = "sk-openai-..."
endpoint = "https://api.openai.com/v1/chat/completions"
model = "gpt-4-turbo"
priority = 3  # Emergency fallback
timeout_secs = 45
max_retries = 2
rate_limit_rps = 100
```

**Example: Cost adjustment**:
```toml
[[providers]]
id = "budget_provider"
api_key = "sk-..."
endpoint = "https://api.provider.com/v1/chat"
model = "budget-model"
priority = 1
cost_multiplier = 0.5  # 50% cheaper (for internal accounting)
```

**Environment overrides** (per-provider):
```bash
# Override API key for provider "anthropic"
export CLAPI_PROVIDER_ANTHROPIC_API_KEY="sk-ant-override-..."

# Override endpoint for provider "openai"
export CLAPI_PROVIDER_OPENAI_ENDPOINT="https://custom.endpoint.com"
```

## Environment Variables Reference

All configuration can be overridden via environment variables using the pattern:
`CLAPI_<SECTION>_<SETTING>` (uppercase, underscores)

### Server
- `CLAPI_LISTEN_ADDR`
- `CLAPI_DEFAULT_BUDGET_CENTS`
- `CLAPI_MAX_BUDGET_SLOTS`
- `CLAPI_ADMIN_API_KEY`

### Circuit Breaker
- `CLAPI_CIRCUIT_BREAKER_FAILURE_THRESHOLD_BP`
- `CLAPI_CIRCUIT_BREAKER_RECOVERY_THRESHOLD_BP`
- `CLAPI_CIRCUIT_BREAKER_COOLDOWN_SECS`
- `CLAPI_CIRCUIT_BREAKER_MIN_SAMPLES`

### Metrics
- `CLAPI_METRICS_ENABLED`
- `CLAPI_METRICS_EXPORT_INTERVAL_SECS`
- `CLAPI_METRICS_PROMETHEUS_ENDPOINT`
- `CLAPI_METRICS_CLOUDWATCH_NAMESPACE`

### Alerting
- `CLAPI_ALERTING_ENABLED`
- `CLAPI_ALERTING_WEBHOOK_URL`
- `CLAPI_ALERTING_PAGERDUTY_KEY`
- `CLAPI_ALERTING_BUDGET_LOW_THRESHOLD_CENTS`

### Audit
- `CLAPI_AUDIT_ENABLED`
- `CLAPI_AUDIT_OUTPUT_DIR`
- `CLAPI_AUDIT_RETENTION_DAYS`

### Providers (per-provider overrides)
- `CLAPI_PROVIDER_<ID>_API_KEY`
- `CLAPI_PROVIDER_<ID>_ENDPOINT`
- `CLAPI_PROVIDER_<ID>_TIMEOUT_SECS`

## Validation

Configuration is validated on startup. Common errors:

### Invalid basis points
```
Error: failure_threshold_bp must be in range [0, 10000] (got 15000)
```
**Fix**: Use basis points (1% = 100bp, 10% = 1000bp)

### Missing required provider fields
```
Error: Provider 'anthropic' missing required field: api_key
```
**Fix**: Add all required fields (id, name, api_key, endpoint, model, priority)

### Duplicate provider IDs
```
Error: Duplicate provider id 'anthropic' (must be unique)
```
**Fix**: Use unique IDs for each provider

### Invalid memory allocation
```
Error: max_budget_slots too large (would allocate 128GB)
```
**Fix**: Reduce `max_budget_slots` (1M = 128MB, 10M = 1.28GB)

## Production Best Practices

### 1. Use environment variables for secrets

**Bad**:
```toml
[[providers]]
api_key = "sk-ant-hardcoded-key"  # ❌ Committed to git
```

**Good**:
```toml
[[providers]]
api_key = "${ANTHROPIC_API_KEY}"  # ✅ From environment
```

### 2. Enable all monitoring

```toml
[metrics]
enabled = true
prometheus_endpoint = "http://prometheus:9091"

[alerting]
enabled = true
webhook_url = "${SLACK_WEBHOOK_URL}"

[audit]
enabled = true
retention_days = 2555  # 7 years
```

### 3. Configure multi-provider failover

Always configure ≥2 providers for redundancy:
```toml
[[providers]]
id = "primary"
priority = 1

[[providers]]
id = "backup"
priority = 2
```

### 4. Tune circuit breaker for your SLA

**High availability (aggressive failover)**:
```toml
[circuit_breaker]
failure_threshold_bp = 500  # 5% failure
cooldown_secs = 30
```

**Cost optimization (tolerant)**:
```toml
[circuit_breaker]
failure_threshold_bp = 2000  # 20% failure
cooldown_secs = 120
```

### 5. Size budget slots appropriately

**Calculation**: `max_budget_slots = peak_concurrent_users × 1.5`

Example: 100K concurrent users → `max_budget_slots = 150_000` (19.2MB RAM)

## Next Steps

- **Quick Start**: [QUICK_START.md](QUICK_START.md)
- **Troubleshooting**: [TROUBLESHOOTING.md](TROUBLESHOOTING.md)
- **Metrics Guide**: [METRICS_ADMIN_GUIDE.md](METRICS_ADMIN_GUIDE.md)
