# Primitives - Computational Capsule Foundation

Core primitives for lockfree, high-performance systems using computational capsule architecture.

## Chaos (Computational Capsule) - Quick Reference

See `/home/samuel/CLAUDE.md` § Mandatory Capsule Architecture for:
- Mandatory reading list (4 docs + XML frameworks)
- Core mandate (100% lockfree, cache-aligned 64B/128B/256B, generation counters)
- Foundation crate (`atomic_capsule/`: tiered alignment, `#[derive(ComputationalCapsule)]`, zero deps)
- Requirements (UCE34/ASSUM/B32/T28/I20/Q34 compliance)

## Mandatory P0 Protection for All Binaries

<mandatory-p0-protection priority="CRITICAL">
<rule>EVERY binary that contains capsule architecture, trade secret logic, or proprietary patterns MUST have full P0 protection (18 layers) before distribution.</rule>

<enforcement>
  <rule>NO binary shall be published to npm, deployed to production, or distributed without P0 protection verification</rule>
  <rule>Protection includes: AntiDebug (13 methods), Emulator Detection, License Validation, Self-Destruct</rule>
  <rule>EVERY binary build command MUST include --features client-protection or equivalent</rule>
  <rule>Pre-publish checklist: Verify protection with objdump, strings, binary size increase (~15-20%)</rule>
</enforcement>

<protected-binaries>
  <binary name="kdb_client_bridge">✅ P0 Protected (2.7MB)</binary>
  <binary name="kdb_configure">✅ P0 Protected (658KB)</binary>
  <binary name="kdb_handler">❌ MUST ADD (472KB → ~550KB with protection)</binary>
  <binary name="mcp_sse_server">❌ Server-side only, protection optional</binary>
</protected-binaries>

<rationale>
  Unprotected binaries expose:
  - Capsule architecture patterns (trade secret)
  - Detector logic and priority systems (trade secret)
  - URL parsing and validation algorithms (trade secret)
  - Terminal launch patterns (trade secret)
  - License format validation (security risk)
</rationale>

<violation-consequences>
  <immediate>Unpublish vulnerable version</immediate>
  <short-term>Add protection and republish</short-term>
  <long-term>Mandatory pre-publish protection verification in CI/CD</long-term>
</violation-consequences>
</mandatory-p0-protection>

## Mandatory Internal Dependency Usage

<mandatory-internal-dependencies priority="ABSOLUTE">
<rule>ALWAYS use internal Primitives dependencies. NEVER add external crates when internal equivalents exist.</rule>

<enforcement>
  <parallelism>
    <forbidden>rayon, tokio::spawn, std::thread::spawn (bare)</forbidden>
    <required>atomic_capsule::parallel (T4 Batch tier, 10-100× speedup, 100% lockfree)</required>
    <reason>External parallelism uses mutex/channels (100× slower). atomic_capsule::parallel uses lockfree queues with generation counters.</reason>
  </parallelism>

  <async-runtime>
    <forbidden>tokio, async-std, smol (as primary runtime)</forbidden>
    <required>atomic_capsule::runtime (T5 Streaming tier, <10ns coordination)</required>
    <reason>External runtimes use Arc/Mutex coordination. atomic_capsule runtime uses DualAtomicU64 capsules.</reason>
  </async-runtime>

  <collections>
    <forbidden>std::collections::HashMap, DashMap, parking_lot</forbidden>
    <required>atomic_capsule::collections (T1 Atomic tier, 3-59× speedup, 100% lockfree)</required>
    <examples>
      - ConcurrentMapCapsule (3-59× vs std::HashMap)
      - LockfreeHashTable (3.9× vs DashMap)
      - HistogramCapsule (50× vs std::HashMap)
    </examples>
  </collections>

  <hashing>
    <forbidden>std::hash::DefaultHasher, ahash, fxhash</forbidden>
    <required>atomic_capsule::hash (T0 Auditable tier, 0ns-8× speedup)</required>
    <examples>
      - const_hash (0ns compile-time hashing)
      - simd_hash (2-8× vs scalar)
      - AtomicHash64/256 (<50ns lockfree)
    </examples>
  </hashing>

  <simd>
    <forbidden>packed_simd, simdeez, wide</forbidden>
    <required>atomic_capsule::simd (T2 SIMD tier, 2-53× speedup, portable_simd)</required>
    <reason>External SIMD libs lack capsule integration. atomic_capsule SIMD uses cache-aligned capsules with generation counters.</reason>
  </simd>

  <probability>
    <forbidden>hyperloglog, bloom (external crates)</forbidden>
    <required>atomic_capsule::probabilistic (T10 Probabilistic tier, 99.97% memory reduction)</required>
    <examples>
      - HyperLogLog (99.97% memory reduction vs HashSet)
      - BloomFilter (<10ns queries, 0.083% FPR)
      - MinHash (23-30× vs Python datasketch)
    </examples>
  </probability>
</enforcement>

<exceptions>
  <allowed-external>
    <crate>criterion</crate>
    <reason>Benchmarking only, not production code</reason>
  </allowed-external>
  <allowed-external>
    <crate>proptest</crate>
    <reason>Property testing only, not production code</reason>
  </allowed-external>
  <allowed-external>
    <crate>serde</crate>
    <reason>Serialization standard, no internal equivalent</reason>
  </allowed-external>
  <allowed-external>
    <crate>thiserror, anyhow</crate>
    <reason>Error handling standard, minimal runtime</reason>
  </allowed-external>
</exceptions>

<violation-consequences>
  <performance>External dependencies cause 3-100× performance loss due to mutex/Arc/channels vs lockfree capsules</performance>
  <safety>External dependencies bypass Chaos verification (no #[derive(ComputationalCapsule)])</safety>
  <maintenance>External dependencies increase attack surface and audit complexity</maintenance>
  <compliance>External dependencies may violate UCE34/ASSUM/B32/T28 framework requirements</compliance>
</violation-consequences>

<verification>
  <command>cargo tree | grep -E "rayon|tokio|dashmap|parking_lot|ahash|packed_simd"</command>
  <expected>NO MATCHES (except test/bench dependencies)</expected>
  <action>If matches found, replace with atomic_capsule equivalents immediately</action>
</verification>
</mandatory-internal-dependencies>

## XML Documentation Best Practices

**Full Guide**: See `docs/XML_BEST_PRACTICES.xml` for LLM-optimized documentation standards.

**Quick Reference**: Use XML for APIs/deployments/test-reports. 10-30× faster queries, 100% accuracy, <20K token budgets. XPath validation mandatory (`xmllint --noout file.xml`).

## XML Framework Discovery (XPath)

**Index**: `docs/xml/INDEX.xml` | **Query Guide**: `docs/xml/XPATH_QUERIES.md` (82+ queries)

### File Inventory (5,500+ lines)

| Category | File | Lines | XPath Root |
|----------|------|-------|------------|
| **Origin** | `xml/origin/computational-capsule-philosophy.xml` | 669 | `//core-philosophy` `//tier-system` `//anti-patterns` |
| **Origin** | `xml/origin/atomic-capsule-patterns.xml` | 523 | `//named-patterns` `//design-rules` `//swemr-pattern` |
| **Origin** | `xml/origin/key-innovations.xml` | 850 | `//validated-innovations` `//unexploited-opportunities` |
| **Architecture** | `xml/metacapsule-patterns.xml` | 851 | `//pattern-catalog` `//topology-definitions` `//lifecycle-states` |
| **Architecture** | `xml/capsule-connections.xml` | 373 | `//connection-types` `//inter-tier-rules` |
| **API** | `xml/capsule-api-template.xml` | 538 | `//template` `//examples` |
| **API** | `xml/capsule-apis/*.xml` | 5 files | `//capsule-api` `//methods` `//state-transitions` |
| **Reference** | `METACAPSULE_ARCHITECTURE.xml` | 369 | `//topologies` `//lifecycle` `//coordination-protocols` |

### XPath Quick Queries

```bash
# Tier lookup
xmllint --xpath "//tier[@id='T1']" docs/xml/origin/computational-capsule-philosophy.xml

# Find capsule pattern by name
xmllint --xpath "//pattern[@id='ACB-64']" docs/xml/origin/atomic-capsule-patterns.xml

# Get all validated innovations with speedup
xmllint --xpath "//validated-innovations/innovation" docs/xml/origin/key-innovations.xml

# Metacapsule topology by ID
xmllint --xpath "//topology[@id='pipeline']" docs/xml/metacapsule-patterns.xml

# Connection type by latency
xmllint --xpath "//connection-types/type[latency]" docs/xml/capsule-connections.xml

# API methods by thread-safety
xmllint --xpath "//methods/method[@category='atomic']" docs/xml/capsule-apis/*.xml

# Lifecycle state transitions
xmllint --xpath "//lifecycle/transitions/transition" docs/METACAPSULE_ARCHITECTURE.xml

# Anti-patterns by severity
xmllint --xpath "//anti-pattern[@severity='critical']" docs/METACAPSULE_ARCHITECTURE.xml
```

### Common Discovery Patterns

| Need | XPath | File |
|------|-------|------|
| **Find tier speedup** | `//tier[@id='T2']/speedup` | `origin/computational-capsule-philosophy.xml` |
| **List all patterns** | `//named-patterns/pattern/@id` | `origin/atomic-capsule-patterns.xml` |
| **Get innovation details** | `//innovation[@id='1']` | `origin/key-innovations.xml` |
| **Metacapsule examples** | `//metacapsule-examples/example` | `METACAPSULE_ARCHITECTURE.xml` |
| **Connection APIs** | `//type[@id='direct']/api` | `capsule-connections.xml` |
| **Coordination protocols** | `//coordination-protocols/protocol` | `metacapsule-patterns.xml` |
| **State machine states** | `//lifecycle/states/state` | `METACAPSULE_ARCHITECTURE.xml` |
| **Design guidelines** | `//design-guidelines/guideline` | `METACAPSULE_ARCHITECTURE.xml` |

## Container Deployment CLI

**Remote**: kindly-hub:9000 (192.168.0.38) | **Daemon**: capsule-container-daemon (2.9MB) | **CLI**: capsule-cli (3.7MB) | **Protocol**: JSON-RPC 2.0/TCP | **Framework**: T6 Mixed (<20ns health, <50ms RPC)

**Hardware**: AMD Ryzen 9 6900HX, 64GB DDR5, Ubuntu 24.04 LTS | **Access**: `ssh samuel@kindly-hub` | **Services**: kindly-db, kindly-verified, kindly-web, http-server, protection

### CLI Commands (7)

| Cmd | Desc | Usage Pattern |
|-----|------|---------------|
| deploy-stack | Deploy all 5 services (dependency-ordered: db → [web,verified] → http → protection) | `capsule-cli deploy-stack --remote kindly-hub:9000 --config production.yaml` |
| start/stop | Start/stop service by ID | `capsule-cli {start\|stop} --remote kindly-hub:9000 <service-id>` |
| list | Show all services (state/PID/uptime/health) | `capsule-cli list --remote kindly-hub:9000` |
| logs | Get logs (10K lines/service capacity, optional follow) | `capsule-cli logs --remote kindly-hub:9000 <service> --lines N --follow` |
| health | Health check (<20ns atomic snapshot) | `capsule-cli health --remote kindly-hub:9000` |
| ps | Process-style service view | `capsule-cli ps --remote kindly-hub:9000` |

### SystemD Commands (7)

| Cmd | Desc | Pattern |
|-----|------|---------|
| status/start/stop/restart | Daemon control | `ssh samuel@kindly-hub "systemctl {cmd} capsule-container-daemon"` |
| enable | Auto-start on boot | `ssh samuel@kindly-hub "systemctl enable --now capsule-container-daemon"` |
| logs/recent | Daemon logs | `ssh samuel@kindly-hub "journalctl -u capsule-container-daemon [-f\|-n 100]"` |

**Build**: `cd capsule-os && cargo build --lib --features std,container --target x86_64-unknown-linux-gnu --release && cd daemon && cargo build --release && cd ../cli && cargo build --release`

**Deploy**: `ssh samuel@kindly-hub "mkdir -p ~/capsule-os/{bin,configs,systemd}" && scp target/.../capsule-{container-daemon,cli} samuel@kindly-hub:~/capsule-os/bin/ && ssh samuel@kindly-hub "sudo systemctl enable --now capsule-container-daemon"`

## Clippy Capsule Verification (Mandatory)

**Location**: `/home/samuel/Primitives/clippy-capsule-verify` | **Version**: 0.2.0-stable | **Impact**: 6-10× faster fixes, 40-150h saved/dev/year

**Mandate**: ALL P0 Critical lints MUST pass (deny level, blocks compilation). Pre-commit hooks MANDATORY.

### Lints (9 total: 4 P0 + 3 P1 + 2 P2)

| ID | Name | Impact | Fix |
|----|------|--------|-----|
| **P0.1** | capsule_mutex_violation | 100× perf loss (1-10μs vs <10ns) | Replace with AtomicU64/DualAtomicU64 |
| **P0.2** | capsule_unaligned_violation | 3-10× slowdown (false sharing) | Add 64B/128B/256B padding |
| **P0.3** | capsule_missing_generation | TOCTOU races, data corruption | Add generation: AtomicU64 |
| **P0.4** | capsule_non_atomic_field | Data races → UB (crashes) | u64→AtomicU64, bool→AtomicBool |
| P1.0 | missing_capsule_verification | Unverified layouts (size/align bugs) | Add #[derive(ComputationalCapsule)] |
| P1.2 | capsule_scattered_atomics | 2× perf loss (105ns vs 9.8ns) | Use DualAtomicU64 pattern |
| P1.3 | capsule_incorrect_padding | 3-5× perf loss (false sharing) | Match exact padding calculation |
| P2.1 | capsule_memory_ordering | 5-20% improvement available | Use Acquire/Release vs Relaxed |
| P2.2 | capsule_missing_assum | Audit compliance (SOX/SOC2/GDPR) | Add #ASSUME/#VERIFY tags |

**Commands**:
- **P0-only** (5-8s, pre-commit): `cargo clippy --all-features -- -D clippy::capsule_{mutex,unaligned,missing_generation,non_atomic}_violation`
- **P0+P1** (15-25s, pre-push): Add `-W clippy::{missing_capsule_verification,capsule_scattered_atomics,capsule_incorrect_padding}`
- **Comprehensive** (20-30s, CI/CD): Add `-W clippy::{capsule_memory_ordering,capsule_missing_assum}`

**CI/CD Setup**: Run `./scripts/setup-ci.sh` (auto-configures GitHub/GitLab/hooks). See `CI_CD_AUTOMATION.md` for details.

**Metrics**: 51/51 tests ✅ | 90-95% detection | <5% false positives | 5-8s pre-commit | 15-25s CI/CD | 100% Chaos compliance

**Troubleshooting**: Unknown lint? Use -D flags, not .clippy.toml. Slow? Use P0-only for pre-commit. False positive? Use `#[allow(clippy::lint_name)]`.

**Documentation**: See `clippy-capsule-verify/{ERROR_MESSAGE_GUIDE,BEFORE_AFTER_EXAMPLES,CI_CD_AUTOMATION,TESTING_GUIDE,ATOMIC_CAPSULE_INTEGRATION}.md`

## Recent Sessions

| Date | Achievement | Capsules Added | Tests | Status | Details |
|------|-------------|-----------------|-------|--------|---------|
| 2025-11-23 | HTTP/3 + QUIC Stack | +22 QUIC, +2 HTTP/3 | 56/56 | ✅ Production | [Full Summary](legacy/sessions/SESSION_2025-11-23.md) |
| 2025-11-22 | Container Deployment + SystemD | +SystemdServiceCapsule (T1) | 28/28 | ✅ Production | [Full Summary](legacy/sessions/SESSION_2025-11-22.md) |
| 2025-11-14 | Debugger + MCP + Async Runtime | +43 capsules, 2 new projects | 175+ | ✅ Production | [Full Summary](legacy/sessions/SESSION_2025-11-14.md) |

**See**: `legacy/sessions/` for comprehensive session archive with full details, performance metrics, and framework compliance reports.

## Capsule Tiers

**Canonical Reference**: See `/home/samuel/CLAUDE.md` § Capsule Tiers for complete 12-tier taxonomy (T0-T11) and `xml/shared/shared-components.xml` for tier definitions, decision trees, and performance claims.

**Quick Reference**: T0 (Auditable, 0ns) → T1 (Atomic, 3-10×) → T2 (SIMD, 2-19×) → T3 (Fixed-Point, 2-10×) → T4 (Batch, 10-100×) → T5 (Streaming, O(1)) → T6 (Mixed, 50-100×) → T7 (Heterogeneous, 100-1000×) → T8 (Network, 10-50×) → T9 (Persistent, ACID) → T10 (Probabilistic, 100-1000×) → T11 (QuantumHybrid, 10-16,667×)

## Metacapsule Architecture Pattern

**Definition**: Orchestrating capsule with 4-18 embedded sub-capsules for multi-stage pipelines. Lockfree hierarchical state coordination via DualAtomicU64 + phase bitmasks. Prevents impossible states at compile-time.

**Use When**: Multi-stage pipeline (3+ stages) OR atomic snapshot required OR complex FSM (8+ states) OR real-time constraints (<100ms SLA).

**Pattern Comparison**:
- **Metacapsule** (256B-1024B): Multi-stage orchestration, <50ns snapshot, 2-20× speedup (compound tier effects)
- **Component** (64B-256B): Single-purpose primitive, <10ns snapshot, 2-19× speedup (single tier)
- **Container** (variable): Large collections (≥100K objects), <1μs snapshot, 10-100× speedup (T4 Batch)

**Topologies**: Pipeline (A→B→C) | Mesh (A↔B↔C) | Fanout (A→[B,C,D]) | Hierarchical (tree)

**Lifecycle States**: Uninitialized → Initializing → Ready → Processing → Draining → Stopped | Error | Failed

**Coordination Protocols**: Sequential (O(n)) | Parallel (O(1)) | Pipelined (O(1) after warmup) | Speculative

**Connection Types**: Direct (<10ns, 1:1) | Pipeline (<50ns/stage, N:1) | Broadcast (<100ns, 1:N) | Mesh (<200ns/hop, N:N) | Request-Response (<1μs, async)

**Examples**: Av1EncoderMetacapsule (T6, 18 subs, 2-20×) | QuicEndpointMetacapsule (T6, 22 subs, 1.76×) | UniversalApiMetaCapsule (T6, 6 protocols, 1.2×) | PNGEncoderCapsule (T6, 3 subs, 2-5×)

**Best Practices**: ≤1024B orchestrator | Acyclic dependency graph | Atomic snapshot before transition | Phase bitmasks for FSM | T28 5-tier testing (including Q29-Q35 determinism) | Generation counters on DualAtomicU64

**Full Spec**: `docs/METACAPSULE_ARCHITECTURE.xml` (v2.0, 369 lines) | **Patterns**: `docs/xml/metacapsule-patterns.xml` (851 lines, 4 patterns, 4 topologies, 8 states, 4 protocols)

**XPath Quick Access**:
```bash
xmllint --xpath "//topology[@id='pipeline']" docs/xml/metacapsule-patterns.xml
xmllint --xpath "//lifecycle/states/state" docs/METACAPSULE_ARCHITECTURE.xml
xmllint --xpath "//coordination-protocols/protocol[@id='pipelined']" docs/xml/metacapsule-patterns.xml
```

## Phase Status

| Phase | Status | Achievement |
|-------|--------|-------------|
| **2.1: SIMD + Fixed-Point** | ✅ Production | 4 capsules, 2-4× speedup, 266 tests |
| **2.2: Nightly Optimizations** | ✅ Production | Const-hash (0ns), SIMD-hash (2-8×) |
| **2.3: atomic_from_mut** | ✅ Production | Zero-copy atomics (mmap/shared mem) |
| **Phase 4: FixedPointSerialize** | ✅ Production | Q34 audit trails (<50ns) |
| **Phase 5.0-5.3: Collections** | ✅ Production | 7 lockfree capsules (3-59× speedup) |
| **Phase 5.4: Memory Ordering** | ✅ Production | 116/116 tests, 99.99% safe |
| **AVX2 Quantization** | ✅ Production | 5.2-5.5× speedup (EXCEPTIONAL) |
| **Phase 6.0: HTTP/3 + QUIC** | ✅ Production | 22 QUIC + 2 HTTP/3 capsules, 1.76× speedup, 56/56 tests |

## Key Modules

**Comprehensive Documentation**: See `/home/samuel/Primitives/atomic_capsule/CLAUDE.md` for detailed APIs, examples, and feature flags for all 328 primitives.

### Core Primitives (atomic_capsule)

| Module | Tier | Key Capsules | Speedup | Tests | Status |
|--------|------|--------------|---------|-------|--------|
| **Collections** | T1+T5 | HistogramCapsule (50×), ConcurrentMapCapsule (3-59×), LockfreeHashTable (3.9×), StatsCapsule64 (1.3-5.7×), RingBufferBroadcast (2-5×), AsyncLogCapsule (20-100×), LockfreeCacheCapsule (3-10×) | 3-100× | 116/116 | ✅ Production |
| **Hash** | T0 | const_hash (0ns compile-time), simd_hash (2-8×), AtomicHash64/256 (<50ns lockfree), keyed_hash (HMAC-SHA256), ConstHashCapsule (T1 integration) | 0ns-8× | 28/28 | ✅ Production |
| **Time-Travel** | T0+T1 | ReplayEngineCapsule<T> (generic bidirectional replay, 5-8ns snapshot, 1024 capacity, Q34 hash-chain integrity) | 200-1000× | 8/8 | ✅ Production |
| **Ring Buffer** | T5 | RingBufferCapsule<T> (generic streaming, <10ns append, 16K capacity, TraceEntry built-in, lockfree wraparound) | <10ns | 14/14 | ✅ Production |
| **Circuit Breaker** | T1+T3 | CircuitBreaker (9 packed fields, Q8.8 fixed-point metrics, 8 bytes, <5ns load, <15ns update SWeMR) | <5ns load | 28/28 | ✅ Production |
| **AVX2 Quantization** | T2 | quantization_avx2 (5.2-5.5× EXCEPTIONAL speedup, 0.37-0.40ns per-element, 2.5-2.8 Gelem/s throughput) | 5.2-5.5× | 28/28 | ✅ Production |
| **QUIC Stack** | T8 | 22 capsules (RFC 9000/9114/9221 compliant, QuicEndpointMetacapsule orchestration, 100% lockfree, 1.76× vs TLS 1.3) | 1.76× | 56/56 | ✅ Production |
| **HTTP/3** | T6 | UniversalApiMetaCapsule (6 protocols: HTTP/1.1, HTTP/2, HTTP/3, WebSocket, gRPC, GraphQL, <10μs RPC latency) | Multi-protocol | 28/28 | ✅ Production |

### External Projects

| Project | Location | Tier | Purpose | Speedup | Status |
|---------|----------|------|---------|---------|--------|
| **kindly_dedup** | `/home/samuel/Primitives/kindly_dedup/` | T10 | LLM training dataset deduplication (MinHashSignatureCapsule, LSH bucketing, Union-Find clustering) | 38× single-threaded, 190× multi-threaded | ✅ Production |
| **kindly_dedup_stripe** | `/home/samuel/Primitives/kindly_dedup_stripe/` | T1 | Stripe payment webhook handler (Axum server, EarlyAdopterCounter <10ns increment, license generation) | <10ns | ✅ Deployed (Fly.io) |
| **kindly-web** | `/home/samuel/Primitives/kindly-web/` | — | Leptos 0.7 WASM marketing site (Byzantine Royal purple design, 665KB WASM bundle, <750ms LCP) | — | ✅ Deployed (Fly.io) |

**API Examples, Usage Patterns, Feature Flags**: See atomic_capsule/CLAUDE.md for comprehensive documentation of all modules.

## Verification (v0.4.0)

**Automatic**: `#[derive(ComputationalCapsule)]` (0ns runtime, <20ms compile-time)

**Crates**: `atomic_capsule_derive` (560 lines), `clippy-capsule-verify` (475 lines, ~95% detection)

**Migration**: 618 manual macros → automatic derive (87.5% code reduction), 7 projects remaining

**Timeline**: v0.4.0 (current) → v0.5.0 (manual macros deprecated) → v0.6.0 (manual macros removed)

## Cross-Project Capsule Patterns

**Tier Usage Across Projects**:

- **T1 (Atomic)**: kindly_hft (7 brain zones), fqbit (phi-resonance), trading (order routing)
- **T2 (SIMD)**: kindly_hft (19× Hebbian BREAKTHROUGH, 7× CSR), fqbit (hash chains), trading (OHLC)
- **T3 (Fixed-Point)**: kindly_hft (83.4ns P&L, Q16.16 STDP), trading (Kelly criterion, Q16.16)
- **T4 (Batch)**: kindly_hft (12× parallel training, 57× atomic updates EXCEPTIONAL), fqbit (block validation)
- **T5 (Streaming)**: kindly_hft (incremental CSR <40GB), trading (real-time P&L)
- **T6 (Mixed)**: kindly_hft (full brain 50-100× BREAKTHROUGH, 2.75× compression)

**Notable Breakthroughs**:
- T2 SIMD: 19× Hebbian learning (2.5ns/connection vs 47.9ns scalar)
- T4 Batch: 57× zone-level atomic updates (10μs vs 570μs)
- T6 Mixed: 50-100× full training (all 5 tiers compound)

## Pattern Documentation

Production patterns documented in:
- `docs/ATOMIC_CAPSULE_PATTERNS.md` - 5 production patterns (ACB-64, APC-512, RLT-1024, AEB-512, PNL-512)
- `docs/ATOMIC_CAPSULE_COMPOSITION.md` - Safe composition patterns and anti-patterns
- `docs/ATOMIC_CAPSULE_FAILURE_MODES.md` - Failure analysis and recovery strategies

## Tools and Automation

### fix_padding_fields (Phase 2 Migration)

**Location**: `/home/samuel/Primitives/tools/fix_padding_fields/`

**Purpose**: Automated padding calculation for computational capsule migration (v0.5.0 → v0.6.0).

**Status**: ✅ Production Ready (9/9 tests passing, zero clippy warnings).

**Quick Start**: See `tools/fix_padding_fields/README.md` for comprehensive guide, commands, examples, and troubleshooting.

## Framework Compliance

See `/home/samuel/CLAUDE.md` § Performance & Validation Standards for complete details on:
- **UCE34**: Q1-Q34 systematic discovery, tier selection (Q10-Q12)
- **Chaos**: 100% lockfree, no mutex/RwLock, cache-aligned, generation counters
- **ASSUM**: 99.5%+ safety, all assumptions verified (#ASSUME → #VERIFY)
- **B32**: 95% CI, 1000+ iterations, fair baselines (not strawman), reproducibility
- **T28**: 5-tier testing (unit/property/integration/production/determinism)
- **I20**: Integration validation (20/20 questions, zero breaking changes)
- **Q34**: Hash-chained audit trails (SOX/SOC2/GDPR/HIPAA compliance)

## Testing

```bash
# All features (comprehensive)
cargo test --lib --all-features

# Stable only (no nightly)
cargo test --features "tier2,tier3"

# Nightly features
cargo test --features "nightly-all"

# Collections (116 tests)
cargo test --features "std,async-log,cache,histogram"

# Specific module
cargo test frequency::tests
```

## Documentation

**Core Frameworks**: See `/home/samuel/CLAUDE.md` § Mandatory Reading Framework for canonical UCE34, Chaos, ASSUM, B32, T28, I20, Q34 documentation.

**Phase Reports**: See `atomic_capsule/CLAUDE.md` for comprehensive session summaries (Nov 14/22/23), phase status, verification reports, and Chaos compliance validation.

**Pattern Guides**: `docs/ATOMIC_CAPSULE_PATTERNS.md`, `docs/ATOMIC_CAPSULE_COMPOSITION.md`, `docs/ATOMIC_CAPSULE_FAILURE_MODES.md`

## Trade Secret Notice

Some components protected as trade secrets:
- `atomic_hedge_capsule` - Full trade secret protection
- `kindly_hft` - Strategic algorithms protected

**MANDATORY**: Never commit trade secret components to public repositories. All commits must use `[TRADE SECRET]` tag.

## Performance Standards

See `/home/samuel/CLAUDE.md` § Performance & Validation Standards for:
- **B32 Framework**: 95% CI, 1000+ iterations, fair baselines, reproducibility
- **Reality Check**: 10-50% typical, 2-10× exceptional, 100×+ extensive validation
- **ASSUM Framework**: Every #ASSUME needs #VERIFY, 99.5%+ safety target

## References

**Foundation**: `/home/samuel/Docs/The Computational Capsule.md`, `/home/samuel/Primitives/Docs/KEY_INNOVATIONS.md`

**Universal Config**: `/home/samuel/CLAUDE.md` - UCE34 framework v6.0 (XML canonical source)

**Project Configs**: `kindly_hft/CLAUDE.md`, `kiang/CLAUDE.md`, `clapi_core/CLAUDE.md`, `kindly_dash/CLAUDE.md`
