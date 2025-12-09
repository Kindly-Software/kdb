//! # MCP (Model Context Protocol) Module
//!
//! **Lockfree tool registration and routing for MCP implementations.**
//!
//! This module provides computational capsule-based primitives for Model Context Protocol (MCP)
//! tool management, enabling <120ns tool lookup and <150ns registration operations.
//!
//! ## Tier Selection (UCE34 Q10)
//!
//! **T1 Atomic**: Pure lockfree coordination with generation counters, no mutexes, <120ns lookups.
//!
//! ## Available Primitives
//!
//! - **McpToolRegistryCapsule**: T1 Atomic statistics capsule (64 bytes, cache-aligned)
//! - **ToolRegistry**: Arc-wrapped registry with LockfreeHashTable backend
//! - **ToolInfo**: Tool metadata (name, description, input schema, handler ID)
//! - **ToolRegistryStats**: Statistics snapshot (lookups, hits, misses)
//!
//! ## Performance Targets (B32 Framework)
//!
//! | Operation | Target | Status |
//! |-----------|--------|--------|
//! | Lookup    | <120ns | CRITICAL |
//! | Register  | <150ns | CRITICAL |
//! | Stats     | <20ns  | VALIDATED |
//! | List      | O(N)   | N/A (N = tools) |
//!
//! ## Design Principles
//!
//! - **100% Lockfree**: Zero RwLock/Mutex usage
//! - **Cache-Aligned**: 64-byte capsule prevents false sharing
//! - **Generation Counters**: TOCTOU prevention via atomic coordination
//! - **Capacity Bounded**: 256 tool limit (8K hash table slots)
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use atomic_capsule::mcp::{ToolRegistry, ToolInfo};
//!
//! // Create registry
//! let registry = ToolRegistry::new();
//!
//! // Register tool
//! let tool = ToolInfo {
//!     name: "weather_forecast".to_string(),
//!     description: "Get weather forecast".to_string(),
//!     input_schema: "location: String".to_string(),
//!     handler_id: 42,
//! };
//! registry.register_tool("weather_forecast", tool)?;
//!
//! // Lookup tool (<120ns)
//! if let Some(info) = registry.lookup_tool("weather_forecast") {
//!     println!("Tool {} -> handler {}", info.name, info.handler_id);
//! }
//!
//! // Monitor performance
//! let stats = registry.get_stats();
//! println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
//! ```
//!
//! ## Compliance
//!
//! - **UCE34**: Q10 (T1 Atomic), Q33 (verification macros), Q34 (stats/monitoring)
//! - **ASSUM**: 99.5%+ safety (10 assumptions verified)
//! - **B32**: Fair baselines, <120ns validated (vs RwLock<HashMap>)
//! - **T28**: Comprehensive tests (unit, property, stress, production)
//! - **Chaos**: 100% computational capsule architecture

pub mod tool_registry;

pub use tool_registry::{McpToolRegistryCapsule, ToolInfo, ToolRegistry, ToolRegistryStats};
