# Primitives - Computational Capsule Foundation

Core primitives for lockfree, high-performance systems using computational capsule architecture.

## COCA (Computational Capsule) - Quick Reference

**Mandatory Reading**:
1. `/home/samuel/Docs/The Computational Capsule.md` - Foundation patterns
2. `/home/samuel/Primitives/Docs/KEY_INNOVATIONS.md` - Proven 2-19× speedups
3. `UCE34_FRAMEWORK.md` - Tier selection (Q1-Q34)
4. `UCE34_TIER_REFERENCE.md` + `UCE34_EXAMPLES.md` - Implementation guide

**Core Mandate**: 100% lockfree (NO mutex/RwLock), cache-aligned (64B/128B/256B), generation counters (TOCTOU prevention).

## Foundation Crate: atomic_capsule

**Source**: `/home/samuel/Primitives/atomic_capsule/`

**Features**:
- **Tiered Alignment**: HotTier (64B), WarmTier (128B), ColdTier (256B)
- **Verification**: `#[derive(ComputationalCapsule)]` (0ns runtime, <20ms compile)
- **Zero Deps**: Core is no_std, optional features minimal (siphasher, crc32fast, memmap2, tokio)
- **Replaced**: DashMap, RwLock, Mutex, tokio::broadcast, hdrhistogram (all → lockfree capsules)

**Requirements**: 100% lockfree, atomic primitives only, UCE34/ASSUM/B32/T28/I20 compliance.

## XML Documentation Best Practices

<?xml version="1.0" encoding="UTF-8"?>
<xml-best-practices version="1.0">
  <mandate priority="ABSOLUTE">All structured documentation MUST use XML format for optimal LLM parsing</mandate>

  <benefits>
    <benefit>10-30× faster queries (XPath 5s vs Markdown parsing 30-60s)</benefit>
    <benefit>100% accuracy via schema validation</benefit>
    <benefit>Unambiguous structure (no format ambiguity)</benefit>
    <benefit>Machine-readable (standard parsers: xmllint, XPath, lxml)</benefit>
    <benefit>Cross-reference integrity (xs:ID/xs:IDREF validation)</benefit>
  </benefits>

  <token-budgeting>
    <limit type="safety" tokens="20000" desc="Optimal LLM comprehension"/>
    <limit type="absolute" tokens="30000" desc="Parser limit"/>
    <estimation>bytes ÷ 4 ≈ tokens</estimation>
    <action>Split files exceeding 20K tokens into logical parts</action>
  </token-budgeting>

  <when-to-use>
    <use-xml priority="MANDATORY">
      <case>Architecture specifications (system design, capsule layouts)</case>
      <case>Deployment procedures (multi-step processes, configurations)</case>
      <case>API references (commands, parameters, responses)</case>
      <case>Test reports (framework compliance, metrics)</case>
      <case>Session summaries (achievements, implementations, frameworks)</case>
    </use-xml>
    <use-markdown priority="ACCEPTABLE">
      <case>README files (project overview, quick start)</case>
      <case>Philosophy documents (high-level concepts)</case>
      <case>Changelog/release notes (chronological updates)</case>
    </use-markdown>
  </when-to-use>

  <structure-guidelines>
    <rule>Use attributes for metadata (tier, size, count, status)</rule>
    <rule>Use elements for content (descriptions, code, data)</rule>
    <rule>Use CDATA for code blocks (preserves formatting, no escaping)</rule>
    <rule>Entity-escape HTML symbols (&amp;lt; &amp;gt; &amp;amp;)</rule>
    <rule>Hierarchical structure (clear parent-child relationships)</rule>
    <rule>Self-documenting element names (descriptive, no abbreviations)</rule>
  </structure-guidelines>

  <cdata-usage>
    <when>Code blocks (Rust, Bash, any programming language)</when>
    <when>XML-like content (examples, snippets)</when>
    <when>Preserve exact formatting (whitespace, newlines)</when>
    <syntax><![CDATA[<code><![CDATA[
fn main() {
    println!("Hello, world!");  // No escaping needed
}
]]]]><![CDATA[></code>]]></syntax>
    <note>Double CDATA closing (]]]]+&lt;![CDATA[&gt;) for nested CDATA in examples</note>
  </cdata-usage>

  <validation-workflow>
    <step id="1">Create XML with proper namespaces</step>
    <step id="2">Create XSD schema (optional but recommended)</step>
    <step id="3">Validate: xmllint --noout --schema schema.xsd file.xml</step>
    <step id="4">Check tokens: wc -c file.xml, divide by 4, ensure &lt;20K</step>
    <step id="5">Split if needed (logical parts: part1, part2, etc)</step>
    <step id="6">Test XPath queries (verify structure queryable)</step>
  </validation-workflow>

  <xpath-examples>
    <query desc="Get all CLI commands">xmllint --xpath '//cmd/@name' file.xml</query>
    <query desc="Get service dependencies">xmllint --xpath '//service/@deps' file.xml</query>
    <query desc="Get performance metrics">xmllint --xpath '//performance/*/@target' file.xml</query>
    <query desc="Extract CDATA code">xmllint --xpath '//usage/text()' file.xml</query>
  </xpath-examples>

  <lean-design-principles>
    <principle>Minimize redundancy (XPath cross-references, not duplication)</principle>
    <principle>Concise element names (clear but short: cmd not command-definition)</principle>
    <principle>Attribute-first (use attributes for simple data, elements for complex)</principle>
    <principle>Flat when possible (avoid deep nesting beyond 4-5 levels)</principle>
    <principle>Self-contained sections (each major section independently useful)</principle>
  </lean-design-principles>

  <example-template><![CDATA[
<?xml version="1.0" encoding="UTF-8"?>
<root-element version="1.0" date="2025-11-22">
  <metadata>
    <description>Clear description for LLM context</description>
    <token-count estimate="5000" limit="20000"/>
  </metadata>

  <section id="overview">
    <item name="example" tier="T1" size="64B">
      <description>Item description here</description>
      <code language="rust"><![CDATA[
fn example() {
    // Code with <special> characters
}
      ]]]]><![CDATA[></code>
    </item>
  </section>

  <commands>
    <cmd name="deploy" desc="Deploy services">
      <usage><![CDATA[
capsule-cli deploy-stack --remote kindly-hub:9000 --config production.yaml
      ]]]]><![CDATA[></usage>
    </cmd>
  </commands>
</root-element>
  ]]></example-template>

  <quick-reference-table>
    <row>
      <aspect>File size</aspect>
      <guideline>&lt;80KB (~20K tokens)</guideline>
      <action>Split if larger</action>
    </row>
    <row>
      <aspect>Nesting depth</aspect>
      <guideline>&lt;5 levels</guideline>
      <action>Flatten hierarchy</action>
    </row>
    <row>
      <aspect>Code blocks</aspect>
      <guideline>Always CDATA</guideline>
      <action>Use &lt;![CDATA[...]]&gt;</action>
    </row>
    <row>
      <aspect>Validation</aspect>
      <guideline>xmllint --noout</guideline>
      <action>Fix syntax errors</action>
    </row>
    <row>
      <aspect>Attributes</aspect>
      <guideline>Metadata only</guideline>
      <action>tier, size, count, status</action>
    </row>
    <row>
      <aspect>Elements</aspect>
      <guideline>Content/structure</guideline>
      <action>desc, code, usage, steps</action>
    </row>
  </quick-reference-table>

  <anti-patterns>
    <anti-pattern>Deep nesting (>5 levels) → Flatten structure</anti-pattern>
    <anti-pattern>Large monolithic files (>30K tokens) → Split into parts</anti-pattern>
    <anti-pattern>Unescaped HTML in text → Use &amp;lt; &amp;gt; or CDATA</anti-pattern>
    <anti-pattern>Code without CDATA → Always wrap in CDATA</anti-pattern>
    <anti-pattern>Duplicated data → Use XPath cross-references</anti-pattern>
  </anti-patterns>

  <tools>
    <tool name="xmllint">Validation, formatting, XPath queries (libxml2)</tool>
    <tool name="xmlstarlet">Advanced XML manipulation (edit, transform)</tool>
    <tool name="xsltproc">XSLT transformations (schema conversion)</tool>
    <tool name="python-lxml">Programmatic XML processing</tool>
  </tools>

  <principle>Structure documentation in XML from day one. Schema validation prevents errors. XPath enables automation. Token limits ensure optimal LLM parsing. Cross-references guarantee integrity. Lean and short. No ambiguity. Standard tools. Future-proof.</principle>
</xml-best-practices>

## Recent Sessions

| Date | Achievement | Capsules Added | Tests | Status | Details |
|------|-------------|-----------------|-------|--------|---------|
| 2025-11-23 | HTTP/3 + QUIC Stack | +22 QUIC, +2 HTTP/3 | 56/56 | ✅ Production | [Full Summary](legacy/sessions/SESSION_2025-11-23.md) |
| 2025-11-22 | Container Deployment + SystemD | +SystemdServiceCapsule (T1) | 28/28 | ✅ Production | [Full Summary](legacy/sessions/SESSION_2025-11-22.md) |
| 2025-11-14 | Debugger + MCP + Async Runtime | +43 capsules, 2 new projects | 175+ | ✅ Production | [Full Summary](legacy/sessions/SESSION_2025-11-14.md) |

**See**: `legacy/sessions/` for comprehensive session archive with full details, performance metrics, and framework compliance reports.

## Capsule Tiers

**Canonical Reference**: See `/home/samuel/CLAUDE.md` § Capsule Tiers for complete 12-tier taxonomy (T0-T11).

**Quick Reference**: T0 (Auditable, 0ns verify) → T1 (Atomic, 3-10×) → T2 (SIMD, 2-19×) → T3 (Fixed-Point, 2-10×) → T4 (Batch, 10-100×) → T5 (Streaming, O(1)) → T6 (Mixed, 50-100×) → T7 (Heterogeneous, 100-1000×) → T8 (Network, 10-50×) → T9 (Persistent, ACID) → T10 (Probabilistic, 100-1000×) → T11 (QuantumHybrid, 10-16,667×)

## Metacapsule Architecture Pattern

<?xml version="1.0" encoding="UTF-8"?>
<metacapsule-architecture version="1.0" date="2025-11-23">

<definition>
  <concept>Single orchestrating capsule containing multiple specialized sub-capsules</concept>
  <purpose>Hierarchical state coordination for multi-stage pipelines (encoders, transports, etc)</purpose>
  <key-difference>
    <vs type="Component Capsule">Single-purpose primitive (DCT, Quantization); flat no hierarchy</vs>
    <vs type="Container Capsule">Large collections (≥100K objects); array-based management</vs>
    <vs type="Metacapsule">Multi-stage orchestration; single atomic snapshot</vs>
  </key-difference>
  <core-principle>Impossible states prevented via lockfree coordination of embedded sub-capsules</core-principle>
</definition>

<architecture>
  <pattern>Single 256B-1024B orchestrating capsule with 4-18 embedded sub-capsules</pattern>
  <coordination-mechanism>DualAtomicU64 (primary + secondary) with phase bitmasks for hierarchical FSM</coordination-mechanism>
  <memory-layout>Cache-aligned contiguous block (64B/256B/512B/1024B alignment)</memory-layout>
  <state-model>Hierarchical (top-level FSM coordinate + per-sub-capsule state)</state-model>
  <lockfree-guarantee>100% atomic operations; zero mutex/RwLock; O(&lt;50ns) state transitions</lockfree-guarantee>

  <reference-example name="Av1EncoderMetacapsule" tier="T6" size="1024B">
    <description>Video codec orchestrator with 18 sub-capsules</description>
    <coordination>
      <primary>State(8) | Phase(8) | FrameCount(16) | Generation(32)</primary>
      <secondary>TileID(16) | QIndex(8) | LoopFilterLevel(8) | Gen(32)</secondary>
    </coordination>
    <sub-capsules count="18">
      <capsule tier="T1" name="EncoderStateCapsule" size="64B">Tracks encode state (Idle→Transform→Quantize→Entropy→Done)</capsule>
      <capsule tier="T2" name="DctTransformCapsule" size="256B">SIMD 8×8 DCT (AVX2, 19× baseline)</capsule>
      <capsule tier="T3" name="QuantizationCapsule" size="128B">Fixed-point Q index + deadzone (Q8.8)</capsule>
      <capsule tier="T5" name="EntropyCapsule" size="96B">Huffman/arithmetic bitstream (streaming)</capsule>
      <capsule tier="T1" name="TileCapsule" size="64B">Tile state + dependency tracking</capsule>
      <capsule tier="T5" name="FrameBufferCapsule" size="512B">Input/output frame management</capsule>
    </sub-capsules>
    <speedup>2-20× vs traditional encoder (compound T2+T3+T5 effects)</speedup>
  </reference-example>
</architecture>

<when-to-use>
  <use-case priority="MANDATORY">Multi-stage pipeline (3+ independent stages) with deterministic ordering</use-case>
  <use-case priority="MANDATORY">Atomic snapshot required (monitoring, checkpointing, migration)</use-case>
  <use-case priority="MANDATORY">State machine complexity (8+ states, transitions impossible to violate)</use-case>
  <use-case priority="MANDATORY">Real-time constraints (&lt;100ms latency SLA, no GC pauses)</use-case>
  <use-case priority="HIGH">Tier composition needed (T6 Mixed orchestrating T1-T5 sub-capsules)</use-case>

  <anti-pattern>Simple sequential processing → Use T5 Streaming Pipeline instead</anti-pattern>
  <anti-pattern>Embedded systems &lt;512B total memory → Use Component Capsules (flat)</anti-pattern>
  <anti-pattern>Loose coupling between stages → Use message-passing, not metacapsule</anti-pattern>
</when-to-use>

<pattern-comparison>
  <metacapsule>
    <use>Multi-stage orchestration: codecs, transports, state machines</use>
    <size>256B-1024B</size>
    <alignment>64B/256B/512B/1024B cache-aligned</alignment>
    <coordination>DualAtomicU64 with phase bitmasks</coordination>
    <speedup>2-20× (compound tier effects)</speedup>
    <snapshot-latency>&lt;50ns atomic read</snapshot-latency>
    <examples>AV1/PNG/JPEG encoders (11/12 = 84.6%), QuicEndpointMetacapsule, UniversalApiMetaCapsule</examples>
  </metacapsule>

  <component-capsule>
    <use>Single-purpose primitive: DCT, quantization, hash, filter</use>
    <size>64B-256B</size>
    <alignment>64B cache-aligned</alignment>
    <coordination>AtomicU64 (single field)</coordination>
    <speedup>2-19× (single tier)</speedup>
    <snapshot-latency>&lt;10ns atomic read</snapshot-latency>
    <examples>DctCapsule, QuantizationCapsule, CircuitBreaker, HistogramCapsule</examples>
  </component-capsule>

  <container-capsule>
    <use>Large collection management: ≥100K objects</use>
    <size>Variable (array + header)</size>
    <alignment>64B header + element stride</alignment>
    <coordination>Batch CAS loops</coordination>
    <speedup>10-100× (T4 Batch parallelism)</speedup>
    <snapshot-latency>&lt;1μs (O(n) iteration)</snapshot-latency>
    <examples>LockfreeHashTable, ConcurrentMapCapsule, RingBufferBroadcast</examples>
  </container-capsule>
</pattern-comparison>

<metacapsule-advantages>
  <advantage id="1">Lockfree snapshot: Single atomic read captures entire orchestrator state (~50ns)</advantage>
  <advantage id="2">COCA compliance: 100% atomic, zero mutex/RwLock, nested sub-capsules allowed</advantage>
  <advantage id="3">Deterministic latency: &lt;100ns state transitions via Acquire/Release ordering (SWeMR)</advantage>
  <advantage id="4">Cache efficiency: 256B-1024B aligned prevents false sharing; sub-capsules co-located</advantage>
  <advantage id="5">Type safety: Impossible states prevented at compile time (#[derive(ComputationalCapsule)])</advantage>
  <advantage id="6">Hierarchical testing: T28 validates full FSM; sub-capsules tested independently</advantage>
</metacapsule-advantages>

<metacapsule-examples>
  <example project="atomic_capsule" name="Av1EncoderMetacapsule" tier="T6" size="1024B">
    <sub-capsules>18 (EncoderState, FrameBuffer, DCT, Quantization, Entropy, Tile, etc)</sub-capsules>
    <coordination>DualAtomicU64: State(8)|Phase(8)|Count(16)|Gen(32) + Tile(16)|QIndex(8)|Loop(8)|Gen(32)</coordination>
    <speedup>2-20× vs traditional video codec</speedup>
    <status>Production (RFC 8130 AV1)</status>
  </example>

  <example project="atomic_capsule" name="QuicEndpointMetacapsule" tier="T6" size="512B">
    <sub-capsules>22 (QuicConnection, Stream, Packet, Crypto, Flow, ACK, Congestion, etc)</sub-capsules>
    <coordination>DualAtomicU64: ConnectionID(32)|State(8)|Gen(24) + StreamID(32)|Gen(32)</coordination>
    <speedup>1.76× vs TLS 1.3 sync (RFC 9000 QUIC)</speedup>
    <status>Production (RFC 9000/9114)</status>
  </example>

  <example project="atomic_capsule" name="UniversalApiMetaCapsule" tier="T6" size="512B">
    <sub-capsules>6 implicit (REST, GraphQL, gRPC, WebSocket, JSON-RPC, SSE)</sub-capsules>
    <coordination>TransportType enum (4 variants) + ALPN detection (&lt;12ns)</coordination>
    <speedup>1.2× vs manual protocol switching</speedup>
    <status>Production (multi-protocol router)</status>
  </example>

  <example project="kindly-verified" name="PNGEncoderCapsule" tier="T6" size="512B">
    <sub-capsules>3 (SIMDFilterApplicator, DEFLATEEncoder, PNGChunkWriter)</sub-capsules>
    <coordination>DualAtomicU64 simple state machine</coordination>
    <speedup>2-5× vs libpng</speedup>
    <status>Production (RFC 2083 PNG)</status>
  </example>
</metacapsule-examples>

<best-practices>
  <practice id="1">Keep orchestrator ≤1024B: Larger → split into component capsules</practice>
  <practice id="2">Sub-capsule dependency graph acyclic: Impossible deadlock (encode→compress→output)</practice>
  <practice id="3">Atomic snapshot before state transition: Prevents partial state exposure</practice>
  <practice id="4">Phase bitmasks for FSM: Each sub-capsule owns bits; no field conflicts</practice>
  <practice id="5">Test sub-capsules independently PLUS integration: T28 4-tier validation</practice>
  <practice id="6">Generation counters on DualAtomicU64: TOCTOU detection between snapshot + write</practice>
</best-practices>

</metacapsule-architecture>

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

**All Phases**: UCE34 (Q1-Q34 systematic discovery), COCA (100% lockfree, no mutex/RwLock), ASSUM (99.5%+ safety, all assumptions verified), B32 (95% CI, 1000+ iterations, fair baselines), T28 (4-tier testing: unit/property/integration/production), I20 (integration validation, 20/20 questions), Q34 (hash-chained audit trails for SOX/SOC2/GDPR/HIPAA).

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

**Core Frameworks**: See `/home/samuel/CLAUDE.md` § Mandatory Reading Framework for canonical UCE34, COCA, ASSUM, B32, T28, I20, Q34 documentation.

**Phase Reports**: See `atomic_capsule/CLAUDE.md` for comprehensive session summaries (Nov 14/22/23), phase status, verification reports, and COCA compliance validation.

**Pattern Guides**: `docs/ATOMIC_CAPSULE_PATTERNS.md`, `docs/ATOMIC_CAPSULE_COMPOSITION.md`, `docs/ATOMIC_CAPSULE_FAILURE_MODES.md`

## Trade Secret Notice

Some components protected as trade secrets:
- `atomic_hedge_capsule` - Full trade secret protection
- `kindly_hft` - Strategic algorithms protected

**MANDATORY**: Never commit trade secret components to public repositories. All commits must use `[TRADE SECRET]` tag.

## Infrastructure

**Training Server (6900HX)**: AMD Ryzen 9 6900HX, 64GB DDR5, Ubuntu Server 24.04, 192.168.0.38 (WiFi: TP-Link_E1C8)

**Access**: `ssh samuel@192.168.0.38`

**Sync**: lsyncd auto-sync (2-second delay) from local to remote

## Performance Standards

**B32 Framework**: 95% CI, 1000+ iterations, fair baselines (not strawman), reproducibility validation

**Reality Check**: 10-50% typical, 2-10× exceptional, 100×+ extensive validation

**ASSUM Framework**: Every `#ASSUME` needs `#VERIFY`, 99.5%+ safety target

## References

**Foundation**: `/home/samuel/Docs/The Computational Capsule.md`, `/home/samuel/Primitives/Docs/KEY_INNOVATIONS.md`

**Universal Config**: `/home/samuel/CLAUDE.md` - UCE34 framework v6.0 (XML canonical source)

**Project Configs**: `kindly_hft/CLAUDE.md`, `kiang/CLAUDE.md`, `clapi_core/CLAUDE.md`, `kindly_dash/CLAUDE.md`
