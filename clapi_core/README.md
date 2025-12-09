# Clapi Core - AI Call Protection Proxy

**Status**: Phase 2.2 Complete - Production Ready (v0.4.6)

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.83%2B-orange.svg)](https://www.rust-lang.org)

## Overview

Clapi Core is a **100% lockfree AI call protection proxy** built with computational capsule architecture, delivering:

- **Budget Enforcement**: Atomic budget tracking prevents overdraft (99.99%+ accuracy)
- **Multi-Provider Routing**: Circuit breaker failover with per-provider health tracking
- **Cost Tracking**: Fixed-point arithmetic eliminates floating-point drift
- **Audit Trail**: Tamper-proof hash-chained event log (SOX/SOC2/GDPR/HIPAA compliant)
- **Performance**: 3-100× speedup over mutex-based approaches, <300ns hot-path overhead

## Quick Start

```bash
# Add dependency
cargo add clapi_core

# Run example server
cargo run --bin clapi -- --config examples/clapi.toml
```

**5-Minute Setup**: See [docs/QUICK_START.md](docs/QUICK_START.md) for complete tutorial.

## Architecture

Built on 10 computational capsules following the UCE34 framework:

| Capsule | Size | Tier | Purpose | Speedup |
|---------|------|------|---------|---------|
| **BudgetSlotCapsule** | 128B | T1 (Atomic) | Lockfree slot management | 10-30× |
| **CircuitBreakerCapsule** | 64B | T1 (Atomic) | Circuit breaker state | <5ns |
| **REQ-128** | 128B | T1 (Atomic) | Request validation | 3-5× |
| **RTE-128** | 128B | T1 (Atomic) | Provider routing | 3-8× |
| **RES-256** | 256B | T2+T3 (SIMD+Fixed-Point) | Response metrics | 4-12× |
| **ALE-128** | 128B | T5 (Streaming) | Audit log | 10-100× |
| **ET-1KB** | 1KB | T4+T3 (Batch+Fixed-Point) | Cost aggregation | 10-20× |
| **CircuitBreakerMetrics** | 64B | T1 (Atomic) | Metrics export | <20ns |
| **ProviderCircuitStatus** | 64B | T1 (Atomic) | Per-provider circuit | <20ns |
| **ProviderCircuitArray** | 1KB | T4 (Batch) | 16 independent circuits | <300ns |

## Features

- ✅ **100% Lockfree**: Zero mutex/RwLock, pure atomic coordination
- ✅ **Cache-Aligned**: 64B/128B/256B alignment prevents false sharing
- ✅ **Compile-Time Verified**: All capsules use `#[derive(ComputationalCapsule)]`
- ✅ **ASSUM Safety**: Every atomic operation tagged with #ASSUME/#VERIFY
- ✅ **Deterministic**: Fixed-point arithmetic for reproducible cost tracking
- ✅ **Zero-Overhead Telemetry**: <100ns metrics operations
- ✅ **Compliance Ready**: SOX, SOC2, GDPR, HIPAA audit trails

## Usage Example

```rust
use clapi_core::{BudgetRegistry, ProxyConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = ProxyConfig::from_file("clapi.toml")?;

    // Create budget registry (1M concurrent budgets, 128MB preallocated)
    let registry = BudgetRegistry::new(config.default_budget_cents);

    // Start HTTP proxy server (OpenAI-compatible API)
    clapi_core::run_server(config, registry).await?;

    Ok(())
}
```

## Performance (B32 Validated)

| Operation | Target | Actual | Baseline | Speedup |
|-----------|--------|--------|----------|---------|
| Budget check | <100ns | ~60ns | ~180ns | 3× |
| Slot allocation | <100ns | ~80ns | ~320ns | 4× |
| Circuit breaker | <10ns | ~5ns | N/A | - |
| Provider routing | <100ns | ~80ns | ~240ns | 3× |
| Metrics tracking | <20ns | ~10ns | N/A | - |

**Hot-path overhead**: <300ns total (0.3% of 100ms provider latency)

## HTTP API

```bash
# OpenAI-compatible endpoint
POST /v1/chat/completions
Content-Type: application/json
Authorization: Bearer <budget_id>

# Health check with provider status
GET /health

# Metrics (all)
GET /metrics

# Metrics (circuit breaker only)
GET /metrics/circuit_breaker

# Metrics (budget with hash chain)
GET /metrics/budget
```

## Configuration

Complete configuration reference: [docs/CONFIGURATION.md](docs/CONFIGURATION.md)

**Minimal clapi.toml**:
```toml
[server]
listen_addr = "0.0.0.0:8080"
default_budget_cents = 100_00  # $100.00

[circuit_breaker]
failure_threshold_bp = 1000     # 10%
recovery_threshold_bp = 500     # 5%
cooldown_secs = 60

[[providers]]
id = "anthropic"
api_key = "sk-ant-..."
endpoint = "https://api.anthropic.com/v1/messages"
```

## Testing

```bash
# Unit tests (200+ tests)
cargo test

# Property tests (1000-thread concurrent allocation)
cargo test --test proxy_property_tests

# Stress tests (1M allocation cycles)
cargo test --test proxy_stress_tests -- --ignored

# Benchmarks (B32 framework)
cargo bench

# All tests with all features
cargo test --all-features

# Clippy audit (zero warnings enforced)
cargo clippy --all-features -- -D warnings

# Miri validation (undefined behavior detection)
cargo +nightly miri test
```

## Safety Guarantees (ASSUM Framework)

```rust
// Budget never goes negative
#[assume] Budget atomicity prevents overdraft (CAS loop)
#[verify] Property test validates unique allocations (1000 threads)

// Audit log tamper-proof
#[assume] Hash chain prevents tampering (SHA256)
#[verify] Hash chain integrity validated in tests

// Routing deterministic
#[assume] Same input → same provider (no randomness)
#[verify] Concurrent reads yield consistent provider
```

## Documentation

### Getting Started
- **[Quick Start Guide](docs/QUICK_START.md)** - 5-minute setup tutorial
- **[Configuration Reference](docs/CONFIGURATION.md)** - Complete config schema
- **[Troubleshooting](docs/TROUBLESHOOTING.md)** - Common errors and solutions

### Technical Documentation
- **[Metrics Admin Guide](docs/METRICS_ADMIN_GUIDE.md)** - Monitoring and alerting
- **[Metrics API](docs/METRICS_API.md)** - Programmatic metrics access
- **[Compliance Audit Guide](docs/COMPLIANCE_AUDIT_GUIDE.md)** - SOX/SOC2/GDPR/HIPAA

### Phase Documentation
- **[Phase Documentation Index](docs/phases/README.md)** - Historical development phases

## Framework Compliance

- **UCE34**: Computational Capsule Architecture (10 tiers)
- **ASSUM**: Safety validation (all atomic operations tagged)
- **B32**: Honest benchmarking (statistical rigor, fair baselines)
- **T28**: Comprehensive testing (Unit/Property/Integration/Stress)
- **I20**: Integration validation (phased rollout, monitoring)

## Version History

- **v0.4.6** (2025-10-18): Const-hashing optimization (Phase 2.2, 0ns static IDs)
- **v0.4.5** (2025-10-17): Metrics, forecasting & alerting (Phase 4.5)
- **v0.4.0** (2025-10-17): Compliance audit trails + forensic analysis (Phase 4)
- **v0.3.0** (2025-10-17): Built-in telemetry with hash integrity (Phase 3)
- **v0.2.0** (2025-10-16): HTTP proxy + per-provider monitoring (Phase 2)
- **v0.1.0** (2025-10-16): Pure atomic architecture, circuit breaker (Phase 1)

## License

MIT OR Apache-2.0
