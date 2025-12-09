# clapi_core Configuration (Lean v0.4.9)

**Status**: Production-Ready | **Framework**: UCE34 T1-T6 | **Architecture**: 100% lockfree, zero panics

## Architecture

**Core Pattern**: Box<[BudgetSlotCapsule; 1M]> + AtomicPtr (all paths lockfree)

**Capsules** (14 total):
| Phase | Name | Size | Tier | Perf | Purpose |
|-------|------|------|------|------|---------|
| 1 | BudgetSlotCapsule | 128B | T1 | 10-30× | Lockfree slot mgmt |
| 1 | CircuitBreakerCapsule | 64B | T1 | <5ns | Circuit breaker state |
| 1 | REQ-128 | 128B | T1 | 3-5× | Request validation |
| 1 | RTE-128 | 128B | T1 | 3-8× | Provider routing |
| 1 | RES-256 | 256B | T2+T3 | 4-12× | Response metrics |
| 1 | ALE-128 | 128B | T5 | 10-100× | Audit log streaming |
| 1 | ET-1KB | 1KB | T4+T3 | 10-20× | Cost aggregation |
| 2 | CircuitBreakerMetrics | 64B | T1 | <20ns | Metrics export |
| 2 | ProviderCircuitStatus | 64B | T1 | <20ns | Per-provider state |
| 2 | ProviderCircuitArray | 1KB | T4 | <300ns | 16 provider circuits |
| 4 | OAuthSessionCapsule | 128B | T1 | <50ns | OAuth 2.0 sessions |
| 4 | PaymentCapsule256 | 256B | T3 | <150ns | Stripe payments (Q16.16) |
| 4 | RateLimitCapsule | 64B | T1 | <40ns | Token bucket limits |
| 4 | CompressionStateCapsule | 512B | T4 | O(1) | Compression state |

**Memory**: 128MB preallocated (1M × 128B), zero hot-path allocations

## Performance Targets

**Lockfree Operations**:
- Budget check: <100ns (~60ns actual, 3× vs RwLock)
- Slot allocation: <100ns (~80ns actual, 3-4× vs RwLock)
- Circuit breaker: <10ns (~5ns actual)
- Deallocation: <100ns (~90ns actual, 2-3× vs RwLock)

**Scalability**: 10M-60M ops/s (1-8 threads), P99 120-200ns

**Hot Path**: <300ns total (0.3% of 100ms provider latency)

## Circuit Breaker Configuration

| Metric | Value |
|--------|-------|
| Open threshold | >10% (1000 bp) |
| Half-open | 5-10% (500-1000 bp) |
| Close threshold | <5% (500 bp) |
| Cooldown | 60s |
| Min samples | 10 |

**States**: Closed (0) | HalfOpen (1) | Open (2)

## Features & APIs

**HTTP Endpoints**:
- `GET /metrics` - All metrics (JSON)
- `GET /metrics/circuit_breaker` - Circuit breaker only
- `GET /health` - Health check + provider status
- `POST /v1/chat/completions` - OpenAI-compatible API

**CLI Commands**:
```
clapi start [--config] [--test] [--listen] [--budget]
clapi config            # Interactive 3-step wizard
clapi doctor            # 6 system diagnostics
clapi metrics --watch N # Real-time dashboard
clapi budget            # Budget management
clapi providers         # Provider management
clapi audit             # Audit log viewer
```

**Test Mode**: `clapi start --test` (zero-config, mock responses, no API keys needed)

**Branding**: Byzantine Purple (#663399) + Gold (#FFD700) accents on clapi.dev

## Implementation

**HTTP Layer**: Axum + Tokio + Reqwest (connection pooling)

**Dependencies**:
- atomic_capsule (foundation)
- criterion (benchmarks)
- proptest (property tests)
- dashmap (concurrent hashmap, Phase 1 only)
- serde_json, toml, clap (config/CLI)
- blake3, xxhash-rust (crypto, feature-gated)

**Phase Migration**:
- Phase 1 ✅: Pure atomic budget registry (100% lockfree)
- Phase 2 ✅: HTTP proxy + per-provider circuits
- Phase 3 ✅: Built-in telemetry + hash integrity
- Phase 4 ✅: Compliance audit trails + OAuth + Stripe + Rate limiting
- Phase 4.5-4.7 ✅: OAuth sessions, Payments (Q16.16), Rate limiting
- Phase 2.2 ✅: const-hashing optimization (0ns static IDs, 1.77 G/s dynamic)

## Testing & Validation

**T28 Framework**:
- Tier 1: 200+ unit tests (capsule invariants)
- Tier 2: 1000-thread property tests
- Tier 3: End-to-end integration tests
- Tier 4: 1M cycle stress tests

**B32 Benchmarks**: Fair baselines (RwLock HashMap comparison), 1000+ iterations, 95% CI, honest 10-30% claims

**ASSUM Framework**: Memory ordering (Acquire/Release), generation counters (ABA prevention), all assumptions documented

**Compilation**: Zero warnings, all 365 tests pass, 24 benchmark suites

## Frameworks Applied

| Framework | Coverage | Status |
|-----------|----------|--------|
| UCE34 | Q1-Q34 (Tiers 1-6) | ✅ Complete |
| ASSUM | All atomic ops tagged | ✅ 99.99% safe |
| B32 | Fair baselines, rigor | ✅ Honest claims |
| T28 | 4-tier test pyramid | ✅ 365 tests pass |
| I20 | Q1-Q20 integration | ✅ Capsule verified |

## Files & Modules

**Core**: `src/proxy/config.rs` (143), `types.rs` (190), `client.rs` (118), `budget_registry.rs` (197), `provider_router.rs` (188), `audit_log.rs` (184), `server.rs` (258)

**CLI**: `src/bin/clapi.rs` (36), `test_mode.rs`, `cli_commands.rs`

**Verification**: `WEEK3_VERIFICATION_REPORT.md`, `ROLLOUT_PLAN.md`

## Deployment

**Strategy**: I20-Capsule (big bang 100%, no canary - deterministic code)

**Rollout Timeline**:
- Week 1: Proxy baseline (0 risk)
- Week 2: OAuth (1% → 100%, LOW risk)
- Week 3: Stripe (10% → 100%, MEDIUM risk)
- Week 4: Full compliance (100%, LOW risk)

**Rollback**: <1 min (feature flag) or <5 min (git revert)

## Mandatory Reading

1. `/home/samuel/Docs/The Computational Capsule.md` - Philosophy
2. `/home/samuel/Primitives/Docs/KEY_INNOVATIONS.md` - Proven results
3. UCE34 Framework + Tier Reference + Examples (tier selection)
4. ASSUM Safety (memory ordering, atomics)
5. B32 Benchmarking (honest performance claims)
