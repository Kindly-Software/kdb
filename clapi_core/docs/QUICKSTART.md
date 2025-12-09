# Quick Start Guide - Clapi Core

**Goal**: Get Clapi Core running in 5 minutes with a Hello World example.

**Prerequisites**:
- Rust 1.83+ (nightly recommended for best performance)
- An AI provider API key (Anthropic Claude or OpenAI GPT-4)
- Basic familiarity with Rust and HTTP APIs

---

## Step 1: Installation (30 seconds)

```bash
# Clone or add as dependency
cargo new my_clapi_project
cd my_clapi_project

# Add clapi_core to Cargo.toml
cargo add clapi_core tokio --features tokio/full
```

Or add manually to `Cargo.toml`:

```toml
[dependencies]
clapi_core = "0.4"
tokio = { version = "1.0", features = ["full"] }
```

---

## Step 2: Configuration (1 minute)

Create `clapi.toml` in your project root:

```toml
[server]
listen_addr = "127.0.0.1:8080"
default_budget_cents = 100_00  # $100.00 starting budget

[circuit_breaker]
failure_threshold_bp = 1000      # 10% failure rate opens circuit
recovery_threshold_bp = 500      # 5% failure rate closes circuit
cooldown_secs = 60               # 60 seconds cooldown after circuit opens
min_samples = 10                 # Minimum requests before circuit evaluation

[[providers]]
id = "anthropic"
name = "Anthropic Claude"
api_key = "sk-ant-..."  # Replace with your API key
endpoint = "https://api.anthropic.com/v1/messages"
priority = 1            # Highest priority (lower number = higher priority)
max_retries = 3
timeout_secs = 30
```

**Important**: Replace `api_key` with your actual Anthropic API key.

**Test Mode** (no API key required):
```bash
# Start in test mode with mock responses
cargo run --bin clapi -- start --test
```

---

## Step 3: Hello World - Single Request (2 minutes)

Create `examples/hello_world.rs`:

```rust
use clapi_core::{BudgetRegistry, ProxyConfig};
use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load configuration
    let config = ProxyConfig::from_file("clapi.toml")?;

    // 2. Create budget registry (lockfree, <1ms initialization)
    //    Preallocates 1M budget slots (128MB)
    let registry = BudgetRegistry::new(config.default_budget_cents);

    // 3. Start server in background task
    let registry_clone = registry.clone();
    let config_clone = config.clone();
    tokio::spawn(async move {
        clapi_core::run_server(config_clone, registry_clone).await
    });

    // Wait for server startup
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 4. Send test request through proxy
    let client = Client::new();
    let response = client
        .post("http://127.0.0.1:8080/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", "Bearer budget_12345")  // Budget ID as bearer token
        .json(&json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 100,
            "messages": [{
                "role": "user",
                "content": "Hello, Claude! This is a test from Clapi Core."
            }]
        }))
        .send()
        .await?;

    // 5. Check response
    println!("Status: {}", response.status());
    println!("Response: {}", response.text().await?);

    // 6. Check remaining budget
    let budget_info = client
        .get("http://127.0.0.1:8080/metrics/budget")
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    println!("Remaining budget: ${:.2}",
        budget_info["remaining_cents"].as_i64().unwrap() as f64 / 100.0);

    Ok(())
}
```

Run it:

```bash
cargo run --example hello_world
```

**Expected Output**:
```
Status: 200 OK
Response: {"id":"msg_...","type":"message",...}
Remaining budget: $99.95
```

---

## Step 4: Budget Enforcement Demo (2 minutes)

Create `examples/budget_enforcement.rs`:

```rust
use clapi_core::{BudgetRegistry, ProxyConfig};
use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ProxyConfig::from_file("clapi.toml")?;
    let registry = BudgetRegistry::new(100);  // Start with only $1.00

    // Start server
    let registry_clone = registry.clone();
    let config_clone = config.clone();
    tokio::spawn(async move {
        clapi_core::run_server(config_clone, registry_clone).await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let client = Client::new();

    // Request 1: Should succeed ($1.00 available)
    println!("Request 1: Sending with $1.00 budget...");
    let response1 = client
        .post("http://127.0.0.1:8080/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", "Bearer budget_enforcement_test")
        .json(&json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 50,
            "messages": [{"role": "user", "content": "Test 1"}]
        }))
        .send()
        .await?;

    println!("✅ Request 1: Status {}", response1.status());

    // Request 2: Should fail (budget exhausted)
    println!("\nRequest 2: Sending after budget exhausted...");
    let response2 = client
        .post("http://127.0.0.1:8080/v1/chat/completions")
        .header("Authorization", "Bearer budget_enforcement_test")
        .json(&json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 50,
            "messages": [{"role": "user", "content": "Test 2"}]
        }))
        .send()
        .await?;

    println!("❌ Request 2: Status {} (Budget Exhausted)", response2.status());

    // Check budget metrics
    let metrics = client
        .get("http://127.0.0.1:8080/metrics/budget")
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    println!("\n📊 Final Budget Metrics:");
    println!("  Remaining: ${:.2}", metrics["remaining_cents"].as_i64().unwrap() as f64 / 100.0);
    println!("  Total Deductions: {}", metrics["total_deductions"].as_u64().unwrap());

    Ok(())
}
```

Run it:

```bash
cargo run --example budget_enforcement
```

**Expected Output**:
```
Request 1: Sending with $1.00 budget...
✅ Request 1: Status 200 OK

Request 2: Sending after budget exhausted...
❌ Request 2: Status 402 Payment Required (Budget Exhausted)

📊 Final Budget Metrics:
  Remaining: $0.00
  Total Deductions: 1
```

---

## Step 5: Circuit Breaker Failover (2 minutes)

Create `clapi_multi_provider.toml`:

```toml
[server]
listen_addr = "127.0.0.1:8080"
default_budget_cents = 100_00

[circuit_breaker]
failure_threshold_bp = 1000
recovery_threshold_bp = 500
cooldown_secs = 60
min_samples = 10

# Primary provider (priority 1)
[[providers]]
id = "anthropic"
name = "Anthropic Claude"
api_key = "sk-ant-..."
endpoint = "https://api.anthropic.com/v1/messages"
priority = 1

# Fallback provider (priority 2)
[[providers]]
id = "openai"
name = "OpenAI GPT-4"
api_key = "sk-..."
endpoint = "https://api.openai.com/v1/chat/completions"
priority = 2
```

Create `examples/circuit_breaker.rs`:

```rust
use clapi_core::{BudgetRegistry, ProxyConfig};
use reqwest::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load multi-provider config
    let config = ProxyConfig::from_file("clapi_multi_provider.toml")?;
    let registry = BudgetRegistry::new(config.default_budget_cents);

    // Start server
    let registry_clone = registry.clone();
    let config_clone = config.clone();
    tokio::spawn(async move {
        clapi_core::run_server(config_clone, registry_clone).await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let client = Client::new();

    // Monitor circuit breaker status
    loop {
        let health = client
            .get("http://127.0.0.1:8080/health")
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        println!("\n🔌 Circuit Breaker Status:");
        if let Some(providers) = health["providers"].as_array() {
            for provider in providers {
                let name = provider["name"].as_str().unwrap();
                let status = provider["circuit_state"].as_str().unwrap();
                let failure_rate = provider["failure_rate_bp"].as_u64().unwrap();

                let emoji = match status {
                    "Closed" => "✅",
                    "HalfOpen" => "⚠️",
                    "Open" => "❌",
                    _ => "❓",
                };

                println!("  {} {}: {} (failure rate: {}bp)",
                    emoji, name, status, failure_rate);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}
```

Run it:

```bash
cargo run --example circuit_breaker
```

**Expected Output**:
```
🔌 Circuit Breaker Status:
  ✅ Anthropic Claude: Closed (failure rate: 0bp)
  ✅ OpenAI GPT-4: Closed (failure rate: 0bp)

# After primary provider fails (>10% failures)
🔌 Circuit Breaker Status:
  ❌ Anthropic Claude: Open (failure rate: 1200bp)
  ✅ OpenAI GPT-4: Closed (failure rate: 0bp)
```

---

## Step 6: Monitoring & Metrics (1 minute)

### Health Check

```bash
curl http://127.0.0.1:8080/health | jq
```

**Response**:
```json
{
  "status": "healthy",
  "providers": [
    {
      "id": "anthropic",
      "name": "Anthropic Claude",
      "priority": 1,
      "circuit_state": "Closed",
      "failure_rate_bp": 0,
      "total_requests": 142,
      "failed_requests": 0
    }
  ]
}
```

### Budget Metrics

```bash
curl http://127.0.0.1:8080/metrics/budget | jq
```

**Response**:
```json
{
  "budget_id": "budget_12345",
  "remaining_cents": 9995,
  "total_deductions": 1,
  "hash_chain_valid": true,
  "last_deduction_ts": 1729536000
}
```

### Circuit Breaker Metrics

```bash
curl http://127.0.0.1:8080/metrics/circuit_breaker | jq
```

**Response**:
```json
{
  "providers": [
    {
      "provider_id": "anthropic",
      "state": "Closed",
      "failure_count": 0,
      "total_requests": 142,
      "failure_rate_bp": 0,
      "last_trip_ts": null
    }
  ]
}
```

---

## Common Tasks

### Add Budget to Existing User

```bash
curl -X POST http://127.0.0.1:8080/budget/allocate \
  -H "Content-Type: application/json" \
  -d '{"budget_id": "user_alice", "amount_cents": 5000}'
```

### Check Budget Remaining

```bash
curl http://127.0.0.1:8080/metrics/budget?budget_id=user_alice | jq '.remaining_cents'
```

### Manually Trip Circuit Breaker (Testing)

```bash
# Send 20 requests to a non-existent endpoint to trigger failures
for i in {1..20}; do
  curl -X POST http://127.0.0.1:8080/v1/chat/completions \
    -H "Authorization: Bearer test" \
    -H "Content-Type: application/json" \
    -d '{"model":"invalid","messages":[]}'
done

# Check circuit breaker status
curl http://127.0.0.1:8080/health | jq '.providers[] | {name, circuit_state, failure_rate_bp}'
```

---

## Next Steps

**Congratulations!** You've successfully:
- ✅ Installed and configured Clapi Core
- ✅ Sent your first request through the proxy
- ✅ Tested budget enforcement
- ✅ Explored circuit breaker failover
- ✅ Monitored health and metrics

### Learn More

- **[Configuration Guide](CONFIGURATION.md)** - Complete config reference (all options explained)
- **[Architecture Overview](ARCHITECTURE_OVERVIEW.md)** - How Clapi Core works internally (<20 min read)
- **[Troubleshooting](TROUBLESHOOTING.md)** - Common errors and solutions
- **[Integration Guide](INTEGRATION_GUIDE.md)** - Grafana + Prometheus monitoring
- **[Performance Tuning](PERFORMANCE.md)** - SLO configuration and optimization

### Production Deployment

For production use:
1. **Security**: Use environment variables for API keys (never commit to git)
2. **Monitoring**: Set up Grafana dashboards ([see Integration Guide](INTEGRATION_GUIDE.md))
3. **Scaling**: Increase `max_budget_slots` for >1M concurrent budgets
4. **Redundancy**: Deploy multiple providers with priority-based failover
5. **Audit**: Enable compliance logging for SOX/SOC2/GDPR/HIPAA ([see Compliance Guide](../COMPLIANCE_AUDIT_GUIDE.md))

---

## Troubleshooting Quick Reference

| Error | Status | Solution |
|-------|--------|----------|
| `Budget exhausted` | 402 | Increase budget or add funds |
| `All providers unavailable` | 503 | Check provider API keys and network |
| `Circuit breaker open` | 503 | Wait 60s for cooldown or check provider health |
| `Invalid API key` | 401 | Verify provider API key in config |
| `Request timeout` | 504 | Increase `timeout_secs` in provider config |

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for detailed error recovery.

---

**Total Time**: ~5-10 minutes

**Questions?** See [docs/](.) for complete documentation or file an issue on GitHub.
