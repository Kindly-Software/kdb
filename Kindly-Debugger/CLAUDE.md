<?xml version="1.0" encoding="UTF-8"?>
<claude-config version="1.2.0">
<project name="Kindly-Debugger" tier="T6-Mixed">

<overview>
  <tagline>World's first audit-compliant time-travel debugger ecosystem</tagline>
  <subtitle>Commercial MCP-delivered debugging platform with tiered licensing</subtitle>
  <size>~85,000 LOC | 6 projects | 55+ capsules | 200+ tests</size>
  <architecture>Unified monorepo with debugger core, MCP server, signup service, marketing, API docs, and documentation</architecture>
  <delivery-model>MCP (Model Context Protocol) - Platform-agnostic access via AI assistants</delivery-model>
  <licensing>Commercial product with Hobby (free), Pro, and Enterprise tiers</licensing>
  <trade-secret>Protected - NOT open source</trade-secret>
</overview>

<commercial-model>
  <status>Commercial product - NOT open source</status>
  <signup-url>https://api.kindly.software/api/v1/signup</signup-url>
  <website>https://kindly.software</website>

  <trial-promo status="ACTIVE">
    <description>7-day free trial with ALL features (Enterprise-level access)</description>
    <feature-mask>0x3FF (all 10 feature flags enabled)</feature-mask>
    <sessions>Unlimited during trial</sessions>
    <credit-card-required>No</credit-card-required>
    <after-trial>Automatically falls back to tier-based limits</after-trial>
  </trial-promo>

  <tiers>
    <tier name="Hobby" price="Free" sessions="5/month">
      <time-travel>Yes (3 step_backward/day)</time-travel>
      <memory-replay>No</memory-replay>
      <find-similar-bugs>No</find-similar-bugs>
      <read-memory-at-snapshot>No</read-memory-at-snapshot>
    </tier>
    <tier name="Pro" price="$19/month" sessions="100/month" note="was Starter">
      <time-travel>Unlimited</time-travel>
      <step-backward>Unlimited</step-backward>
      <memory-replay>Basic</memory-replay>
      <find-similar-bugs>No</find-similar-bugs>
      <read-memory-at-snapshot>No</read-memory-at-snapshot>
    </tier>
    <tier name="Engineer" price="$49/month" sessions="500/month" note="was Developer">
      <time-travel>Unlimited</time-travel>
      <step-backward>Unlimited</step-backward>
      <memory-replay>Full</memory-replay>
      <find-similar-bugs>Yes (LSH)</find-similar-bugs>
      <read-memory-at-snapshot>Yes</read-memory-at-snapshot>
    </tier>
    <tier name="Teams" price="$129/month" sessions="2,000/month" note="was Professional">
      <seats>5 included (+$20/seat)</seats>
      <time-travel>Unlimited</time-travel>
      <memory-replay>Full</memory-replay>
      <team-audit-logs>Yes</team-audit-logs>
      <memory-integrity-verification>Yes</memory-integrity-verification>
    </tier>
    <tier name="Enterprise" price="From $999/month" sessions="Unlimited">
      <everything>Unlimited</everything>
      <compliance>SOX/SOC2/GDPR/HIPAA</compliance>
      <audit-trail>Q34 cryptographic hash-chain</audit-trail>
      <retention>Custom (up to 7 years)</retention>
      <support>Priority + dedicated</support>
    </tier>
  </tiers>

  <platform-access>
    <note>Platform-agnostic via MCP: Users on any OS (macOS, Windows, Linux) connect to the MCP server</note>
    <note>Core debugger uses Linux ptrace, but users never interact with it directly</note>
    <note>Access through Claude Code, Cursor, or any MCP-compatible client</note>
  </platform-access>
</commercial-model>

<projects>
  <!-- CORE PROJECTS -->
  <project id="kdb" tier="T6" location="./kdb/">
    <purpose>Core debugger engine with ptrace, time-travel, Q34 audit</purpose>
    <loc>57,587</loc>
    <capsules>37</capsules>
    <binary>./kdb/target/release/kdb</binary>
    <platform>Linux x86_64 (server-side only, users access via MCP)</platform>
    <claude-md>./kdb/CLAUDE.md</claude-md>
  </project>

  <project id="kdb-mcp" tier="T6" location="./kdb-mcp/">
    <purpose>MCP server exposing debugger via Model Context Protocol - PRIMARY USER INTERFACE</purpose>
    <loc>75,962</loc>
    <capsules>18+</capsules>
    <depends-on>kdb (path = "../kdb")</depends-on>
    <note>This is how users interact with the debugger - platform-agnostic</note>
    <claude-md>./kdb-mcp/CLAUDE.md</claude-md>
  </project>

  <project id="kdb-signup" tier="T1" location="./kdb-signup/">
    <purpose>User signup and license generation service for Hobby tier</purpose>
    <endpoint>https://api.kindly.software/api/v1/signup</endpoint>
    <deployment>Fly.io (kindly-api.fly.dev)</deployment>
    <features>Ed25519 license generation, 7-day promo tracking, Hobby tier onboarding</features>
    <claude-md>./kdb-signup/CLAUDE.md</claude-md>
  </project>

  <!-- FRONTEND & MARKETING -->
  <project id="kindly-services" tier="T6" location="./kindly-services/">
    <purpose>Marketing website for kindly.software (Leptos/WASM)</purpose>
    <loc>7,584</loc>
    <components>10</components>
    <deployment>Cloudflare Pages</deployment>
    <build>trunk serve</build>
    <claude-md>./kindly-services/CLAUDE.md</claude-md>
  </project>

  <project id="kdb-api-landing" tier="T0" location="./kdb-api-landing/">
    <purpose>API documentation landing page (Leptos/WASM)</purpose>
    <build>trunk build</build>
  </project>

  <!-- DOCUMENTATION -->
  <project id="kdb-docs" tier="T0" location="./kdb-docs/">
    <purpose>User documentation for commercial product</purpose>
    <contents>Getting Started, API Reference, Tools, Authentication, FAQ, Pricing</contents>
    <note>Documentation for MCP-based access, not direct CLI usage</note>
  </project>
</projects>

<connections>
  <flow name="user-journey">
    <step>1. User discovers KDB via kindly.software marketing site or kdb-docs</step>
    <step>2. User signs up at https://api.kindly.software/api/v1/signup (kdb-signup service)</step>
    <step>3. User receives Ed25519-signed license key for their tier (Hobby free, Pro, Enterprise)</step>
    <step>4. User configures MCP client (Claude Code, Cursor, etc.) with API key</step>
    <step>5. User debugs via natural language through AI assistant (platform-agnostic)</step>
    <step>6. kdb-mcp validates license via LicenseValidatorCapsule</step>
    <step>7. MCP tools route to kdb core via path dependency</step>
    <step>8. kdb executes ptrace operations on Linux server, returns results via MCP</step>
  </flow>

  <dependencies>
    <dep from="kdb-mcp" to="kdb" type="path">Core debugger functionality</dep>
    <dep from="kdb-signup" to="kdb-mcp" type="license">License generation and validation</dep>
    <dep from="kindly-services" to="kdb-signup" type="api">Signup flow integration</dep>
    <dep from="kdb-api-landing" to="kdb-mcp" type="docs">API endpoint documentation</dep>
    <dep from="kdb-docs" to="kdb-mcp" type="docs">User documentation for MCP interface</dep>
  </dependencies>
</connections>

<trade-secret-notice>
  <status>PROTECTED - NOT OPEN SOURCE</status>
  <notice>
    This software is proprietary and confidential. All rights reserved.
    The KDB debugger ecosystem is a commercial product of Kindly Software.
    Unauthorized copying, distribution, or use is strictly prohibited.
  </notice>
  <commit-rules>
    <rule>NEVER push to public repositories</rule>
    <rule>Use [TRADE SECRET] tag in all commits</rule>
    <rule>LOCAL COMMITS ONLY unless authorized</rule>
  </commit-rules>
</trade-secret-notice>

<quick-start>
  <!-- Core debugger -->
  <build name="kdb">cd kdb && cargo build --release</build>

  <!-- MCP server -->
  <build name="kdb-mcp">cd kdb-mcp && cargo build --release --features std,runtime</build>

  <!-- Marketing site -->
  <build name="kindly-services">cd kindly-services && trunk serve</build>

  <!-- API landing page -->
  <build name="kdb-api-landing">cd kdb-api-landing && trunk build</build>
</quick-start>

<framework-compliance>
  <uce34>Q10 T6 Mixed tier | Q33 lockfree atomics | Q34 audit trails</uce34>
  <coca>100% computational capsules, zero mutex, cache-aligned</coca>
  <t28>200+ tests (unit/property/integration/production)</t28>
  <b32>Fair benchmarking, validated performance claims</b32>
  <assum>99.99% safe, all assumptions documented</assum>
</framework-compliance>

<directory-structure>
  <tree>
Kindly-Debugger/              (this directory - COMMERCIAL PRODUCT)
├── CLAUDE.md                 (this file - ecosystem overview)
├── kdb/                      (core debugger engine, T6, 57K LOC)
│   ├── CLAUDE.md             (Linux ptrace-based, server-side only)
│   └── src/
├── kdb-mcp/                  (MCP server - PRIMARY USER INTERFACE, T6, 76K LOC)
│   ├── CLAUDE.md             (platform-agnostic access via MCP)
│   └── src/
├── kdb-signup/               (signup service for Hobby tier, T1)
│   ├── CLAUDE.md             (https://api.kindly.software/api/v1/signup)
│   └── src/
├── kindly-services/          (marketing site, Leptos WASM)
│   ├── CLAUDE.md             (https://kindly.software)
│   └── src/
├── kdb-api-landing/          (API docs landing, Leptos WASM)
│   └── src/
└── kdb-docs/                 (user documentation for MCP interface)
    ├── GETTING_STARTED.md    (MCP client configuration)
    ├── API_REFERENCE.md      (MCP protocol details)
    ├── TOOLS.md              (MCP tool documentation)
    ├── AUTHENTICATION.md     (API key and licensing)
    └── FAQ.md                (Pricing, support, platform info)
  </tree>
</directory-structure>

</project>
</claude-config>
