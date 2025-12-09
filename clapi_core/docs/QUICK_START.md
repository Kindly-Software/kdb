# Quick Start Guide

**Goal**: Get Clapi Core running in 5 minutes.

## Prerequisites

- Rust 1.83+ ([install rustup](https://rustup.rs/))
- An AI provider API key (Anthropic, OpenAI, etc.)

## Installation

### Option 1: Add as Dependency

```bash
cargo add clapi_core
```

### Option 2: Clone Repository

```bash
git clone https://github.com/primitives/clapi_core
cd clapi_core
cargo build --release
```

## Minimal Configuration

Create `clapi.toml`:

```toml
[server]
listen_addr = "0.0.0.0:8080"
default_budget_cents = 100_00  # $100.00 initial budget

[circuit_breaker]
failure_threshold_bp = 1000     # 10% failure rate triggers circuit open
recovery_threshold_bp = 500     # 5% failure rate for recovery
cooldown_secs = 60              # 60s cooldown before retry

[[providers]]
id = "anthropic"
name = "Anthropic Claude"
api_key = "sk-ant-..."          # Replace with your API key
endpoint = "https://api.anthropic.com/v1/messages"
model = "claude-3-5-sonnet-20241022"
priority = 1                     # Higher = preferred
```

## Run the Server

```bash
# From repository root
cargo run --bin clapi -- --config clapi.toml

# Or if installed as binary
clapi --config clapi.toml
```

**Expected output**:
```
[INFO] Clapi Core v0.4.6 starting...
[INFO] Budget registry initialized (1M slots, 128MB preallocated)
[INFO] Circuit breaker configured (10% failure threshold)
[INFO] Listening on http://0.0.0.0:8080
```

## Test the API

### 1. Health Check

```bash
curl http://localhost:8080/health
```

**Response**:
```json
{
  "status": "healthy",
  "providers": [
    {
      "id": "anthropic",
      "state": "Closed",
      "failure_rate_bp": 0
    }
  ]
}
```

### 2. Create a Budget

Budgets are created automatically on first use. Use the budget ID as the Bearer token.

### 3. Chat Completion (OpenAI-Compatible)

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer my_budget_id" \
  -d '{
    "model": "claude-3-5-sonnet-20241022",
    "messages": [
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

**Response**:
```json
{
  "id": "msg_...",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "text",
      "text": "Hello! How can I assist you today?"
    }
  ],
  "model": "claude-3-5-sonnet-20241022",
  "usage": {
    "input_tokens": 10,
    "output_tokens": 15
  }
}
```

### 4. Check Budget Status

```bash
curl http://localhost:8080/metrics/budget
```

**Response**:
```json
{
  "budget_id": "my_budget_id",
  "initial_cents": 10000,
  "spent_cents": 50,
  "remaining_cents": 9950,
  "deduction_count": 1,
  "hash_chain_valid": true
}
```

### 5. Monitor Circuit Breaker

```bash
curl http://localhost:8080/metrics/circuit_breaker
```

**Response**:
```json
{
  "state": "Closed",
  "failure_rate_bp": 0,
  "total_requests": 1,
  "total_failures": 0,
  "trip_count": 0,
  "cooldown_remaining_ns": 0
}
```

## Common Use Cases

### Use Case 1: Multi-Provider Failover

Add multiple providers for automatic failover:

```toml
[[providers]]
id = "anthropic"
api_key = "sk-ant-..."
endpoint = "https://api.anthropic.com/v1/messages"
priority = 1  # Preferred

[[providers]]
id = "openai"
api_key = "sk-..."
endpoint = "https://api.openai.com/v1/chat/completions"
priority = 2  # Fallback
```

**Behavior**: If Anthropic fails >10%, circuit opens and requests failover to OpenAI automatically.

### Use Case 2: Budget Limits Per Team

Create separate budgets per team:

```bash
# Team A: $500 budget
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer team_a_budget" \
  ...

# Team B: $200 budget
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer team_b_budget" \
  ...
```

Each budget is tracked independently. When exhausted, requests return `BudgetExhausted` error.

### Use Case 3: Cost Tracking & Forecasting

Query metrics for spending analysis:

```bash
curl http://localhost:8080/metrics
```

**Response includes**:
- Historical spending (last 7/30/90 days)
- Forecasted depletion time (SMA, EWMA, Linear Regression)
- Budget recommendations (p50/p90/p95/p99)
- Cost breakdown by provider

### Use Case 4: Compliance Audit Trail

All requests are logged with tamper-proof hash chain:

```bash
# Export audit log (JSON format)
curl http://localhost:8080/audit/export?format=json > audit.json

# Verify hash chain integrity
curl http://localhost:8080/audit/verify
```

**Response**:
```json
{
  "valid": true,
  "chain_length": 1543,
  "first_event": "2025-10-18T10:00:00Z",
  "last_event": "2025-10-18T15:30:00Z"
}
```

## Performance Tuning

### Increase Budget Slots (Default: 1M)

For >1M concurrent budgets, increase slot count:

```toml
[server]
max_budget_slots = 10_000_000  # 10M budgets (1.28GB preallocated)
```

**Memory usage**: `slots × 128B` (128 bytes per slot)

### Circuit Breaker Sensitivity

Adjust thresholds for your workload:

```toml
[circuit_breaker]
failure_threshold_bp = 500      # 5% (more sensitive)
min_samples = 20                 # Require 20 samples before evaluation
cooldown_secs = 120              # 2-minute cooldown
```

### Enable Metrics Export

Configure Prometheus/CloudWatch export:

```toml
[metrics]
enabled = true
export_interval_secs = 60       # Export every 60s
retention_days = 90              # Keep 90 days history
```

## Next Steps

1. **Configuration**: See [CONFIGURATION.md](CONFIGURATION.md) for complete schema
2. **Troubleshooting**: See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for common errors
3. **Monitoring**: See [METRICS_ADMIN_GUIDE.md](METRICS_ADMIN_GUIDE.md) for alerting setup
4. **Compliance**: See [COMPLIANCE_AUDIT_GUIDE.md](COMPLIANCE_AUDIT_GUIDE.md) for SOX/SOC2

## FAQ

### Q: How do I create a new budget?

**A**: Budgets are created automatically on first use. Just use any string as the Bearer token.

### Q: What happens when budget is exhausted?

**A**: Requests return `BudgetExhausted` error. Refill via `/budget/add` endpoint (requires admin key).

### Q: How does multi-provider failover work?

**A**: When primary provider fails >10%, circuit opens. Requests automatically route to next priority provider.

### Q: Is the audit log tamper-proof?

**A**: Yes. Every event is SHA256-hashed and linked to previous event (hash chain). Tampering breaks verification.

### Q: What's the performance overhead?

**A**: <300ns per request (<0.3% of typical 100ms provider latency). Zero allocations on hot path.

### Q: Can I use this in production?

**A**: Yes. Clapi Core is production-ready (v0.4.6):
- 100% lockfree (no mutex deadlocks)
- Property tested (1000-thread concurrent allocation)
- ASSUM validated (all atomic operations verified)
- B32 benchmarked (statistical rigor, fair baselines)

## Support

- **Issues**: [GitHub Issues](https://github.com/primitives/clapi_core/issues)
- **Discussions**: [GitHub Discussions](https://github.com/primitives/clapi_core/discussions)
- **Email**: samuel@primitives.io
