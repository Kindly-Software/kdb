<?xml version="1.0" encoding="UTF-8"?>
<!-- kdb-signup - User Signup Service for KDB Hobby Tier -->
<!-- Version: 1.0.0 | Updated: 2025-12-07 | Status: Production Ready -->
<project name="kdb-signup" version="1.0.0">

<metadata>
  <description>User signup and license generation service for KDB Hobby Tier</description>
  <location>/home/samuel/Primitives/Kindly-Debugger/kdb-signup/</location>
  <api-url>https://api.kindly.software/v1</api-url>
  <size>~3,200 LOC | 3 T1 capsules | 4 API endpoints</size>
  <framework>Axum (async HTTP server)</framework>
  <deployment>kindly-hub (192.168.0.38:8091)</deployment>
  <protection>T1 Atomic capsules with UCE34/Chaos compliance</protection>
  <promo-period>7-day launch promo: unlimited sessions, then 5/month</promo-period>
  <status>Production Ready</status>
</metadata>

<!-- ============================================================================
     QUICK REFERENCE
     ============================================================================ -->
<quick-reference>
  <commands>
    <cmd name="check">cargo check --lib</cmd>
    <cmd name="test">cargo test --test integration_tests</cmd>
    <cmd name="build">cargo build --release</cmd>
    <cmd name="deploy">./deploy/deploy.sh</cmd>
    <cmd name="health">curl http://localhost:8091/health</cmd>
  </commands>

  <key-files>
    <file path="src/capsules/user_registration.rs" lines="200">T1 Atomic, 256B, rate limiting</file>
    <file path="src/capsules/email_verification.rs" lines="180">T1 Atomic, 256B, BLAKE3 tokens</file>
    <file path="src/capsules/license_generator.rs" lines="220">T1 Atomic, 512B, Ed25519 signing</file>
    <file path="src/routes/signup.rs" lines="730">Axum handlers for signup flow</file>
    <file path="src/email/resend_client.rs" lines="433">Resend API integration</file>
    <file path="src/email/disposable.rs" lines="150">Disposable email blocker</file>
    <file path="tests/integration_tests.rs" lines="893">33 T28 integration tests</file>
  </key-files>
</quick-reference>

<!-- ============================================================================
     PROJECT STRUCTURE
     ============================================================================ -->
<project-structure>
<category name="Capsules" path="src/capsules/" lines="600">
  <file name="mod.rs" lines="42">Module exports</file>
  <file name="user_registration.rs" lines="200">UserRegistrationCapsule (T1, 256B)</file>
  <file name="email_verification.rs" lines="180">EmailVerificationCapsule (T1, 256B)</file>
  <file name="license_generator.rs" lines="220">LicenseGeneratorCapsule (T1, 512B)</file>
</category>

<category name="Email" path="src/email/" lines="583">
  <file name="mod.rs" lines="20">Module exports</file>
  <file name="resend_client.rs" lines="433">Resend API client (async)</file>
  <file name="disposable.rs" lines="150">FNV-1a blocklist + mailchecker</file>
</category>

<category name="Routes" path="src/routes/" lines="800">
  <file name="mod.rs" lines="30">Router configuration</file>
  <file name="signup.rs" lines="730">POST /signup, GET /verify, POST /resend</file>
</category>

<category name="Database" path="src/db/" lines="544">
  <file name="mod.rs" lines="20">Module exports</file>
  <file name="kindlydb_client.rs" lines="544">KindlyDB HTTP client (planned)</file>
</category>

<category name="Core" path="src/" lines="250">
  <file name="lib.rs" lines="100">Library root with feature flags</file>
  <file name="main.rs" lines="150">Axum server entry point</file>
</category>

<category name="Tests" path="tests/" lines="893">
  <file name="integration_tests.rs" lines="893">33 T28 integration tests</file>
</category>

<category name="Deploy" path="deploy/" lines="839">
  <file name="deploy.sh" lines="206">Deployment automation script</file>
  <file name="kdb-signup.service" lines="43">SystemD unit file</file>
  <file name="kdb-signup.env.template" lines="41">Environment variable template</file>
  <file name="README.md" lines="549">Deployment documentation</file>
</category>
</project-structure>

<!-- ============================================================================
     API ENDPOINTS
     ============================================================================ -->
<api-endpoints count="4">
  <endpoint method="POST" path="/api/v1/signup">
    <description>Create new user, send verification email</description>
    <request>{ "email": "user@example.com", "org_name": "Acme Corp" }</request>
    <response>{ "status": "verification_sent", "email_hash": "..." }</response>
    <rate-limit>5 signups/IP/hour</rate-limit>
  </endpoint>

  <endpoint method="GET" path="/api/v1/verify/{token}">
    <description>Verify email, generate license, redirect to /verified</description>
    <response>302 Redirect to /#verified?license={key}</response>
  </endpoint>

  <endpoint method="POST" path="/api/v1/resend-verification">
    <description>Resend verification email</description>
    <request>{ "email": "user@example.com" }</request>
    <response>{ "status": "verification_resent" }</response>
  </endpoint>

  <endpoint method="GET" path="/health">
    <description>Health check for monitoring</description>
    <response>{ "status": "healthy", "capsules": { "registration": {...}, ... } }</response>
  </endpoint>
</api-endpoints>

<!-- ============================================================================
     CAPSULE ARCHITECTURE
     ============================================================================ -->
<capsules tier="T1-Atomic">
  <capsule name="UserRegistrationCapsule" size="256B" align="64B">
    <purpose>Rate limiting and signup validation</purpose>
    <fields>
      <field name="registrations_total" type="AtomicU64">Total successful signups</field>
      <field name="blocked_count" type="AtomicU64">Blocked by rate limit</field>
      <field name="generation" type="AtomicU64">TOCTOU prevention counter</field>
      <field name="last_ip_hash" type="AtomicU64">FNV-1a hash of last IP</field>
      <field name="ip_counter" type="AtomicU64">Requests from current IP</field>
    </fields>
    <latency>less than 10ns rate check</latency>
  </capsule>

  <capsule name="EmailVerificationCapsule" size="256B" align="64B">
    <purpose>Token generation and verification</purpose>
    <fields>
      <field name="tokens_generated" type="AtomicU64">Total tokens created</field>
      <field name="tokens_verified" type="AtomicU64">Successfully verified</field>
      <field name="generation" type="AtomicU64">TOCTOU prevention counter</field>
      <field name="failed_attempts" type="AtomicU64">Max 5 per token</field>
    </fields>
    <algorithm>BLAKE3 hashing, 24h expiry</algorithm>
    <latency>less than 50ns token generation</latency>
  </capsule>

  <capsule name="LicenseGeneratorCapsule" size="512B" align="128B">
    <purpose>Ed25519 license signing with promo tracking</purpose>
    <fields>
      <field name="licenses_issued" type="AtomicU64">Total licenses generated</field>
      <field name="promo_licenses" type="AtomicU64">Licenses during promo period</field>
      <field name="promo_end_timestamp" type="AtomicU64">Unix timestamp (7 days from launch)</field>
      <field name="generation" type="AtomicU64">TOCTOU prevention counter</field>
    </fields>
    <format>KDB-{TIER}-{timestamp}-{org_hash}-{signature}</format>
    <latency>less than 1μs license generation</latency>
  </capsule>

  <coca-compliance>
    <lockfree>100% (AtomicU64 only, zero mutex)</lockfree>
    <cache-aligned>64B/128B padding</cache-aligned>
    <generation-counters>All capsules have generation field</generation-counters>
  </coca-compliance>
</capsules>

<!-- ============================================================================
     PROMO PERIOD LOGIC
     ============================================================================ -->
<promo-period>
  <duration>7 days (604,800 seconds)</duration>
  <constant>PROMO_DURATION_SECS = 604_800</constant>
  <start>License capsule creation time (stored in promo_start_timestamp)</start>
  <end>promo_start_timestamp + PROMO_DURATION_SECS</end>

  <hobby-tier-limits>
    <during-promo>Unlimited sessions</during-promo>
    <after-promo>5 sessions/month</after-promo>
  </hobby-tier-limits>

  <enforcement>
    <check-method>is_promo_active() -> bool</check-method>
    <api>sessions_per_month() returns 5 for Hobby (after promo)</api>
    <api>promo_sessions_per_month() returns u64::MAX for Hobby (during promo)</api>
  </enforcement>
</promo-period>

<!-- ============================================================================
     DEPENDENCIES
     ============================================================================ -->
<dependencies>
  <external>
    <dep name="axum" version="0.7">Async HTTP server</dep>
    <dep name="tokio" version="1.0" features="full">Async runtime</dep>
    <dep name="resend-rs" version="0.9">Resend email API</dep>
    <dep name="mailchecker" version="6">Disposable email detection (55K+ domains)</dep>
    <dep name="ed25519-dalek" version="2.0">Ed25519 signing</dep>
    <dep name="blake3" version="1.0">BLAKE3 hashing</dep>
    <dep name="serde" version="1.0">Serialization</dep>
    <dep name="serde_json" version="1.0">JSON handling</dep>
  </external>

  <internal>
    <dep name="kdb" path="../kdb">License format compatibility</dep>
  </internal>
</dependencies>

<!-- ============================================================================
     TESTING (T28 5-Tier)
     ============================================================================ -->
<testing framework="T28">
  <summary>33 integration tests (33 passing, 1 ignored)</summary>

  <tier name="Q1-Q7: Unit-Level Integration" count="16">
    <test>Signup success scenarios (gmail, subdomain, plus addressing)</test>
    <test>Email format validation (invalid, empty, missing @, no domain)</test>
    <test>Disposable email blocking (mailinator, tempmail, guerrillamail)</test>
    <test>Token format validation</test>
  </tier>

  <tier name="Q8-Q14: Rate Limiting" count="2">
    <test>5 signups per IP enforcement</test>
    <test>Rate limit check endpoint</test>
  </tier>

  <tier name="Q15-Q21: Full Integration" count="4">
    <test>Email verification flow with redirects</test>
    <test>Token validation</test>
    <test>Resend verification flow</test>
  </tier>

  <tier name="Q22-Q28: Production Simulation" count="2">
    <test>Promo period active (unlimited sessions)</test>
    <test>Promo period expired (5 sessions/month)</test>
  </tier>

  <tier name="Q29-Q35: Determinism" count="9">
    <test>Email hash determinism (case-insensitive)</test>
    <test>Verification token uniqueness</test>
    <test>License key format consistency</test>
    <test>Generation counter monotonicity</test>
    <test>Capsule size and alignment verification</test>
  </tier>

  <commands>
    <cmd>cargo test --test integration_tests</cmd>
    <cmd>cargo test --test integration_tests -- --test-threads=1 --include-ignored</cmd>
  </commands>
</testing>

<!-- ============================================================================
     DEPLOYMENT
     ============================================================================ -->
<deployment target="kindly-hub">
  <host>192.168.0.38</host>
  <port>8091</port>
  <user>kdb</user>
  <working-dir>/opt/kdb-signup</working-dir>
  <config-dir>/etc/kdb</config-dir>

  <systemd>
    <service>kdb-signup.service</service>
    <start>sudo systemctl start kdb-signup</start>
    <stop>sudo systemctl stop kdb-signup</stop>
    <logs>sudo journalctl -u kdb-signup -f</logs>
  </systemd>

  <environment-variables>
    <var name="RESEND_API_KEY" required="true">Email delivery API key</var>
    <var name="SIGNING_KEY_PATH" required="true">Ed25519 private key file</var>
    <var name="KINDLYDB_URL" default="http://localhost:8080">KindlyDB endpoint</var>
    <var name="VERIFICATION_URL" default="https://api.kindly.software/v1/verify">Verify endpoint</var>
    <var name="FROM_EMAIL" default="noreply@kindly.software">Sender address</var>
    <var name="RUST_LOG" default="info">Log level</var>
    <var name="PORT" default="8091">HTTP port</var>
  </environment-variables>

  <deploy-script>./deploy/deploy.sh</deploy-script>
</deployment>

<!-- ============================================================================
     FRAMEWORK COMPLIANCE
     ============================================================================ -->
<compliance>
  <uce34>Q10 T1 Atomic tier | Q33 lockfree | Q34 audit trails (promo tracking)</uce34>
  <coca>100% lockfree (AtomicU64 only), cache-aligned (64B/128B)</coca>
  <t28>33/33 integration tests (5-tier structure)</t28>
  <assum>99.99% safe (no unsafe blocks)</assum>
  <i20>Integrates with kdb-mcp license validation</i20>
</compliance>

<!-- ============================================================================
     SIGNATURE
     ============================================================================ -->
<signature>
  <project>kdb-signup</project>
  <version>1.0.0</version>
  <description>User Signup Service for KDB Hobby Tier</description>
  <api-url>https://api.kindly.software/v1</api-url>
  <size>~3,200 LOC | 3 T1 capsules | 33 tests</size>
  <framework>Axum + Resend + Ed25519</framework>
  <deployment>kindly-hub:8091</deployment>
  <protection>T1 Atomic (UCE34/Chaos compliant)</protection>
  <promo>7-day launch: unlimited, then 5 sessions/month</promo>
  <date>2025-12-07</date>
</signature>

</project>
