<?xml version="1.0" encoding="UTF-8"?>
<!-- kdb-mcp - MCP Debugging Server for kdb (T6 Mixed) -->
<!-- Version: 0.3.0 | Updated: 2025-12-10 -->
<!-- COMMERCIAL PRODUCT - NOT OPEN SOURCE -->
<project name="kdb-mcp" version="0.3.0">

<metadata>
  <description>T6 Mixed JSON-RPC MCP server with OAuth 2.1 + Google, Streamable HTTP, and tier-based subscription enforcement</description>
  <role>PRIMARY USER INTERFACE - This is how users interact with KDB</role>
  <delivery-model>MCP (Model Context Protocol) - Platform agnostic access from any OS</delivery-model>
  <location>/home/samuel/Primitives/Kindly-Debugger/kdb-mcp/</location>
  <size>80,000+ LOC | 64 modules | 27 MCP tools | 4 OAuth capsules</size>
  <tier>T6 Mixed (T0+T1+T2+T4+T5+T8+T10)</tier>
  <framework>UCE35, Chaos 100% lockfree, 99.99% ASSUM safe</framework>
  <performance>&lt;10μs RPC orchestration | &lt;50ns OAuth state lookup</performance>
  <commercial-status>PROPRIETARY - NOT OPEN SOURCE</commercial-status>
  <trade-secret>YES - MCP server implementation protected</trade-secret>
  <status>Production Ready | 427+ tests (333 existing + 94 OAuth)</status>
  <signup-url>https://api.kindly.software/api/v1/signup</signup-url>
  <live-endpoints>
    <endpoint>https://mcp.kindly.software/mcp (Streamable HTTP - recommended)</endpoint>
    <endpoint>https://mcp.kindly.software/sse (SSE - legacy)</endpoint>
  </live-endpoints>
  <auth-methods>OAuth 2.1 + Google | API Key (X-License-Key) | Bearer Token</auth-methods>

  <npm-client version="2.0.1" published="2025-12-11" status="RECOMMENDED">
    <package>@kindly-software-inc/kdb</package>
    <why>Claude Code HTTP transport broken - stdio bridge is the ONLY way to access kdb from Claude Code</why>
    <install>npm install @kindly-software-inc/kdb</install>
    <features>Retry, Circuit breaker, Caching (100× faster), Offline mode, P0 Protection, 224 tests</features>
    <size>2.7MB binary | 11,000 LOC | UNLICENSED (Proprietary)</size>
  </npm-client>
</metadata>

<commercial-model>
  <status>Commercial product with tiered licensing</status>

  <trial-promo status="ACTIVE">
    <description>7-day free trial with ALL features (Enterprise-level access: 0x3FF)</description>
    <sessions>Unlimited during trial</sessions>
    <credit-card>Not required</credit-card>
    <after-trial>Falls back to tier-based limits</after-trial>
  </trial-promo>

  <tiers>
    <tier name="Hobby" price="Free" sessions="5/month">
      <features>Time-travel (3 step_backward/day), breakpoints, stack traces, audit trail</features>
      <limitations>No memory replay, no LSH bug search, no read_memory_at_snapshot</limitations>
    </tier>
    <tier name="Pro" price="$19/month" sessions="100/month" note="was Starter">
      <features>Unlimited time-travel, unlimited step_backward, basic memory replay</features>
      <limitations>No LSH bug search, no read_memory_at_snapshot</limitations>
    </tier>
    <tier name="Engineer" price="$49/month" sessions="500/month" note="was Developer">
      <features>Full memory replay, LSH bug search (find_similar_bugs), read_memory_at_snapshot</features>
      <all-debugging-tools>Yes</all-debugging-tools>
    </tier>
    <tier name="Teams" price="$129/month" sessions="2,000/month" note="was Professional">
      <features>Same as Engineer + 5 seats (+$20/seat), team audit logs, memory integrity verification</features>
    </tier>
    <tier name="Enterprise" price="From $999/month" sessions="Unlimited">
      <features>Everything unlimited, SOX/SOC2/GDPR/HIPAA compliance, Q34 cryptographic audit trail</features>
      <retention>Custom (up to 7 years)</retention>
      <support>Priority + dedicated</support>
    </tier>
  </tiers>

  <platform-support>
    <note>Users on ANY OS (macOS, Windows, Linux) connect via MCP clients</note>
    <note>AI assistants (Claude Code, Cursor, etc.) handle the MCP protocol</note>
    <note>Users debug via natural language - never touch ptrace directly</note>
  </platform-support>
</commercial-model>

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
     SSE TRANSPORT SYSTEM (NEW - 2025-12-09)
     ============================================================================ -->
<sse-transport version="1.0" status="PRODUCTION" spec="MCP 2024-11-05">
  <description>T6 Mixed SSE transport implementing MCP Server-Sent Events protocol</description>
  <live-status>✅ LIVE at https://mcp.kindly.software/sse</live-status>

  <protocol-flow>
    <step order="1" method="GET" path="/sse" response="200 + text/event-stream">
      Client establishes SSE connection, receives session ID via endpoint event
    </step>
    <step order="2" event="endpoint" format="data: /message?sessionId={uuid}">
      Server sends endpoint event with unique session ID (UUID format)
    </step>
    <step order="3" method="POST" path="/message?sessionId={uuid}" response="204 No Content">
      Client sends JSON-RPC requests to message endpoint
    </step>
    <step order="4" event="message" format="data: {json-rpc-response}">
      Server pushes JSON-RPC response via SSE stream
    </step>
  </protocol-flow>

  <capsules>
    <capsule name="HttpTransportCapsule" file="http_transport.rs" tier="T6 Mixed" size="512B" alignment="256B">
      <latency>&lt;1μs handle_request()</latency>
      <purpose>Chaos-compliant HTTP request handling with metrics tracking</purpose>
      <key-method name="is_protocol_method()" latency="&lt;100ns">
        Detects MCP protocol methods (initialize, ping, notifications/initialized) that bypass auth per MCP spec
      </key-method>
      <features>
        <feature>Path validation: /mcp, /sse, /message, /health</feature>
        <feature>Header extraction: X-License-Key, Authorization, Content-Type</feature>
        <feature>Content-Type relaxation: allows empty (MCP client compatibility)</feature>
        <feature>Atomic metrics: total_requests, successful_requests, auth_failures, rate_limit_hits</feature>
      </features>
    </capsule>
    <capsule name="SseConnectionPoolCapsule" file="sse_connection_pool.rs" tier="T4+T1" size="~13KB" alignment="256B">
      <slots>100 concurrent SSE connections</slots>
      <latency>&lt;100ns allocate/release</latency>
      <purpose>Lockfree connection slot management with bitmap tracking</purpose>
    </capsule>
    <capsule name="RateLimiterCapsule" file="rate_limiter.rs" tier="T1" size="4KB" alignment="64B">
      <latency>&lt;20ns check_and_consume()</latency>
      <purpose>Global token bucket rate limiting</purpose>
    </capsule>
  </capsules>

  <mcp-spec-compliance>
    <requirement name="initialize without auth" status="✅">
      Protocol methods bypass authentication per MCP 2024-11-05 spec.
      is_protocol_method() detects: "initialize", "ping", "notifications/initialized"
    </requirement>
    <requirement name="204 No Content for POST" status="✅">
      POST /message returns 204, response delivered via SSE push
    </requirement>
    <requirement name="endpoint event first" status="✅">
      Server sends 'event: endpoint' with session ID immediately after SSE handshake
    </requirement>
    <requirement name="heartbeat/keepalive" status="✅">
      30-second SSE heartbeat interval prevents connection timeout
    </requirement>
  </mcp-spec-compliance>

  <binary name="mcp_sse_server" file="src/bin/mcp_sse_server.rs" lines="~800">
    <build>cargo build --release --bin mcp_sse_server --features "std,json-rpc,sse-transport,http-transport"</build>
    <deployment>/home/samuel/mcp_servers/kdb-mcp/bin/mcp_sse_server</deployment>
    <systemd-service>kdb-mcp-sse.service</systemd-service>
    <port>8081</port>
    <memory>~3MB runtime (100 concurrent connections)</memory>
  </binary>

  <tests count="7" file="http_transport.rs:999-1076">
    <test name="test_is_protocol_method_initialize">initialize method detection</test>
    <test name="test_is_protocol_method_ping">ping method detection</test>
    <test name="test_is_protocol_method_notifications_initialized">notification detection</test>
    <test name="test_is_protocol_method_tool_calls_require_auth">tools/list requires auth</test>
    <test name="test_is_protocol_method_resources_require_auth">resources/* requires auth</test>
    <test name="test_is_protocol_method_empty_and_invalid">edge case handling</test>
    <test name="test_is_protocol_method_edge_cases">key ordering variations</test>
  </tests>
</sse-transport>

<!-- ============================================================================
     STREAMABLE HTTP TRANSPORT (NEW - 2025-12-10)
     ============================================================================ -->
<streamable-http-transport version="1.0" status="PRODUCTION" spec="MCP 2025-06-18">
  <description>T6 Mixed Streamable HTTP transport - unified /mcp endpoint replacing SSE</description>
  <live-status>✅ LIVE at https://mcp.kindly.software/mcp</live-status>

  <protocol-summary>
    <endpoint path="/mcp" methods="POST, GET, DELETE">Unified endpoint for MCP protocol</endpoint>
    <session-management>Mcp-Session-Id header (not query param)</session-management>
    <content-negotiation>JSON (immediate) or SSE (streaming) based on Accept header</content-negotiation>
    <protocol-version>Mcp-Protocol-Version: 2025-06-18 header</protocol-version>
  </protocol-summary>

  <protocol-flow>
    <step order="1" method="POST" path="/mcp" body="initialize request">
      Client initializes with protocolVersion, capabilities, clientInfo
    </step>
    <step order="2" response="200 JSON + Mcp-Session-Id header">
      Server responds with session ID, capabilities, serverInfo
    </step>
    <step order="3" method="POST" path="/mcp" headers="Mcp-Session-Id, Authorization">
      Client sends subsequent requests with session binding
    </step>
    <step order="4" response="200 JSON or 202 Accepted">
      Server returns immediate JSON or 202 for notifications
    </step>
    <step order="5" method="GET" path="/mcp" headers="Mcp-Session-Id, Accept: text/event-stream">
      Client opens SSE stream for server-initiated messages (optional)
    </step>
    <step order="6" method="DELETE" path="/mcp" headers="Mcp-Session-Id">
      Client explicitly terminates session (returns 204 No Content)
    </step>
  </protocol-flow>

  <capsule name="StreamableHttpTransportCapsule" file="streamable_http.rs" tier="T6 Mixed" size="768B" alignment="256B">
    <loc>1,564 lines</loc>
    <tests>20 unit tests</tests>
    <latency>&lt;100μs per request</latency>
    <purpose>Unified /mcp endpoint handler with content negotiation</purpose>
    <features>
      <feature>POST /mcp: JSON-RPC messages (requests, notifications, responses)</feature>
      <feature>GET /mcp: SSE stream for server→client push</feature>
      <feature>DELETE /mcp: Explicit session termination</feature>
      <feature>Mcp-Session-Id header for session binding</feature>
      <feature>Mcp-Protocol-Version header (2025-06-18)</feature>
      <feature>Dynamic protocol version negotiation</feature>
      <feature>Content negotiation: Accept header determines response type</feature>
      <feature>202 Accepted for notifications (no response expected)</feature>
    </features>
  </capsule>

  <mcp-spec-compliance spec="2025-06-18">
    <requirement name="Unified endpoint" status="✅">Single /mcp endpoint for all methods</requirement>
    <requirement name="Session header" status="✅">Mcp-Session-Id in header (not query param)</requirement>
    <requirement name="Content negotiation" status="✅">JSON or SSE based on Accept header</requirement>
    <requirement name="Protocol version negotiation" status="✅">Server echoes client's requested version</requirement>
    <requirement name="DELETE support" status="✅">Explicit session termination</requirement>
  </mcp-spec-compliance>

  <backwards-compatibility>
    <note>Both /sse (legacy) and /mcp (new) endpoints supported simultaneously</note>
    <deprecation-timeline>
      <phase>Now: /mcp available alongside /sse</phase>
      <phase>+3 months: Deprecation warning in /sse responses</phase>
      <phase>+12 months: Move /sse to legacy-sse feature flag</phase>
    </deprecation-timeline>
  </backwards-compatibility>
</streamable-http-transport>

<!-- ============================================================================
     OAUTH 2.1 + GOOGLE AUTHENTICATION (NEW - 2025-12-10)
     ============================================================================ -->
<oauth-authentication version="1.0" status="PRODUCTION" spec="OAuth 2.1 + RFC 7636 PKCE">
  <description>Complete OAuth 2.1 implementation with Google as identity provider</description>
  <google-oauth-status>✅ CONFIGURED (client_id: 895635138024-8elt5mbuut1vj4n5kko0kdh38rbl0kee)</google-oauth-status>

  <oauth-flow>
    <step order="1">Client discovers OAuth endpoints via /.well-known/oauth-authorization-server</step>
    <step order="2">Client redirects to /oauth/authorize with state + PKCE code_challenge</step>
    <step order="3">Server stores state/PKCE, redirects to Google OAuth</step>
    <step order="4">User authenticates with Google</step>
    <step order="5">Google redirects to /oauth/callback with authorization code</step>
    <step order="6">Server exchanges Google code for tokens, validates ID token</step>
    <step order="7">Server auto-provisions Hobby license for new users OR links Google ID to existing license</step>
    <step order="8">Server generates MCP authorization code, redirects to Claude callback</step>
    <step order="9">Client exchanges MCP code for access token at /oauth/token with PKCE verification</step>
    <step order="10">Client uses Bearer token for authenticated MCP requests</step>
  </oauth-flow>

  <oauth-capsules total="4" lines="3,600+" tests="94">
    <capsule name="OAuthStateCapsule" file="oauth/state_capsule.rs" tier="T1 Atomic" size="16KB" alignment="64B">
      <loc>~900 lines</loc>
      <tests>24 unit tests</tests>
      <slots>256 concurrent OAuth flows</slots>
      <latency>&lt;50ns lookup, &lt;100ns insert</latency>
      <ttl>600 seconds (10 minutes)</ttl>
      <purpose>CSRF state parameter storage with PKCE code_challenge</purpose>
      <security>
        <csrf>256-bit random state parameter prevents cross-site request forgery</csrf>
        <pkce>SHA-256 S256 code challenge prevents authorization code interception</pkce>
        <one-time-use>State consumed after validation (replay attack prevention)</one-time-use>
      </security>
    </capsule>

    <capsule name="GoogleOAuthClientCapsule" file="oauth/google_client.rs" tier="T1 Atomic" size="512B" alignment="64B">
      <loc>~600 lines</loc>
      <tests>17 unit tests</tests>
      <latency>&lt;1ms URL generation, ~500ms token exchange (network)</latency>
      <purpose>Google OAuth 2.0 integration for token exchange and user info</purpose>
      <google-endpoints>
        <auth>https://accounts.google.com/o/oauth2/v2/auth</auth>
        <token>https://oauth2.googleapis.com/token</token>
        <userinfo>https://www.googleapis.com/oauth2/v2/userinfo</userinfo>
        <scopes>openid email profile</scopes>
      </google-endpoints>
      <security>
        <id-token-validation>JWT RS256 signature verification with Google's public keys</id-token-validation>
        <claims-validation>iss, aud, exp validation per OpenID Connect spec</claims-validation>
      </security>
    </capsule>

    <capsule name="OAuthUserCapsule" file="oauth/user_mapping.rs" tier="T1 Atomic" size="17KB" alignment="256B">
      <loc>916 lines</loc>
      <tests>25 unit tests</tests>
      <slots>1,024 user mappings</slots>
      <latency>&lt;50ns lookup, &lt;100ns link</latency>
      <purpose>Maps Google user IDs (sub) to license keys</purpose>
      <auto-provisioning>
        <new-users>Generates KDB-HOBBY-{timestamp}-{email_hash} license</new-users>
        <existing-users>Links Google ID to existing license via email lookup</existing-users>
      </auto-provisioning>
    </capsule>

    <capsule name="AuthorizationCodeCapsule" file="oauth/authorization_codes.rs" tier="T1 Atomic" size="25KB" alignment="256B">
      <loc>787 lines</loc>
      <tests>28 unit tests</tests>
      <slots>512 authorization codes</slots>
      <latency>&lt;100ns generate, &lt;50ns validate</latency>
      <ttl>60 seconds (OAuth 2.1 recommendation)</ttl>
      <purpose>MCP authorization codes with PKCE validation</purpose>
      <security>
        <one-time-use>Code consumed on validation (replay prevention)</one-time-use>
        <pkce-verification>S256 code_verifier must match code_challenge</pkce-verification>
        <redirect-uri-binding>redirect_uri validated on token exchange</redirect-uri-binding>
        <cryptographic-random>256-bit codes via ring::rand::SystemRandom</cryptographic-random>
      </security>
    </capsule>
  </oauth-capsules>

  <oauth-endpoints>
    <metadata path="/.well-known/oauth-authorization-server" status="✅">
      Returns issuer, authorization_endpoint, token_endpoint, code_challenge_methods_supported
    </metadata>
    <metadata path="/.well-known/oauth-protected-resource" status="✅">
      Returns authorization_servers array pointing to OAuth server
    </metadata>
    <authorize path="/oauth/authorize" method="GET" status="✅">
      Stores state/PKCE, redirects to Google OAuth
    </authorize>
    <callback path="/oauth/callback" method="GET" status="✅">
      Exchanges Google code for tokens, provisions user, redirects to Claude
    </callback>
    <token path="/oauth/token" method="POST" status="✅">
      Validates PKCE, returns access token (1-year expiry)
    </token>
    <register path="/register" method="POST" status="✅">
      Dynamic Client Registration per RFC 7591
    </register>
  </oauth-endpoints>

  <google-cloud-setup>
    <project>kindly (project ID: kindly-465221)</project>
    <client-id>895635138024-8elt5mbuut1vj4n5kko0kdh38rbl0kee.apps.googleusercontent.com</client-id>
    <redirect-uri>https://mcp.kindly.software/oauth/callback</redirect-uri>
    <scopes>openid email profile</scopes>
    <consent-screen>External, production-ready</consent-screen>
  </google-cloud-setup>

  <environment-variables>
    <var name="GOOGLE_CLIENT_ID">895635138024-8elt5mbuut1vj4n5kko0kdh38rbl0kee.apps.googleusercontent.com</var>
    <var name="GOOGLE_CLIENT_SECRET">GOCSPX-*** (stored in /etc/kdb/oauth.env, mode 0600)</var>
    <var name="OAUTH_CALLBACK_URL">https://mcp.kindly.software/oauth/callback</var>
  </environment-variables>

  <known-issues>
    <issue severity="high" platform="Claude Desktop">
      <name>OAuth flow completes but connection fails (GitHub #5826)</name>
      <symptom>OAuth completes in browser, redirects to Claude Desktop, but shows "Disconnected"</symptom>
      <root-cause>Claude Desktop infrastructure bug - server never receives requests after OAuth</root-cause>
      <workaround>Use API key authentication (X-License-Key header) instead of OAuth</workaround>
      <status>Anthropic aware, no ETA for fix</status>
    </issue>
    <issue severity="medium" platform="Claude Code CLI">
      <name>HTTP transport requires API key in headers</name>
      <symptom>Initialize succeeds, tools/list fails with 401</symptom>
      <fix>Add X-License-Key to headers in MCP config</fix>
    </issue>
  </known-issues>

  <authentication-methods>
    <method name="OAuth 2.1 + Google" status="✅ Implemented">
      <flow>Authorization Code with PKCE (S256)</flow>
      <auto-provision>Yes - creates Hobby license for new Google users</auto-provision>
      <claude-desktop-status>❌ Blocked by Anthropic bug #5826</claude-desktop-status>
      <claude-code-status>⚠️ Requires local bridge for full compatibility</claude-code-status>
    </method>
    <method name="API Key (X-License-Key)" status="✅ Production">
      <header>X-License-Key: KDB-{TIER}-{timestamp}-{hash}</header>
      <validation>&lt;10ns cached FNV-1a + Ed25519 validation</validation>
      <claude-desktop-status>✅ Works</claude-desktop-status>
      <claude-code-status>✅ Works</claude-code-status>
    </method>
    <method name="Bearer Token (Authorization)" status="✅ Production">
      <header>Authorization: Bearer {license-key or oauth-token}</header>
      <validation>&lt;10ns cached FNV-1a validation</validation>
      <claude-desktop-status>✅ Works</claude-desktop-status>
      <claude-code-status>✅ Works</claude-code-status>
    </method>
  </authentication-methods>

  <uce35-compliance>
    <q10-tier>T1 Atomic for all OAuth capsules</q10-tier>
    <q33-lockfree>100% lockfree (AtomicU64, FNV-1a hash tables, CAS operations)</q33-lockfree>
    <q34-audit>OAuth events logged (authorize, callback, token, failures)</q34-audit>
    <cache-alignment>64B/256B alignment prevents false sharing</cache-alignment>
    <generation-counters>TOCTOU prevention on all state transitions</generation-counters>
  </uce35-compliance>

  <performance-targets>
    <operation name="Content negotiation" latency="&lt;50ns"/>
    <operation name="OAuth state storage" latency="&lt;50ns"/>
    <operation name="PKCE S256 validation" latency="&lt;100ns (SHA-256)"/>
    <operation name="Session lookup" latency="&lt;50ns (FNV-1a hash)"/>
    <operation name="Google token exchange" latency="~500ms (network bound)"/>
    <operation name="ID token validation" latency="&lt;1ms (JWT RS256)"/>
    <operation name="License auto-provision" latency="&lt;100ns (hash table insert)"/>
  </performance-targets>
</oauth-authentication>

<!-- ============================================================================
     TIER ENFORCEMENT SYSTEM (NEW - 2025-12-06)
     ============================================================================ -->
<tier-enforcement version="1.0" payment-provider="Paddle (agnostic)">
  <description>T1 Atomic tier-based subscription enforcement with 20% grace period</description>
  <total-latency>&lt;200ns per request</total-latency>

  <subscription-tiers>
    <tier name="Hobby" value="0" rpm="60" burst="10" sessions="5/month" snapshots="100" retention="7d" features="0x0F">
      <step-backward-limit>3/day</step-backward-limit>
      <memory-replay>disabled</memory-replay>
    </tier>
    <tier name="Pro" value="1" rpm="300" burst="30" sessions="100/month" snapshots="1000" retention="7d" features="0x1F" note="was Starter">
      <step-backward>unlimited</step-backward>
      <memory-replay>basic</memory-replay>
    </tier>
    <tier name="Engineer" value="2" rpm="1000" burst="100" sessions="500/month" snapshots="10000" retention="30d" features="0x3F" note="was Developer">
      <memory-replay>full</memory-replay>
      <find-similar-bugs>enabled (LSH)</find-similar-bugs>
      <read-memory-at-snapshot>enabled</read-memory-at-snapshot>
    </tier>
    <tier name="Teams" value="3" rpm="5000" burst="500" sessions="2000/month" snapshots="100000" retention="90d" features="0xFF" note="was Professional">
      <seats>5 (+$20/seat)</seats>
      <team-audit-logs>enabled</team-audit-logs>
      <memory-integrity-verification>enabled</memory-integrity-verification>
    </tier>
    <tier name="Enterprise" value="4" rpm="MAX" burst="MAX" sessions="unlimited" snapshots="MAX" retention="365d+" features="0x3FF">
      <compliance>SOX/SOC2/GDPR/HIPAA</compliance>
      <audit-trail>Q34 cryptographic hash-chain</audit-trail>
      <retention>custom (up to 7 years)</retention>
    </tier>
  </subscription-tiers>

  <trial-period status="ACTIVE">
    <duration>7 days</duration>
    <feature-level>Enterprise (0x3FF - all features)</feature-level>
    <sessions>Unlimited</sessions>
    <credit-card-required>No</credit-card-required>
  </trial-period>

  <feature-flags>
    <flag bit="0" name="TIME_TRAVEL" tiers="all">Bidirectional replay</flag>
    <flag bit="1" name="BREAKPOINTS" tiers="all">Breakpoint management</flag>
    <flag bit="2" name="STACK_TRACE" tiers="all">Stack unwinding</flag>
    <flag bit="3" name="AUDIT_TRAIL" tiers="all">Export audit traces</flag>
    <flag bit="4" name="MEMORY_READ" tiers="Pro+">Read process memory</flag>
    <flag bit="5" name="MEMORY_WRITE" tiers="Engineer+">Write process memory</flag>
    <flag bit="6" name="SYMBOL_RESOLUTION" tiers="Teams+">DWARF symbol lookup</flag>
    <flag bit="7" name="STEP_BACKWARD_UNLIMITED" tiers="Pro+">Unlimited time-travel backward (Hobby: 3/day)</flag>
    <flag bit="8" name="PRIORITY_SUPPORT" tiers="Enterprise">Priority support queue</flag>
    <flag bit="9" name="CUSTOM_RETENTION" tiers="Enterprise">Custom retention policies (up to 7 years)</flag>
  </feature-flags>

  <tier-specific-limits>
    <hobby>
      <step-backward>3 per day</step-backward>
      <memory-replay>disabled</memory-replay>
      <find-similar-bugs>disabled</find-similar-bugs>
      <read-memory-at-snapshot>disabled</read-memory-at-snapshot>
    </hobby>
    <pro note="was Starter">
      <step-backward>unlimited</step-backward>
      <memory-replay>basic</memory-replay>
      <find-similar-bugs>disabled</find-similar-bugs>
      <read-memory-at-snapshot>disabled</read-memory-at-snapshot>
    </pro>
    <engineer note="was Developer">
      <step-backward>unlimited</step-backward>
      <memory-replay>full</memory-replay>
      <find-similar-bugs>enabled (LSH probabilistic search)</find-similar-bugs>
      <read-memory-at-snapshot>enabled</read-memory-at-snapshot>
    </engineer>
    <teams note="was Professional">
      <all-features>same as Engineer</all-features>
      <seats>5 included (+$20/additional seat)</seats>
      <team-audit-logs>enabled</team-audit-logs>
      <memory-integrity-verification>enabled</memory-integrity-verification>
    </teams>
    <enterprise>
      <everything>unlimited</everything>
      <compliance>SOX/SOC2/GDPR/HIPAA</compliance>
      <audit-trail>Q34 cryptographic hash-chain</audit-trail>
      <retention>custom (up to 7 years)</retention>
    </enterprise>
  </tier-specific-limits>

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
<modules total="61">
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

  <category name="Phase 2C: Infrastructure" files="10" lines="~4500">
    <module name="runtime.rs">T5 async runtime</module>
    <module name="http_transport.rs" lines="1076" tier="T6 Mixed">HttpTransportCapsule (512B, 256B-aligned), is_protocol_method() for MCP auth bypass, 7 tests</module>
    <module name="sse_transport.rs" lines="~600" tier="T5+T8">SseTransportCapsule, SSE event formatting, session management</module>
    <module name="sse_connection_pool.rs" lines="~400" tier="T4+T1">SseConnectionPoolCapsule (~13KB), 100 concurrent connections, bitmap tracking</module>
    <module name="stdio_transport.rs">T5 stdio transport</module>
    <module name="connection_pool.rs">Connection pooling</module>
    <module name="tool_executor.rs">Tool execution</module>
    <module name="feature_flags.rs">Feature flag system</module>
  </category>
</modules>

<!-- ============================================================================
     FEATURE FLAGS
     ============================================================================ -->
<feature-flags total="41">
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

  <category name="Phase 2C: Infrastructure" count="9">
    <flag name="runtime"/>
    <flag name="http-transport"/>
    <flag name="sse-transport" note="MCP 2024-11-05 SSE protocol (production live)"/>
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
  <npm-package version="1.0.1" published="2025-12-09">
    <name>@kindly-software-inc/kdb</name>
    <registry>https://registry.npmjs.org/</registry>
    <install>npm install @kindly-software-inc/kdb</install>
    <transport type="sse">
      <url>https://mcp.kindly.software/sse</url>
      <message-endpoint>POST /message?sessionId={uuid}</message-endpoint>
      <health-endpoint>GET /health</health-endpoint>
    </transport>
    <authentication>
      <type>api-key</type>
      <header>X-License-Key</header>
      <signup>https://kindly.software</signup>
    </authentication>
    <client-directory>/home/samuel/Primitives/Kindly-Debugger/kdb-mcp-client/</client-directory>
  </npm-package>

  <sse-server version="0.2.0" status="✅ PRODUCTION LIVE" updated="2025-12-09">
    <binary>/home/samuel/mcp_servers/kdb-mcp/bin/mcp_sse_server</binary>
    <source>/home/samuel/Primitives/Kindly-Debugger/kdb-mcp/src/bin/mcp_sse_server.rs</source>
    <port>8081</port>
    <systemd-service>kdb-mcp-sse.service</systemd-service>
    <cloudflare-tunnel>kindly-mcp (92183b6d-8059-4cb5-b1c2-1b4974b62f7a)</cloudflare-tunnel>
    <public-url>https://mcp.kindly.software</public-url>
    <capsules-in-use>
      <capsule>HttpTransportCapsule (512B, T6 Mixed) - HTTP request handling</capsule>
      <capsule>SseConnectionPoolCapsule (~13KB, T4+T1) - 100 connection slots</capsule>
      <capsule>RateLimiterCapsule (4KB, T1) - global token bucket</capsule>
      <capsule>McpServerCapsule (256KB, T6 Mixed) - request orchestration</capsule>
      <capsule>DebuggerCapsule (1MB, T6 Mixed) - kdb debugger core</capsule>
    </capsules-in-use>
    <verified-endpoints>
      <endpoint method="GET" path="/sse" response="200 text/event-stream" verified="2025-12-09">SSE connection</endpoint>
      <endpoint method="POST" path="/message?sessionId={uuid}" response="204" verified="2025-12-09">JSON-RPC messages</endpoint>
      <endpoint method="GET" path="/health" response="200 JSON" verified="2025-12-09">Health check</endpoint>
    </verified-endpoints>
    <mcp-methods-tested>
      <method name="initialize" auth-required="no" verified="2025-12-09">Protocol handshake</method>
      <method name="tools/list" auth-required="yes" verified="2025-12-09">Returns 27 tools</method>
    </mcp-methods-tested>
  </sse-server>

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
    <cmd name="build-sse">cargo build --release --bin mcp_sse_server --features "std,json-rpc,sse-transport,http-transport"</cmd>
    <cmd name="test">cargo test --lib --features "std,json-rpc"</cmd>
    <cmd name="test-http">cargo test http_transport --features "std,json-rpc,http-transport"</cmd>
    <cmd name="bench">ssh samuel@kindly-hub "cd ~/Primitives/kdb-mcp &amp;&amp; cargo bench"</cmd>
    <cmd name="clippy">cargo clippy --all-features</cmd>
    <cmd name="deploy-sse">scp target/release/mcp_sse_server samuel@kindly-hub:~/mcp_servers/kdb-mcp/bin/</cmd>
    <cmd name="restart-sse">ssh samuel@kindly-hub "sudo systemctl restart kdb-mcp-sse.service"</cmd>
    <cmd name="logs-sse">ssh samuel@kindly-hub "journalctl -u kdb-mcp-sse.service -f"</cmd>
  </commands>

  <key-files>
    <file path="src/server.rs">McpServerCapsule orchestrator (dispatch_tool at line 680)</file>
    <file path="src/http_transport.rs">HttpTransportCapsule (512B), is_protocol_method() at line 328, 7 tests at line 999</file>
    <file path="src/bin/mcp_sse_server.rs">SSE server binary (~800 lines), uses HttpTransportCapsule + SseConnectionPoolCapsule</file>
    <file path="src/tier_enforcement.rs">TierEnforcementCapsule + FeatureFlags</file>
    <file path="src/snapshot_quota.rs">SnapshotQuotaCapsule + EnforcementStage</file>
    <file path="src/session_tier_map.rs">SessionTierMapCapsule (FNV-1a hash table)</file>
    <file path="src/tools/mod.rs">27 MCP tool implementations</file>
  </key-files>
</quick-reference>

</project>
