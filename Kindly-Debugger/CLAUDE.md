<?xml version="1.0" encoding="UTF-8"?>
<claude-config version="1.1.0">
<project name="Kindly-Debugger" tier="T6-Mixed">

<overview>
  <tagline>World's first audit-compliant time-travel debugger ecosystem</tagline>
  <size>~85,000 LOC | 5 projects | 55+ capsules | 200+ tests</size>
  <architecture>Unified monorepo with debugger core, MCP server, marketing, API docs, and documentation</architecture>
</overview>

<projects>
  <!-- CORE PROJECTS -->
  <project id="kdb" tier="T6" location="./kdb/">
    <purpose>Core debugger engine with ptrace, time-travel, Q34 audit</purpose>
    <loc>57,587</loc>
    <capsules>37</capsules>
    <binary>./kdb/target/release/kdb</binary>
    <claude-md>./kdb/CLAUDE.md</claude-md>
  </project>

  <project id="kdb-mcp" tier="T6" location="./kdb-mcp/">
    <purpose>MCP server exposing debugger via Model Context Protocol</purpose>
    <loc>75,962</loc>
    <capsules>18+</capsules>
    <depends-on>kdb (path = "../kdb")</depends-on>
    <claude-md>./kdb-mcp/CLAUDE.md</claude-md>
  </project>

  <!-- FRONTEND & MARKETING -->
  <project id="kindly-services" tier="T6" location="./kindly-services/">
    <purpose>Marketing website for kindly.services (Leptos/WASM)</purpose>
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
    <purpose>Comprehensive documentation site (Markdown)</purpose>
    <contents>Getting Started, API Reference, Tools, Authentication, FAQ</contents>
  </project>
</projects>

<connections>
  <flow name="user-journey">
    <step>1. User discovers kdb via kdb-docs (documentation)</step>
    <step>2. User visits kindly-services (marketing) → signs up for tier</step>
    <step>3. User references kdb-api-landing for API endpoints</step>
    <step>4. kdb-mcp validates license via LicenseValidatorCapsule</step>
    <step>5. MCP tools route to kdb core via path dependency</step>
    <step>6. kdb executes ptrace operations, captures snapshots</step>
  </flow>

  <dependencies>
    <dep from="kdb-mcp" to="kdb" type="path">Core debugger functionality</dep>
    <dep from="kindly-services" to="kdb-mcp" type="api">License/tier validation</dep>
    <dep from="kdb-api-landing" to="kdb-mcp" type="docs">API endpoint documentation</dep>
    <dep from="kdb-docs" to="kdb" type="docs">User documentation</dep>
  </dependencies>
</connections>

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
Kindly-Debugger/              (this directory)
├── CLAUDE.md                 (this file - ecosystem overview)
├── kdb/                      (core debugger, T6, 57K LOC)
│   ├── CLAUDE.md
│   └── src/
├── kdb-mcp/                  (MCP server, T6, 76K LOC)
│   ├── CLAUDE.md
│   └── src/
├── kindly-services/          (marketing site, Leptos WASM)
│   ├── CLAUDE.md
│   └── src/
├── kdb-api-landing/          (API docs landing, Leptos WASM)
│   └── src/
└── kdb-docs/                 (documentation, Markdown)
    ├── GETTING_STARTED.md
    ├── API_REFERENCE.md
    ├── TOOLS.md
    └── AUTHENTICATION.md
  </tree>
</directory-structure>

</project>
</claude-config>
