# Primitives - Computational Capsule Foundation

Core primitives for lockfree, high-performance systems using computational capsule architecture.

## Chaos (Computational Capsule) - Quick Reference

See `/home/samuel/CLAUDE.md` § Mandatory Capsule Architecture for:
- Mandatory reading list (4 docs + XML frameworks)
- Core mandate (100% lockfree, cache-aligned 64B/128B/256B, generation counters)
- Foundation crate (`atomic_capsule/`: tiered alignment, `#[derive(ComputationalCapsule)]`, zero deps)
- Requirements (UCE34/ASSUM/B32/T28/I20/Q34 compliance)

## XML Documentation Best Practices

**Full Guide**: See `docs/XML_BEST_PRACTICES.xml` for LLM-optimized documentation standards.

**Quick Reference**: Use XML for APIs/deployments/test-reports. 10-30× faster queries, 100% accuracy, <20K token budgets. XPath validation mandatory (`xmllint --noout file.xml`).

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

**Examples**: Av1EncoderMetacapsule (T6, 18 subs, 2-20×) | QuicEndpointMetacapsule (T6, 22 subs, 1.76×) | UniversalApiMetaCapsule (T6, 6 protocols, 1.2×) | PNGEncoderCapsule (T6, 3 subs, 2-5×)

**Best Practices**: ≤1024B orchestrator | Acyclic dependency graph | Atomic snapshot before transition | Phase bitmasks for FSM | T28 5-tier testing (including Q29-Q35 determinism) | Generation counters on DualAtomicU64

**Full Spec**: See `docs/METACAPSULE_ARCHITECTURE.xml` for complete architecture, when-to-use criteria, 6 advantages, anti-patterns.

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

**Dependency policy**: Before adding external crates, look for an equivalent capsule in `atomic_capsule` and prefer the in-tree primitive.

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

## Current Work: Kindly_Rub (Motion Sequencer)

- Location: `Kindly_Rub/` (workspace member).
- Capsules: `MotionBlockCapsule` (64B atomic), `TimelineCapsule` (snap grid ms/BPM, stretch handles, duplicate/invert/loop shortcuts, tempo/duration hints), `Sampler` (Hermite/PCHIP positions + vel/accel), `Inflection` (ImpactHigh/Low/Buzz), `Overlay` (HUD themes light/dark/neon, jitter on vibration, ghost next impact, latency indicator), `Audio` (per-preset gain/eq, density limiter, hover pre-listen, click preview), `Funscript` (clamp/monotone/density validation + manual JSON, no external deps), `Renderer` (internal pipeline only; funscript writer + ExportReport; video/audio mux stays in-tree media backend, never external FFmpeg).
- Capsules: `MotionBlockCapsule` (64B atomic), `TimelineCapsule` (snap grid ms/BPM, stretch handles, duplicate/invert/loop shortcuts, tempo/duration hints), `Sampler` (Hermite/PCHIP positions + vel/accel), `Inflection` (ImpactHigh/Low/Buzz with tunable thresholds), `Overlay` (HUD themes light/dark/neon, jitter on vibration, ghost next impact, latency indicator), `Audio` (per-preset gain/eq, density limiter, hover pre-listen, click preview), `Funscript` (clamp/monotone/density validation + manual JSON, no external deps), `Renderer` (internal pipeline only; funscript writer + ExportReport; video/audio mux stays in-tree media backend, never external FFmpeg).
- Presets: `PresetLibraryCapsule` (name → MotionBlock) with auto sparkline preview, tags (full/top/bottom/vibration), generation label, per-preset gain/eq metadata; timeline instantiation keeps meta for audio/render hints.
- App/UI hooks: `KindlyRubAppCapsule::run_pipeline[_with_profile/_with_options]` produces HUD/audio/funscript JSON + validation + render plan (Simple/Advanced profile, optional output path); `run_and_render[_with_profile/_with_options/_from_ui]` writes `.funscript` via internal renderer and returns `RenderOutput` (counts/paths/report + media mux result). `live_preview`/`ui_preview_bundle` feed HUD snapshot + ghost indicator + light click + hover pre-listen for realtime sync without full render. Inflection tuning is configurable for real-footage calibration. Renderer exposes `set_mux_handler` (pluggable in-tree media muxer) and `use_internal_backend` fallback—never external FFmpeg.
- HUD rasterizer: `HudRasterizerCapsule` builds RGBA frames (960x540 simple, 1280x720 advanced) from HUD overlay (ribbon, cursor, indicators, ghost, jitter) for internal muxing; RenderPlan now carries width/height.
- Media mux: `WebmMuxAdapterCapsule` writes WebM header + tracks + clusters using in-tree muxer (no FFmpeg); includes downsampled HUD rasters (RLE-compressed RGBA tagged VP9) and a stub Opus audio track from synthesized PCM (grains → envelope → PCM16, density limited).
- Smoke command: `cargo run -p kindly_rub --bin smoke [out_dir]` produces HUD-only WebM header (in-tree mux) plus `.funscript` and checks non-zero sizes.
- UX: timeline UI snapshot refreshes duration/tempo, returns `grid_lines_ms` derived from snap grid/BPM + zoom for HUD grid; drag/stretch hint stays aligned. Calibration now returns `CalibrationReport` (measured buzz rate/interval, tuned thresholds) for UI.
- Export: renderer writes `export_report.json` alongside `.funscript` with duration/impact/buzz counts and validation flags; `RenderOutput` carries report path/bytes; app exposes `ui_export_summary`/`export_panel_data` (report JSON, paths, funscript size + validation) and `calibrate_and_export_summary` (calibration + export bundle) for UI panels; internal mux unchanged (atomic_capsule mux only). Smoke prints report summary.

## Current Work: Capsule Cache (Redis-style)
- Location: `capsule_cache/` (workspace member, Proprietary/Trade Secret). Rust nightly pinned via `rust-toolchain.toml`.
- Goal: Redis-like cache using computational capsules; lockfree, cache-line aligned, generation-tagged; **dependencies: internal `atomic_capsule` only**.
- Capsules: `LockfreeCacheCapsule` + `StatsCapsule64` + `HistogramCapsule`; optional integrity/multi-tenant/encryption/distributed features forwarded to `atomic_capsule`.
- Features shipped: AOF append/replay (`AOF_PATH`), modulo sharding (`SHARDS`, `SHARD_CAPACITY`), RESP inline/array parsing, AUTH token, rate limit (2000 ops/s), commands: PING/SET/GET/DEL/TTL/EXPIRE/INCR/MSET/MGET/STATS/SLOWLOG/FLUSHDB. Aggregated latency percentiles and bounded slowlog (RingBufferCapsule, `SLOWLOG_US` threshold) with optional export (`SLOWLOG_PATH`) included.
- Roadmap: distributed/quorum profile (no new deps), benchmarks (<120ns hit / <220ns insert), admin ops (SCAN-lite/KEYS-lite), systemd/capsule runtime smoke.

## Tools and Automation

### fix_padding_fields (Phase 2 Migration)

**Location**: `/home/samuel/Primitives/tools/fix_padding_fields/`

**Purpose**: Automated padding calculation for computational capsule migration (v0.5.0 → v0.6.0).

**Status**: ✅ Production Ready (9/9 tests passing, zero clippy warnings).

**Quick Start**: See `tools/fix_padding_fields/README.md` for comprehensive guide, commands, examples, and troubleshooting.

## Kindly-Engine (Total War / Paradox Parity Plan)

**Context**: Kindly-Engine targets large-scale, deterministic Napoleonic battles (100% lockfree, cache-aligned). Compared to Total War (RTS battles + light campaign) and Paradox titles (grand strategy/diplomacy/economy), the engine is battle-strong but needs campaign/diplomacy depth and richer ops AI.

**Gaps vs TW/Paradox**: campaign layer (economy/diplomacy, war exhaustion), siege/fortification play, fog-of-war/intel loops, operational AI pacing, logistics over time (attrition/supply lines), replay/telemetry for AI intent, and doctrine-aware scripting.

**Capsule/Tier Roadmap (UCE34/Chaos)**:
- `BattleAiCapsule` (T4/T6 metacapsule): foundation shipped (bounded decisions, generation counters). Next: log AI decisions to replay (0xC900), inject doctrine/stance/threat maps, and expose order rate telemetry.
- `FogOfWarCapsule` (T2/T5): SIMD LOS sampling + streaming intel deltas; feeds AI threat maps and campaign visibility. Align 64B/128B per-shard, generation-tagged snapshots.
- `OpsC2Metacapsule` (T1/T4/T6): command latency, courier reliability, priority queues; bridges player/AI orders with shard OrderQueueCapsules. Enforce cadence gates (no mutex/RwLock) and dual-generation ids. **Status: guardrails pass 1** (order-rate backpressure surfaced to overlays/replay, courier reliability feedback into command stress).
- `SiegeCapsule` (T3/T4): fortification integrity, breach progress, sapper engineering, morale shocks; batch updates per wall section, fixed-point damage to stay deterministic. **Status: pass 1 shipped** (integrity/attrition capsules, snapshot v11, replay payload 0xC820, engineering sap/repair hooks, breach/seal on structures).
- `LogisticsCapsule` (T4/T5): supply depots, baggage trains, ammo/water/food decay, attrition ticks; streaming counters for campaign stats; generation counters for audit. **Status: pass 1 shipped** (per-road integrity/disruption tracking, throughput snapshots, logistics-driven command-delay penalties, replay tag 0xC230).
- `CampaignMetacapsule` (T6/T9/T10): provinces/economy/diplomacy/war exhaustion; orchestrates fog-of-war + logistics + ops AI; uses Replay/Index capsules for persistent saves and probabilistic events (T10 guarded by audit trails).

**Implementation Order (one-by-one)**:
1) Close loop on `BattleAiCapsule`: replay logging + telemetry counters.  
2) Add `FogOfWarCapsule` + threat/stance maps feeding battle AI.  
3) Harden `OpsC2Metacapsule`: order-rate guardrails, courier reliability, shard-level backpressure.  
4) Introduce `SiegeCapsule` + engineering hooks (bridges to `engineering.rs`/structures). **(DONE pass 1: integrity/breach progress, replay tag, sap/repair hooks)**
5) Stand up `LogisticsCapsule` (supply lines, attrition, resupply ticks). **(DONE pass 1: throughput + disruption/attrition events, command-delay penalties, replay payload)**  
6) Layer `CampaignMetacapsule` for diplomacy/economy/war exhaustion and Paradox-style pacing. **(STARTED pass 1: campaign metacapsule orchestrates strategic+diplomacy+economy, war exhaustion → province resistance, hash-chained bundle snapshots)**  
7) Expand replay/telemetry (T5/T9) to capture AI intent, doctrine switches, and C2 delays for B32 honesty.

**Validation**: Each capsule follows UCE34 discovery (Q1-Q34), Chaos mandates (atomic + generation + alignment), T28 determinism (replay equivalence), B32 honesty (p50/p99/p999 tick latency), and metacapsule checks (acyclic orchestration, <1024B coordinator, snapshot-before-transition).

### Recent Kindly-Engine Progress
- Added `FogOfWarCapsule` + `FogOfWarView`: LOS/visibility filters AI targets; shard stats/logs now include contacts/visibility ratios and AI replay payloads.
- Battle AI intent overlays: threat centroid tiles + stance histogram + doctrine mode/generation flow into shard stats/overlays and replay tag 0xC901 (helpers for analytics series). Doctrine recommendation now derived from intent/visibility/courier latency; NDJSON helper provided for dashboards.
- Logistics pass 1: per-road integrity/disruption with attrition/repair, throughput + command-delay penalties surfaced to ticks/overlays/replay (tag 0xC230), supply snapshots now carry penalties and throughput for command delay/morale hooks.
- Added `GeneralCapsule` (aura morale/fatigue) + `snapshot_generals` helper and test; shard tick applies auras from snapshots (no driver-held generals).
- Added `CommanderCapsule` + `CommandHierarchyCapsule` with `commanders_to_generals` conversion; driver spawns commander, snapshots each tick, and feeds shard contexts.
- Added `StrategicMapCapsule` + `ProvinceCapsule`: supply graph, weather scripts (optional wind), hash-chained `StrategicSnapshot`; driver seeds ammo/depot pressure, steps map each tick, and passes `SupplySnapshot` into shards/io_uring demo.
- Strategic map depth: provinces now track supply output + resistance + generation; supply injection scales with infrastructure/resistance, decay penalizes low infra/high resistance, and hash-chains include prev hash + tick/generation. Shard contexts/overlays carry strategic hash + province averages; new tests cover capture/resistance + strategic propagation.
- Ops backpressure guardrail: ready-order cap tracks `ops_backpressure_drops` on overlays/KGPU and replay (tag 0xC342); congestion buckets added to overlays/KGPU and replay (0xC343) with fair scheduling of ready orders. Siege face events tagged (0xC821) for breach/repair overlays.
- C2/fog telemetry overlays: threat_pressure_q16 derived from fog visible ratio/contacts plus command_delay/courier ETA p95 buckets now flow into Shard/KGPU overlays and WorldFrame; courier reliability defaults to healthy when no samples to avoid cold-start penalties. P95 buckets logged to replay (0xC324/0xC325).
- Threat overlays: threat_pressure/fog visibility exposed via `make_threat_overlay_from_render` for KGPU/NDJSON dashboards.
- Logistics route cuts: clustered supply disruptions raise command stress/courier ETA, surface on overlays, and emit replay events (0xC231).
- Congestion penalty: ops_congestion_bucket feeds command stress + courier ETA scaling; replay tag 0xC343 added. NDJSON hooks in driver (`INTENT_NDJSON=1`, `DASHBOARD_NDJSON=1`) stream AI intent and combined threat/ops snapshots for dashboards.
- Command chain integration (pass 1): driver builds `CommandHierarchyCapsule`, assigns commanders to formations, and feeds commander snapshots into shards. `tick_shard` now applies command-delay penalties when out of range (raises command stress/courier ETA) while reusing general auras for morale/fatigue; tests cover in-range vs out-of-range effects.
- Strategic events + replay: `StrategicSnapshot` now emits ownership/infra-repair events, persists them in snapshots (v7), and logs replay payloads for audit trails (hash-chained).
- Command delay telemetry: `CommandDelayBufferCapsule` gates delayed orders, per-shard/world stats track delay histograms, applied counts, and averages, and replay logs expose histogram chunks + applied payloads.
- Diplomacy core: `DiplomaticStateCapsule` (war/peace/truce/alliance, casus belli timers, war exhaustion) with hash-chained snapshots; campaign snapshot v8 persists diplomatic graph alongside tactical/strategic data.
- Economy core: `ProvinceEconomyCapsule` (infra build queues, hash-chained snapshot) updates provinces and emits infra-repair strategic events; campaign snapshot v9 includes economy orders. Command delay buffer now snapshots pending delayed orders (snapshot v10).
- `BattleAi` path still bounded with generation counters; aura test covers morale propagation.
- Siege pass 1: `SiegeCapsule` + per-face sections track integrity/breach progress, sapper attrition, breach/seal against `StructureCapsule`; engineering sap/repair hooks; snapshot v11 persists siege sections; replay tag 0xC820 publishes integrity/breach/repair overlays; artillery calls now feed siege capsule.
- `atomic_capsule` encoder now gated behind `std`+`encoder` features to avoid duplicate symbol errors; terrain/LOS adapters derive Debug for diagnostics.
- Tests: `cargo test -p kindly-engine --lib` passing after strategic-event + command-delay telemetry changes.

### Next Grand Strategy Steps (Chaos/UCE34 aligned)
- SiegeCapsule + Engineering hooks: fort integrity, breach progress, sapper attrition; strategic events (breach opened/closed), artillery overlays, fixed-point breach damage.
- LogisticsCapsule & supply lines: explicit routes, attrition/throughput over distance/weather; feed command delay penalties when cut; emit supply disruption events into StratOps.
- OpsC2 guardrails: order-rate throttles, courier reliability shaping, command-net congestion; congestion buckets/fair scheduling/p95 latency logging landed—next add congestion penalties and fairness scheduling.
- Fog/AI integration: threat/stance maps from FogOfWar into BattleAi; AI intent replay/overlay (0xC901) landed—doctrine scoring + NDJSON dashboards landed; next drive doctrine scoring + UI dashboards from intent/threat maps.
- Economy/Espionage expansion: raiding/scorched earth/sabotage events tied to `ProvinceEconomyCapsule`; resistance growth and misinformation ticks that perturb command delays/threat maps.
- Doctrine profiles: commander doctrine presets persisted in snapshots and summarized in StratOps lane.
- Streaming StratOps summaries: periodic JSON/NDJSON during long runs (not just end-of-run) for C2/strategic monitoring.

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
