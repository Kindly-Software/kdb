<?xml version="1.0" encoding="UTF-8"?>
<claude-config version="1.0.0">
<project name="Kindly-Debugger" tier="T6-Mixed">

<overview>
  <tagline>World's first audit-compliant time-travel debugger ecosystem</tagline>
  <size>73,971 LOC | 5 projects | 55+ capsules | 200+ tests</size>
  <architecture>Debugger core + MCP server in this directory, related projects at Primitives level</architecture>
</overview>

<projects>
  <!-- CORE PROJECTS (in this directory) -->
  <project id="kdb" tier="T6" location="./kdb/">
    <purpose>Core debugger engine with ptrace, time-travel, Q34 audit</purpose>
    <loc>57,587</loc>
    <capsules>37</capsules>
    <binary>./kdb/target/release/kdb</binary>
    <claude-md>./kdb/CLAUDE.md</claude-md>
  </project>

  <project id="kdb-mcp" tier="T6" location="./kdb-mcp/">
    <purpose>MCP server exposing debugger via Model Context Protocol</purpose>
    <loc>8,900</loc>
    <capsules>18+</capsules>
    <depends-on>kdb (path = "../kdb")</depends-on>
    <claude-md>./kdb-mcp/CLAUDE.md</claude-md>
  </project>

  <!-- RELATED PROJECTS (at Primitives level - separate git repos) -->
  <project id="kindly-services" tier="T6" location="../kindly-services/">
    <purpose>Marketing website for kindly.services (Leptos/WASM)</purpose>
    <loc>7,584</loc>
    <components>10</components>
    <deployment>Cloudflare Pages</deployment>
    <note>Separate git repository - not moved to preserve history</note>
    <claude-md>../kindly-services/CLAUDE.md</claude-md>
  </project>

  <project id="kdb-api-landing" tier="T0" location="../kdb-api-landing/">
    <purpose>API documentation landing page</purpose>
    <status>Auxiliary</status>
    <note>Untracked in parent git - left in place</note>
  </project>

  <project id="kdb-docs" tier="T0" location="../kdb-docs/">
    <purpose>Comprehensive documentation site</purpose>
    <status>Auxiliary</status>
    <note>Separate git repository</note>
  </project>
</projects>

<connections>
  <flow name="user-to-debugger">
    <step>1. User visits kindly.services (../kindly-services/) - signs up for tier</step>
    <step>2. kdb-mcp validates license via LicenseValidatorCapsule</step>
    <step>3. MCP tools route to kdb core via path dependency</step>
    <step>4. kdb executes ptrace operations, captures snapshots</step>
  </flow>

  <dependencies>
    <dep from="kdb-mcp" to="kdb" type="path">Core debugger functionality</dep>
    <dep from="kindly-services" to="kdb-mcp" type="api">License/tier validation</dep>
  </dependencies>
</connections>

<quick-start>
  <!-- Core debugger -->
  <build name="kdb">cd kdb && cargo build --release</build>

  <!-- MCP server -->
  <build name="kdb-mcp">cd kdb-mcp && cargo build --release --features std,runtime</build>

  <!-- Marketing site (separate repo) -->
  <build name="kindly-services">cd ../kindly-services && trunk serve</build>
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
Kindly-Debugger/          (this directory)
├── CLAUDE.md             (this file - ecosystem overview)
├── kdb/                  (core debugger, T6)
│   ├── CLAUDE.md
│   └── src/
└── kdb-mcp/              (MCP server, T6)
    ├── CLAUDE.md
    └── src/

Related (at ../):
├── kindly-services/      (marketing site, separate git repo)
├── kdb-api-landing/      (API docs, untracked)
└── kdb-docs/             (documentation, separate git repo)
  </tree>
</directory-structure>

</project>
</claude-config>
