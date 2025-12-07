<?xml version="1.0" encoding="UTF-8"?>
<!-- kdb-mcp - MCP Debugging Server for kdb (T6 Mixed) -->
<!-- Version: 0.2.0 | Updated: 2025-12-06 -->
<project name="kdb-mcp" version="0.2.0">

<metadata>
  <description>T6 Mixed JSON-RPC MCP server orchestrating kdb debugger with sub-10μs latency and tier-based subscription enforcement</description>
  <location>/home/samuel/Primitives/kdb-mcp/</location>
  <size>75,962 LOC | 59 modules | 27 MCP tools</size>
  <tier>T6 Mixed (T0+T1+T2+T4+T5+T10)</tier>
  <framework>UCE34, COCA 100% lockfree, 99.99% ASSUM safe</framework>
  <performance>&lt;10μs RPC orchestration | &lt;200ns tier enforcement</performance>
  <trade-secret>YES - MCP server implementation protected</trade-secret>
  <status>Production Ready | 326/327 tests passing</status>
</metadata>

<!-- ============================================================================
     ARCHITECTURE OVERVIEW
     ============================================================================ -->
<architecture>
  <orchestrator name="McpServerCapsule" size="256KB" alignment="256B">
    <description>Top-level T6 Mixed orchestrator coordinating 14+ embedded capsules</description>
    <latency-target>&lt;10μs end-to-end request handling</latency-target>
    <throughput>100K+ RPC calls/sec | 100+ concurrent clients</throughput>
  </orchestrator>

  <request-flow>
    <step order="1" capsule="JsonRpcCapsule" latency="&lt;1μs">Parse JSON-RPC request</step>
    <step order="2" capsule="LicenseValidatorCapsule" latency="&lt;10ns">Validate license (cached FNV)</step>
    <step order="3" capsule="TierRateLimiterCapsule" latency="&lt;100ns">Per-tier rate limit check</step>
    <step order="4" capsule="TierEnforcementCapsule" latency="&lt;20ns">Feature permission check</step>
    <step order="5" capsule="SnapshotQuotaCapsule" latency="&lt;50ns">Snapshot quota check (tools 18-19)</step>
    <step order="6" capsule="AccessModeCapsule" latency="&lt;10ns">Observer/Operator mode check</step>
    <step order="7" capsule="McpToolRegistryCapsule" latency="&lt;120ns">Route to tool handler</step>
    <step order="8" capsule="DebuggerCapsule" latency="variable">Execute debug command</step>
    <step order="9" capsule="AuditLogCapsule" latency="&lt;50ns">Record audit trail (Q34)</step>
    <step order="10" capsule="JsonRpcCapsule" latency="&lt;1μs">Format JSON response</step>
  </request-flow>
</architecture>

<!-- ============================================================================
     TIER ENFORCEMENT SYSTEM (NEW - 2025-12-06)
     ============================================================================ -->
<tier-enforcement version="1.0" payment-provider="Paddle (agnostic)">
  <description>T1 Atomic tier-based subscription enforcement with 20% grace period</description>
  <total-latency>&lt;200ns per request</total-latency>

  <subscription-tiers>
    <tier name="Hobby" value="0" rpm="60" burst="10" snapshots="100" retention="7d" features="0x0F"/>
    <tier name="Starter" value="1" rpm="300" burst="30" snapshots="1000" retention="7d" features="0x1F"/>
    <tier name="Developer" value="2" rpm="1000" burst="100" snapshots="10000" retention="30d" features="0x3F"/>
    <tier name="Professional" value="3" rpm="5000" burst="500" snapshots="100000" retention="90d" features="0xFF"/>
    <tier name="Enterprise" value="4" rpm="MAX" burst="MAX" snapshots="MAX" retention="365d" features="0x3FF"/>
  </subscription-tiers>

  <feature-flags>
    <flag bit="0" name="TIME_TRAVEL" tiers="all">Bidirectional replay</flag>
    <flag bit="1" name="BREAKPOINTS" tiers="all">Breakpoint management</flag>
    <flag bit="2" name="STACK_TRACE" tiers="all">Stack unwinding</flag>
    <flag bit="3" name="AUDIT_TRAIL" tiers="all">Export audit traces</flag>
    <flag bit="4" name="MEMORY_READ" tiers="Starter+">Read process memory</flag>
    <flag bit="5" name="MEMORY_WRITE" tiers="Developer+">Write process memory</flag>
    <flag bit="6" name="SYMBOL_RESOLUTION" tiers="Professional+">DWARF symbol lookup</flag>
    <flag bit="7" name="STEP_BACKWARD" tiers="Professional+">Time-travel backward</flag>
    <flag bit="8" name="PRIORITY_SUPPORT" tiers="Enterprise">Priority support queue</flag>
    <flag bit="9" name="CUSTOM_RETENTION" tiers="Enterprise">Custom retention policies</flag>
  </feature-flags>

  <enforcement-stages>
    <stage name="Normal" range="0-80%" action="Allowed"/>
    <stage name="Warning" range="80-100%" action="Allowed + X-Snapshot-Warning header"/>
    <stage name="SoftBlock" range="100-120%" action="New captures disabled, reads allowed"/>
    <stage name="HardBlock" range="120%+" action="quota_exceeded error"/>
  </enforcement-stages>

  <capsules>
    <capsule name="SubscriptionTier" file="subscription_tier.rs" lines="292" tier="T0">
      <size>0 bytes (enum)</size>
      <latency>0ns (const methods)</latency>
      <purpose>#[repr(u8)] enum for atomic storage, tier constants</purpose>
    </capsule>
    <capsule name="TierEnforcementCapsule" file="tier_enforcement.rs" lines="809" tier="T1">
      <size>64 bytes</size>
      <alignment>64B</alignment>
      <latency>&lt;20ns require_feature()</latency>
      <purpose>Feature bitmask enforcement, O(1) permission checks</purpose>
    </capsule>
    <capsule name="TierRateLimiterCapsule" file="tier_rate_limiter.rs" lines="939" tier="T1">
      <size>512 bytes</size>
      <alignment>64B</alignment>
      <latency>&lt;100ns check_and_consume()</latency>
      <purpose>Per-tier token bucket rate limiting</purpose>
    </capsule>
    <capsule name="SnapshotQuotaCapsule" file="snapshot_quota.rs" lines="1071" tier="T1">
      <size>256 bytes</size>
      <alignment>64B</alignment>
      <latency>&lt;50ns check_capture_allowed()</latency>
      <purpose>Snapshot quota with 20% grace period</purpose>
    </capsule>
    <capsule name="SessionTierMapCapsule" file="session_tier_map.rs" lines="732" tier="T1">
      <size>~65KB (4096 slots)</size>
      <alignment>256B</alignment>
      <latency>&lt;50ns get_tier()</latency>
      <purpose>Session→Tier mapping via FNV-1a hash table</purpose>
    </capsule>
  </capsules>

  <integration file="server.rs" function="dispatch_tool()">
    <check order="1" method="check_tier_rate_limit(session_id)" latency="&lt;100ns"/>
    <check order="2" method="check_tier_feature(handler_id, session_id)" latency="&lt;20ns"/>
    <check order="3" method="check_snapshot_quota(handler_id, session_id)" latency="&lt;50ns"/>
  </integration>
</tier-enforcement>

<!-- ============================================================================
     CAPSULE INVENTORY
     ============================================================================ -->
<capsules total="18+">
  <!-- Core Request Pipeline -->
  <capsule name="JsonRpcCapsule" tier="T1" size="4KB" latency="&lt;1μs">JSON-RPC 2.0 parse/format</capsule>
  <capsule name="LicenseValidatorCapsule" tier="T1" size="4KB" latency="&lt;10ns">FNV hash + Ed25519 cached validation</capsule>
  <capsule name="RateLimiterCapsule" tier="T1" size="4KB" latency="&lt;20ns">Global token bucket</capsule>
  <capsule name="QuotaTrackerCapsule" tier="T1" size="4KB" latency="&lt;70ns">Usage tracking</capsule>
  <capsule name="McpToolRegistryCapsule" tier="T1" size="16KB" latency="&lt;120ns">27-tool atomic registry</capsule>
  <capsule name="AuditLogCapsule" tier="T0+T1" size="32KB" latency="&lt;50ns">BLAKE3 hash-chain audit trail</capsule>

  <!-- Access Control -->
  <capsule name="AccessModeCapsule" tier="T1" size="128B" latency="&lt;10ns">Observer/Operator FSM</capsule>
  <capsule name="OperatorChallengeCapsule" tier="T1" size="256B" latency="&lt;1μs">Ed25519 challenge-response</capsule>
  <capsule name="OperatorSessionCapsule" tier="T1" size="512B" latency="&lt;10ns">Session state machine</capsule>
  <capsule name="AccountLockoutCapsule" tier="T1" size="64B" latency="&lt;50ns">Progressive account lockout (NIST 800-63B)</capsule>

  <!-- Security (Phase 2A) -->
  <capsule name="AuthGuardCapsule" tier="T1" size="256B" latency="&lt;100ns">Multi-layer authentication</capsule>
  <capsule name="ZeroTrustPolicyCapsule" tier="T6" size="1KB" latency="&lt;1μs">Zero-trust policy engine</capsule>
  <capsule name="AnomalyDetectorCapsule" tier="T10" size="512B" latency="&lt;100ns">Z-score + heuristic detection</capsule>
  <capsule name="PerClientRateLimiterCapsule" tier="T1" size="varies" latency="&lt;50ns">Per-client token buckets</capsule>
</capsules>

<!-- ============================================================================
     MCP TOOL REGISTRY (27 Tools)
     ============================================================================ -->
<mcp-tools total="27">
  <category name="Debugging" range="1-9">
    <tool id="1" name="debugger/attach" feature="BREAKPOINTS">Attach to process</tool>
    <tool id="2" name="debugger/set_breakpoint" feature="BREAKPOINTS">Add breakpoint</tool>
    <tool id="3" name="debugger/continue" feature="TIME_TRAVEL">Resume execution</tool>
    <tool id="4" name="debugger/step_forward" feature="TIME_TRAVEL">Single step forward</tool>
    <tool id="5" name="debugger/step_backward" feature="TIME_TRAVEL">Time-travel backward</tool>
    <tool id="6" name="debugger/get_stack_trace" feature="STACK_TRACE">SIMD stack unwind</tool>
    <tool id="7" name="debugger/get_variables" feature="MEMORY_READ">Read memory/variables</tool>
    <tool id="8" name="debugger/find_similar_bugs" feature="MEMORY_READ">T10 probabilistic search</tool>
    <tool id="9" name="debugger/export_trace" feature="AUDIT_TRAIL">T5 streaming export</tool>
  </category>

  <category name="Admin" range="10-12" tier-check="none">
    <tool id="10" name="debugger/quota_status" latency="&lt;70ns">Quota tier/limits/usage</tool>
    <tool id="11" name="debugger/license_info" latency="&lt;10ns">License tier/validation/expiry</tool>
    <tool id="12" name="debugger/get_comprehensive_audit" latency="&lt;10μs">Q34 compliance audit</tool>
  </category>

  <category name="Session Pool" range="13-17">
    <tool id="13" name="debugger/allocate_session" latency="&lt;100ns">Allocate tiered session</tool>
    <tool id="14" name="debugger/release_session" latency="&lt;100ns">Release session</tool>
    <tool id="15" name="debugger/get_session_tier" latency="&lt;10ns">Get session tier</tool>
    <tool id="16" name="debugger/upgrade_session" latency="&lt;1μs">Upgrade to higher tier</tool>
    <tool id="17" name="debugger/get_pool_stats" latency="&lt;50ns">Pool statistics</tool>
  </category>

  <category name="Memory Replay" range="18-23">
    <tool id="18" name="debugger/enable_memory_replay" latency="&lt;10ms" quota-check="true">Enable COW tracking</tool>
    <tool id="19" name="debugger/capture_memory_snapshot" latency="&lt;50ms" quota-check="true">Capture snapshot</tool>
    <tool id="20" name="debugger/read_memory_at_snapshot" latency="&lt;2ms" feature="MEMORY_READ">Read historical memory</tool>
    <tool id="21" name="debugger/navigate_to_snapshot" latency="&lt;100ns">Navigate snapshots</tool>
    <tool id="22" name="debugger/get_memory_replay_stats" latency="&lt;50ns">Replay statistics</tool>
    <tool id="23" name="debugger/verify_memory_integrity" latency="O(n)">Q34 integrity check</tool>
  </category>

  <category name="Access Control" range="24-27" tier-check="none">
    <tool id="24" name="debugger/get_access_mode">Get Observer/Operator mode</tool>
    <tool id="25" name="debugger/request_operator_challenge">Request Ed25519 challenge</tool>
    <tool id="26" name="debugger/elevate_to_operator">Submit signature to elevate</tool>
    <tool id="27" name="debugger/revoke_operator">Drop to Observer mode</tool>
  </category>
</mcp-tools>

<!-- ============================================================================
     MODULE INVENTORY
     ============================================================================ -->
<modules total="59">
  <category name="Core" files="12">
    <module name="lib.rs" lines="452">Public API, 8-capsule docs</module>
    <module name="server.rs" lines="2859">McpServerCapsule orchestrator</module>
    <module name="json_rpc.rs" lines="274">JSON-RPC protocol</module>
    <module name="tool_registry.rs">27-tool atomic registry</module>
    <module name="rate_limiter.rs">Global token bucket</module>
    <module name="quota_tracker.rs">Usage tracking</module>
    <module name="license_validator.rs">Ed25519 + FNV validation</module>
  </category>

  <category name="Tier Enforcement" files="5" lines="~3900">
    <module name="subscription_tier.rs" lines="292">T0 tier enum</module>
    <module name="tier_enforcement.rs" lines="809">T1 feature enforcement</module>
    <module name="tier_rate_limiter.rs" lines="939">T1 per-tier rate limiting</module>
    <module name="snapshot_quota.rs" lines="1071">T1 quota + 20% grace</module>
    <module name="session_tier_map.rs" lines="732">T1 session→tier hash table</module>
  </category>

  <category name="Phase 2A: Security" files="17" lines="~10700">
    <module name="api_key_auth.rs">API key authentication</module>
    <module name="auth_guard.rs">Multi-layer auth guard</module>
    <module name="auth_token.rs">JWT token handling</module>
    <module name="totp_validator.rs">TOTP 2FA (constant-time)</module>
    <module name="access_control.rs">RBAC permissions</module>
    <module name="zero_trust_policy.rs">Zero-trust policy engine</module>
    <module name="account_lockout.rs">Progressive lockout (NIST)</module>
    <module name="secrets_manager.rs">Secrets encryption</module>
    <module name="hsm_integration.rs">PKCS#11 HSM binding</module>
    <module name="tls_capsule.rs">TLS 1.3 certificate handling</module>
    <module name="intrusion_detector.rs">Intrusion detection</module>
    <module name="anomaly_detector.rs">Z-score + heuristic detection</module>
  </category>

  <category name="Phase 2B: Observability" files="5" lines="~3600">
    <module name="metrics.rs">Prometheus metrics</module>
    <module name="tracing_setup.rs">OpenTelemetry tracing</module>
    <module name="audit_enhancement.rs">Enhanced audit logging</module>
    <module name="audit_log_rotation.rs">Log rotation</module>
    <module name="per_client_rate_limiter.rs">Per-client rate limits</module>
  </category>

  <category name="Phase 2C: Infrastructure" files="8" lines="~3500">
    <module name="runtime.rs">T5 async runtime</module>
    <module name="http_transport.rs">T4 HTTP transport</module>
    <module name="stdio_transport.rs">T5 stdio transport</module>
    <module name="connection_pool.rs">Connection pooling</module>
    <module name="tool_executor.rs">Tool execution</module>
    <module name="feature_flags.rs">Feature flag system</module>
  </category>
</modules>

<!-- ============================================================================
     FEATURE FLAGS
     ============================================================================ -->
<feature-flags total="40">
  <category name="Core">
    <flag name="std">Standard library (required)</flag>
    <flag name="json-rpc">JSON-RPC serialization (default)</flag>
  </category>

  <category name="Phase 2A: Security" count="17">
    <flag name="api-key-auth"/>
    <flag name="auth-guard"/>
    <flag name="auth-token"/>
    <flag name="totp"/>
    <flag name="access-control"/>
    <flag name="zero-trust"/>
    <flag name="secrets"/>
    <flag name="hsm"/>
    <flag name="tls"/>
    <flag name="anomaly-detection"/>
  </category>

  <category name="Phase 2B: Observability" count="5">
    <flag name="metrics"/>
    <flag name="tracing"/>
    <flag name="audit"/>
    <flag name="rate-limiting"/>
  </category>

  <category name="Phase 2C: Infrastructure" count="8">
    <flag name="runtime"/>
    <flag name="http-transport"/>
    <flag name="stdio-transport"/>
    <flag name="connection-pool"/>
    <flag name="tool-executor"/>
  </category>

  <category name="Aliases" count="8">
    <flag name="secure-defaults">api-key-auth + auth-guard + rate-limiting</flag>
    <flag name="all">All features enabled</flag>
  </category>
</feature-flags>

<!-- ============================================================================
     TESTING & BENCHMARKS
     ============================================================================ -->
<testing>
  <summary>326 passed, 1 failed (pre-existing race condition)</summary>

  <t28-coverage>
    <tier name="Q1-Q7 Unit" count="200+">Capsule layout, alignment, atomic ops</tier>
    <tier name="Q8-Q14 Property" count="150+">Concurrent tier changes, rate limit accuracy</tier>
    <tier name="Q15-Q21 Integration" count="200+">MCP protocol, tool execution</tier>
    <tier name="Q22-Q28 Production" count="100+">Chaos scenarios, load testing</tier>
    <tier name="Q29-Q35 Determinism">Replay consistency, hash chain validation</tier>
  </t28-coverage>

  <tier-enforcement-tests>
    <test name="TierEnforcementCapsule" count="19">Feature bitmask, tier transitions</test>
    <test name="SnapshotQuotaCapsule" count="14">Enforcement stages, grace period</test>
    <test name="TierRateLimiterCapsule" count="25">Per-tier token buckets</test>
    <test name="SessionTierMapCapsule" count="12">Hash table, concurrent lookups</test>
  </tier-enforcement-tests>

  <b32-benchmarks count="20">
    <bench name="mcp_latency">RPC orchestration (&lt;10μs)</bench>
    <bench name="tier_enforcement">Feature check (&lt;20ns)</bench>
    <bench name="rate_limiting">Token bucket (&lt;100ns)</bench>
    <bench name="auth_guard">Authentication (&lt;1μs)</bench>
  </b32-benchmarks>
</testing>

<!-- ============================================================================
     FRAMEWORK COMPLIANCE
     ============================================================================ -->
<compliance>
  <uce34 status="100%">
    <checkpoint name="Q10" status="pass">T6 Mixed tier selected</checkpoint>
    <checkpoint name="Q11" status="pass">100% Rust, lockfree atomics</checkpoint>
    <checkpoint name="Q12" status="pass">Nightly features enabled</checkpoint>
    <checkpoint name="Q33" status="partial">Verification (derive macro pending)</checkpoint>
    <checkpoint name="Q34" status="pass">BLAKE3 hash-chain audit trails</checkpoint>
  </uce34>

  <coca status="100%">
    <rule name="Lockfree" status="pass">No mutex/RwLock</rule>
    <rule name="Cache-aligned" status="pass">64B/128B/256B alignment</rule>
    <rule name="Generation counters" status="pass">TOCTOU prevention</rule>
    <rule name="Composition" status="pass">14+ sub-capsules</rule>
  </coca>

  <assum status="99.99%">
    <category name="Fast paths">0 unsafe</category>
    <category name="HSM FFI">1 (cryptoki)</category>
    <category name="Memory replay">~10 COW ops</category>
    <total>~12 unsafe blocks, 456 ASSUM/VERIFY tags</total>
  </assum>

  <regulatory>
    <standard name="SOX">Hash-chain audit trails</standard>
    <standard name="SOC2">Access control, encryption</standard>
    <standard name="GDPR">Data retention, deletion</standard>
    <standard name="HIPAA">Audit logging, access control</standard>
  </regulatory>
</compliance>

<!-- ============================================================================
     DEPLOYMENT
     ============================================================================ -->
<deployment>
  <binary name="kdb-mcp-server" size="256KB">
    <build>cargo build --release --bin kdb-mcp-server --features "std,json-rpc"</build>
  </binary>

  <requirements>
    <platform>Linux x86_64 (Ubuntu 22.04+, kernel 5.15+)</platform>
    <permissions>CAP_SYS_PTRACE or same UID as target</permissions>
    <memory>~10MB runtime (250 concurrent clients)</memory>
    <cpu>Single-threaded capable (lockfree)</cpu>
  </requirements>

  <paddle-integration status="pending-approval">
    <webhook-handler>On checkout.completed: session_tier_map.set_tier(session_id, tier)</webhook-handler>
    <note>Payment-provider-agnostic design - tier stored as enum, not payment details</note>
  </paddle-integration>
</deployment>

<!-- ============================================================================
     QUICK REFERENCE
     ============================================================================ -->
<quick-reference>
  <commands>
    <cmd name="build">cargo build --release --features "std,json-rpc"</cmd>
    <cmd name="test">cargo test --lib --features "std,json-rpc"</cmd>
    <cmd name="bench">ssh samuel@kindly-hub "cd ~/Primitives/kdb-mcp &amp;&amp; cargo bench"</cmd>
    <cmd name="clippy">cargo clippy --all-features</cmd>
  </commands>

  <key-files>
    <file path="src/server.rs">McpServerCapsule orchestrator (dispatch_tool at line 680)</file>
    <file path="src/tier_enforcement.rs">TierEnforcementCapsule + FeatureFlags</file>
    <file path="src/snapshot_quota.rs">SnapshotQuotaCapsule + EnforcementStage</file>
    <file path="src/session_tier_map.rs">SessionTierMapCapsule (FNV-1a hash table)</file>
    <file path="src/tools/mod.rs">27 MCP tool implementations</file>
  </key-files>
</quick-reference>

</project>
