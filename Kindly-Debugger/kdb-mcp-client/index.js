/**
 * KDB - The Kindly Debugger
 * MCP Client Configuration
 * 
 * This package provides MCP client configuration for connecting to the
 * KDB time-travel debugger service.
 * 
 * Get your free license key at: https://kindly.software
 */

export const config = {
  name: "kdb",
  description: "AI-powered time-travel debugger with bidirectional execution replay, audit-compliant hash-chain logging, and SIMD-accelerated stack unwinding.",
  version: "1.0.1",
  transport: {
    type: "sse",
    url: "https://mcp.kindly.software/sse"
  },
  authentication: {
    type: "api-key",
    header: "X-License-Key",
    instructions: "Get your free license key at https://kindly.software"
  },
  tools: [
    "attach",
    "detach", 
    "breakpoint_set",
    "breakpoint_remove",
    "breakpoint_list",
    "step",
    "continue",
    "snapshot",
    "back",
    "stack",
    "memory_read",
    "registers"
  ],
  tiers: {
    hobby: { sessions: 5, price: "Free" },
    pro: { sessions: 100, price: "Coming soon" },
    enterprise: { sessions: "Unlimited", price: "Contact us" }
  }
};

export default config;
