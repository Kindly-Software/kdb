//! # Load Balancing Capsules (T1+T8+T10)
//!
//! High-performance load balancing with health checking, session affinity, and consistent hashing.
//!
//! ## Architecture
//!
//! - **HealthCheckCapsule** (256B, T1+T8): Coordination and statistics tracking
//! - **BackendHealthState** (64B, T1): Per-backend health state with cache alignment
//! - **Active Health Checks**: HTTP GET/HEAD, TCP connect, ICMP ping
//! - **Passive Monitoring**: Request success/failure recording and state transitions
//! - **Circuit Breaker Integration**: Automatic state management based on failures
//! - **SessionAffinityCapsule** (256B, T1+T10): Sticky sessions with consistent hashing
//! - **Session Modes**: Cookie, ClientIP, Header, QueryParam, Custom
//! - **Consistent Hashing**: Jump hash with virtual nodes for minimal rebalancing
//!
//! ## Performance Targets (B32 Validated)
//!
//! ### Health Checking
//! - **<3ms HTTP health check** (with network latency)
//! - **<1ms TCP health check** (local network)
//! - **<50ns passive monitoring record** (lockfree atomic)
//! - **<100ns health status lookup** (cache-aligned)
//! - **<500ns state transition** (atomic CAS)
//!
//! ### Session Affinity
//! - **<500ns session lookup** (hash table atomic load)
//! - **<1μs cookie generation** (with HMAC-SHA256)
//! - **<300ns IP hash lookup** (direct hash)
//! - **<200ns consistent hash lookup** (binary search)
//! - **<1ms hash ring construction** (100 backends × 150 vnodes)
//!
//! ## Example
//!
//! ```rust,ignore
//! use atomic_capsule::load_balancing::{
//!     HealthCheckCapsule, HealthCheckType, HealthStatus,
//!     SessionAffinityCapsule, AffinityMode,
//! };
//!
//! // Health checking
//! let health_capsule = HealthCheckCapsule::new();
//! let result = health_capsule.check_http_health(1, "/health", 200)?;
//!
//! // Session affinity
//! let session_capsule = SessionAffinityCapsule::new();
//! let cookie = session_capsule.set_cookie_affinity("JSESSIONID", 42)?;
//! let backend = session_capsule.get_backend_from_cookie(&cookie)?;
//! ```

pub mod capsule;
pub mod backend_state;
pub mod check_types;
pub mod passive_monitoring;
pub mod circuit_breaker_integration;
pub mod session_affinity;

// Agent 56: Load Balancer Metrics & Observability (T0+T1)
pub mod metrics;

pub use capsule::HealthCheckCapsule;
pub use backend_state::BackendHealthState;
pub use check_types::{
    HealthCheckType, HealthCheckResult, HealthCheckError, HealthStatus, ErrorType,
};
pub use passive_monitoring::PassiveHealthMonitor;
pub use circuit_breaker_integration::CircuitBreakerIntegration;
pub use session_affinity::{
    AffinityError, AffinityMode, SessionAffinityCapsule, SessionEntry, SessionStatistics,
    SESSION_DEFAULT_TIMEOUT_MS, SESSION_DEFAULT_MAX_SESSIONS, SESSION_DEFAULT_VNODES_PER_BACKEND,
};
pub use metrics::{
    LoadBalancerMetricsCapsule, BackendMetrics, BackendState, MetricsSnapshot, Alert, AlertLevel,
    AlertThresholds,
};
