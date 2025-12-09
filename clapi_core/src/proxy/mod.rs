//! HTTP Proxy Server Implementation
//!
//! Phase 2: Complete HTTP proxy with:
//! - Budget enforcement via RequestCapsule128
//! - Provider routing via RoutingCapsule128
//! - Metrics tracking via ResponseCapsule256
//! - Audit logging via AuditLogEntry128
//!
//! Week 2 UX: Test mode with MockProvider
//! - MockRouter for transparent test mode integration
//! - Zero-config testing without API keys
//! - Realistic latency and cost simulation

pub mod config;
pub mod types;
pub mod client;
pub mod budget_registry;
pub mod provider_router;
pub mod audit_log;
pub mod server;
pub mod mock_router;
pub mod cost_analyzer;
pub mod coalescing;
pub mod rate_limiter_jitter;
pub mod dashboard;
pub mod ws;
pub mod audit_bridge;
pub mod timeline_bridge;

pub use config::{ProxyConfig, ProviderConfig};
pub use types::{ChatCompletionRequest, ChatCompletionResponse, Message, Choice, Usage};
pub use client::ProviderClient;
pub use budget_registry::{BudgetRegistry, BudgetId};
pub use provider_router::ProviderRouter;
pub use audit_log::AuditLog;
pub use server::ProxyServer;
pub use mock_router::MockRouter;
pub use cost_analyzer::{CostAnalyzer, CostAnalysis, AlertLevel};
pub use coalescing::CoalescingRegistry;
pub use ws::{BroadcastState, MetricsMessage, create_ws_router, broadcast_metrics_task, get_broadcast_stats};
pub use audit_bridge::AuditLogBridge;
pub use timeline_bridge::{TimelineBridge, TimelineEvent};
