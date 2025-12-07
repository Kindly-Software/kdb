<?xml version="1.0" encoding="UTF-8"?>
<!-- kindly-services - Kindly Debugger Marketing Website -->
<!-- Version: 1.0.0 | Updated: 2025-12-06 | Status: Production (Cloudflare Pages) -->
<project name="kindly-services" version="1.0.0">

<metadata>
  <description>Marketing landing page for Kindly Debugger - "Time-travel debugging for AI workflows"</description>
  <location>/home/samuel/Primitives/kindly-services/</location>
  <url>https://kindly.services</url>
  <size>7,584 LOC | 10 components | 5 pages</size>
  <framework>Leptos 0.7 (CSR/WASM)</framework>
  <deployment>Cloudflare Pages (auto-deploy on push)</deployment>
  <bundle-size>331 KB WASM + 38 KB JS (~140-180 KB gzipped)</bundle-size>
  <protection>T6 Mixed (T1 + T0 + T9) with COCA compliance</protection>
  <trade-secret>YES - HTTP server and protection capsules protected</trade-secret>
  <status>Production Ready</status>
</metadata>

<!-- ============================================================================
     QUICK REFERENCE
     ============================================================================ -->
<quick-reference>
  <commands>
    <cmd name="dev">trunk serve --port 8080</cmd>
    <cmd name="build">trunk build --release</cmd>
    <cmd name="deploy">npx wrangler pages deploy dist --project-name=kindly-services</cmd>
    <cmd name="http-server">cargo build --release --bin http_server</cmd>
  </commands>

  <key-files>
    <file path="src/lib.rs" lines="223">Main app + Leptos routing</file>
    <file path="src/components/pricing.rs" lines="382">5 pricing tiers</file>
    <file path="src/effects/mesh_gradient.rs" lines="286">WebGL2 animated background</file>
    <file path="src/security_orchestrator.rs" lines="800">T6 protection coordination</file>
    <file path="src/bin/http_server.rs" lines="1003">COCA static file server</file>
    <file path="Trunk.toml" lines="29">WASM build config</file>
    <file path="_headers" lines="43">Cloudflare security headers + CSP</file>
  </key-files>
</quick-reference>

<!-- ============================================================================
     PROJECT STRUCTURE
     ============================================================================ -->
<project-structure>
<category name="Components" path="src/components/" lines="3695">
  <file name="navbar.rs" lines="235">Navigation + hamburger mobile menu</file>
  <file name="hero.rs" lines="203">Hero section with shimmer animation</file>
  <file name="features.rs" lines="150">Feature cards grid</file>
  <file name="pricing.rs" lines="382">5 pricing tiers with feature comparison</file>
  <file name="docs.rs" lines="431">Documentation page</file>
  <file name="privacy.rs" lines="454">Privacy policy (GDPR compliant)</file>
  <file name="terms.rs" lines="527">Terms of service</file>
  <file name="license.rs" lines="845">License information</file>
  <file name="cta.rs" lines="151">Call-to-action section</file>
  <file name="footer.rs" lines="184">Footer with links</file>
</category>

<category name="Effects" path="src/effects/" lines="443">
  <file name="mesh_gradient.rs" lines="286">WebGL2 animated gradient (GPU rendering)</file>
  <file name="particles.rs" lines="145">Particle effects background</file>
</category>

<category name="Protection" path="src/" lines="1577">
  <file name="security_orchestrator.rs" lines="800">T6 Mixed security coordination</file>
  <file name="protection_state.rs" lines="777">T9+T0 encrypted audit trail</file>
</category>

<category name="Binaries" path="src/bin/" lines="1371">
  <file name="http_server.rs" lines="1003">T6 COCA static file server (12/12 tests)</file>
  <file name="supply_chain.rs" lines="368">Supply chain verification</file>
</category>

<category name="Core" path="src/" lines="327">
  <file name="lib.rs" lines="223">Main app + SPA routing</file>
  <file name="main.rs" lines="104">Server entry point</file>
</category>

<category name="Build" path="/" lines="200">
  <file name="Cargo.toml" lines="112">Dependencies + 6 feature flags</file>
  <file name="Trunk.toml" lines="29">WASM build (skip_wasm_opt=true)</file>
  <file name="index.html" lines="16KB">HTML template + inline CSS</file>
  <file name="_headers" lines="43">Cloudflare security headers</file>
</category>
</project-structure>

<!-- ============================================================================
     PAGES & ROUTING
     ============================================================================ -->
<pages total="5" routing="hash-based">
  <page path="/" component="Home">Hero + Features + Pricing + CTA</page>
  <page path="#docs" component="Docs">Documentation (431 lines)</page>
  <page path="#privacy" component="Privacy">Privacy Policy (GDPR)</page>
  <page path="#terms" component="Terms">Terms of Service</page>
  <page path="#license" component="License">License Information</page>
</pages>

<!-- ============================================================================
     DESIGN SYSTEM
     ============================================================================ -->
<design-system>
  <branding>
    <theme>Byzantine Royal</theme>
    <primary>#4B0082 (Indigo Purple)</primary>
    <accent>#FFD700 (Gold)</accent>
    <background>Linear gradient + WebGL2 mesh</background>
  </branding>

  <effects>
    <effect name="WebGL2 Mesh Gradient">Real-time GPU-rendered animated background</effect>
    <effect name="Glassmorphism">Frosted glass (backdrop-filter blur)</effect>
    <effect name="Particles">Animated floating particles</effect>
    <effect name="Shimmer">Logo shimmer animation</effect>
  </effects>

  <responsive>
    <breakpoint size="768px">Desktop/Tablet</breakpoint>
    <breakpoint size="480px">Mobile</breakpoint>
    <breakpoint size="375px">Minimum (iPhone SE)</breakpoint>
    <touch-target>48px minimum</touch-target>
    <hamburger>Signal-based toggle, slide-in panel</hamburger>
  </responsive>
</design-system>

<!-- ============================================================================
     TECHNICAL STACK
     ============================================================================ -->
<tech-stack>
  <framework name="Leptos" version="0.7">
    <render-mode>CSR (Client-Side Rendering)</render-mode>
    <routing>Hash-based SPA (no SSR)</routing>
    <build-tool>Trunk (WASM bundler)</build-tool>
    <wasm-opt>Skipped (bulk memory operations bypass)</wasm-opt>
  </framework>

  <dependencies>
    <dep name="leptos" version="0.7">Rust web framework</dep>
    <dep name="leptos_meta" version="0.7">Metadata management</dep>
    <dep name="leptos_router" version="0.7">SPA routing</dep>
    <dep name="wasm-bindgen" version="0.2">JS interop</dep>
    <dep name="web-sys" version="0.3">WebGL2, Canvas, DOM APIs</dep>
    <dep name="js-sys" version="0.3">JavaScript types</dep>
    <dep name="gloo-timers" version="0.3">Async timers</dep>
    <dep name="gloo-events" version="0.2">Event handling</dep>
    <dep name="serde" version="1.0">Serialization</dep>
    <dep name="getrandom" version="0.2">Random generation</dep>
    <dep name="atomic_capsule" optional="true">Protection capsules (native only)</dep>
  </dependencies>

  <bundle-metrics>
    <wasm-binary>331 KB (raw)</wasm-binary>
    <js-glue>38 KB</js-glue>
    <total>369 KB raw (~140-180 KB gzipped)</total>
    <load-time>Less than 500ms on modern networks</load-time>
  </bundle-metrics>
</tech-stack>

<!-- ============================================================================
     PROTECTION ARCHITECTURE (T6 Mixed)
     ============================================================================ -->
<protection tier="T6-Mixed">
  <description>COCA-compliant security orchestration with 3 capsules</description>
  <total-latency>Less than 200ns per request</total-latency>

  <capsules>
    <capsule name="SecurityOrchestrator" tier="T6" lines="800">
      <purpose>Coordinates 3 protection capsules</purpose>
      <latency>Less than 200ns total orchestration</latency>
    </capsule>
    <capsule name="AdaptiveRateLimiterCapsule" tier="T1" feature="rate-limiting">
      <purpose>Token bucket rate limiting</purpose>
      <latency>Less than 100ns</latency>
    </capsule>
    <capsule name="SecurityHeadersCapsule" tier="T1" feature="security-headers">
      <purpose>HTTP security headers (CSP, HSTS, X-Frame-Options)</purpose>
      <latency>Less than 50ns</latency>
    </capsule>
    <capsule name="HttpAuditLogCapsule" tier="T0" feature="http-audit">
      <purpose>Q34 hash-chain audit logging</purpose>
      <latency>Less than 50ns</latency>
    </capsule>
    <capsule name="EncryptedStateCapsule" tier="T9+T0" feature="encryption">
      <purpose>AES-256-GCM encryption at rest</purpose>
      <latency>Less than 5ms</latency>
    </capsule>
  </capsules>

  <coca-compliance>
    <lockfree>100% (zero mutex/RwLock)</lockfree>
    <cache-aligned>64B/128B padding</cache-aligned>
    <generation-counters>TOCTOU prevention</generation-counters>
    <verification>#[derive(ComputationalCapsule)]</verification>
  </coca-compliance>
</protection>

<!-- ============================================================================
     FEATURE FLAGS
     ============================================================================ -->
<feature-flags total="6">
  <flag name="security-headers">HTTP security headers (X-Frame-Options, CSP, HSTS)</flag>
  <flag name="http-audit">Q34 audit logging with BLAKE3 hash-chain</flag>
  <flag name="rate-limiting">T1 adaptive rate limiter (less than 100ns)</flag>
  <flag name="supply-chain">Supply chain verification binary</flag>
  <flag name="encryption">AES-256-GCM encryption at rest</flag>
  <flag name="full-protection">All above features enabled</flag>
</feature-flags>

<!-- ============================================================================
     SECURITY HEADERS (CSP)
     ============================================================================ -->
<security-headers file="_headers">
  <header name="Content-Security-Policy">
    <directive name="default-src">'self'</directive>
    <directive name="script-src">'self' 'wasm-unsafe-eval' https://static.cloudflareinsights.com</directive>
    <directive name="style-src">'self' 'unsafe-inline' https://fonts.googleapis.com</directive>
    <directive name="connect-src">'self' https://api.kindly.services https://cloudflareinsights.com</directive>
    <directive name="img-src">'self' data: blob:</directive>
    <directive name="font-src">'self' https://fonts.gstatic.com</directive>
  </header>
  <header name="X-Frame-Options">DENY</header>
  <header name="X-Content-Type-Options">nosniff</header>
  <header name="Strict-Transport-Security">max-age=31536000; includeSubDomains</header>
  <header name="Referrer-Policy">strict-origin-when-cross-origin</header>
</security-headers>

<!-- ============================================================================
     HTTP SERVER (COCA Binary)
     ============================================================================ -->
<http-server tier="T6-Mixed">
  <file>src/bin/http_server.rs</file>
  <lines>1,003</lines>
  <port>8082</port>
  <size>369 KB stripped</size>
  <tests>12/12 passing</tests>

  <features>
    <feature>SPA routing (index.html fallback)</feature>
    <feature>Path traversal prevention</feature>
    <feature>MIME type detection (19 types)</feature>
    <feature>Q34 audit logging</feature>
    <feature>100% lockfree (zero mutex)</feature>
  </features>

  <performance>
    <latency>Less than 200ns per request (coordinated security)</latency>
    <throughput>10,000+ requests/sec</throughput>
  </performance>

  <build>cargo build --release --bin http_server</build>
  <run>./target/release/http_server</run>
</http-server>

<!-- ============================================================================
     PRICING TIERS
     ============================================================================ -->
<pricing-tiers count="5">
  <tier name="Hobby" price="Free">
    <snapshots>100</snapshots>
    <retention>7 days</retention>
    <features>Time-travel, breakpoints, stack trace, audit trail</features>
  </tier>
  <tier name="Starter" price="$9.99/mo">
    <snapshots>1,000</snapshots>
    <retention>7 days</retention>
    <features>+ Memory read</features>
  </tier>
  <tier name="Developer" price="$49.99/mo">
    <snapshots>10,000</snapshots>
    <retention>30 days</retention>
    <features>+ Memory write</features>
  </tier>
  <tier name="Professional" price="$499/mo">
    <snapshots>100,000</snapshots>
    <retention>90 days</retention>
    <features>+ Symbol resolution, step backward, priority support</features>
  </tier>
  <tier name="Enterprise" price="Contact">
    <snapshots>Unlimited</snapshots>
    <retention>Custom</retention>
    <features>+ Custom retention, SLA guarantee</features>
  </tier>
</pricing-tiers>

<!-- ============================================================================
     PERFORMANCE (B32 Web Vitals)
     ============================================================================ -->
<performance framework="B32">
  <web-vitals>
    <metric name="LCP" target="1000ms" actual="750ms" status="Good"/>
    <metric name="FID" target="100ms" actual="100ms" status="Good"/>
    <metric name="CLS" target="0" actual="0" status="Perfect"/>
    <metric name="TTI" target="2000ms" actual="1500ms" status="Good"/>
  </web-vitals>

  <bundle>
    <target>500 KB</target>
    <actual>369 KB raw, ~140-180 KB gzipped</actual>
    <status>Under budget</status>
  </bundle>
</performance>

<!-- ============================================================================
     COMPLIANCE
     ============================================================================ -->
<compliance>
  <standard name="SOX">Hash-chain audit trails</standard>
  <standard name="SOC2">Access control, encryption</standard>
  <standard name="GDPR">Privacy policy, data retention</standard>
  <standard name="HIPAA">Audit logging, access control</standard>
</compliance>

<!-- ============================================================================
     DEPLOYMENT
     ============================================================================ -->
<deployment platform="Cloudflare Pages">
  <domain>https://kindly.services</domain>
  <auto-deploy>On push to main branch</auto-deploy>
  <build-command>trunk build --release</build-command>
  <output-directory>dist/</output-directory>

  <manual-deploy>
    <step>trunk build --release</step>
    <step>npx wrangler pages deploy dist --project-name=kindly-services</step>
  </manual-deploy>

  <assets>
    <asset name="index.html" size="17 KB">Compiled HTML</asset>
    <asset name="kindly-services-*.wasm" size="331 KB">WASM binary</asset>
    <asset name="kindly-services-*.js" size="38 KB">JS glue code</asset>
    <asset name="kdb-logo.jpg" size="435 KB">Hero logo</asset>
    <asset name="navbar-logo.png" size="471 KB">Navbar logo</asset>
  </assets>
</deployment>

<!-- ============================================================================
     DEVELOPMENT
     ============================================================================ -->
<development>
  <setup>
    <step>rustup target add wasm32-unknown-unknown</step>
    <step>cargo install trunk wasm-bindgen-cli</step>
  </setup>

  <commands>
    <cmd name="serve">trunk serve --port 8080</cmd>
    <cmd name="build">trunk build --release</cmd>
    <cmd name="check">cargo check --target wasm32-unknown-unknown</cmd>
  </commands>

  <hot-reload>Enabled via Trunk dev server</hot-reload>
</development>

<!-- ============================================================================
     ATOMIC CAPSULE INTEGRATION
     ============================================================================ -->
<atomic-capsule-integration optional="true">
  <description>Optional integration for native binaries (not WASM)</description>
  <path>../atomic_capsule</path>

  <features-used>
    <feature>std</feature>
    <feature>http</feature>
    <feature>security-adaptive-rate-limiter</feature>
    <feature>security-constant-time-ops</feature>
    <feature>encrypted-state</feature>
  </features-used>
</atomic-capsule-integration>

<!-- ============================================================================
     TRADE SECRET PROTECTION
     ============================================================================ -->
<trade-secret>
  <status>PROTECTED</status>
  <commit-tag>[TRADE SECRET] kindly-services</commit-tag>
  <protected-components>
    <component>HTTP server implementation (src/bin/http_server.rs)</component>
    <component>Security orchestrator (src/security_orchestrator.rs)</component>
    <component>Protection state (src/protection_state.rs)</component>
  </protected-components>
  <safe-to-ship>
    <component>WASM bundle (no proprietary algorithms)</component>
    <component>Static assets (logos, CSS)</component>
  </safe-to-ship>
</trade-secret>

<!-- ============================================================================
     SIGNATURE
     ============================================================================ -->
<signature>
  <project>kindly-services</project>
  <version>1.0.0</version>
  <description>Kindly Debugger Marketing Website</description>
  <url>https://kindly.services</url>
  <size>7,584 LOC | 10 components | 5 pages</size>
  <framework>Leptos 0.7 (CSR/WASM)</framework>
  <deployment>Cloudflare Pages</deployment>
  <bundle>331 KB WASM + 38 KB JS</bundle>
  <protection>T6 Mixed (T1 + T0 + T9)</protection>
  <http-server>T6 COCA binary (1,003 lines, 12/12 tests)</http-server>
  <compliance>SOX, SOC2, GDPR, HIPAA</compliance>
  <date>2025-12-06</date>
</signature>

</project>
