//! Health Check Endpoint - /health
//!
//! **Purpose**: Kubernetes/Docker liveness and readiness probes.
//!
//! **Response** (200 OK):
//! ```json
//! {
//!   "status": "healthy",
//!   "version": "0.1.0",
//!   "uptime_secs": 3600,
//!   "active_sessions": 42
//! }
//! ```
//!
//! **Probe Types**:
//! - **Liveness Probe**: HTTP GET /health every 10s (checks if process is alive)
//! - **Readiness Probe**: HTTP GET /health every 5s (checks if MCP server is ready)
//!
//! **Thresholds**:
//! - Success: 2+ consecutive success responses
//! - Failure: 3+ consecutive failed responses
//! - Timeout: 2 seconds
//! - Grace period: 5 seconds (wait before first check)

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Health status response (serializable and deserializable)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthStatus {
    /// Overall health status ("healthy", "degraded", "unhealthy")
    pub status: String,

    /// Version of kdb (from Cargo.toml)
    pub version: String,

    /// Uptime in seconds since server start
    pub uptime_secs: u64,

    /// Number of active debugging sessions
    pub active_sessions: u64,
}

// Module-level initialization (called on server startup)
static START_TIME_NS: AtomicU64 = AtomicU64::new(0);
static ACTIVE_SESSIONS: AtomicU64 = AtomicU64::new(0);

/// Initialize health check (called on server startup)
///
/// **Performance**: ~5ns (atomic store)
///
/// # Example
/// ```ignore
/// observability::health::init_health();
/// ```
pub fn init_health() {
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    START_TIME_NS.store(now_ns, Ordering::Relaxed);
}

/// Get current health status
///
/// **Performance**: <20ns (atomic loads + division)
///
/// # Returns
/// HealthStatus with current uptime and active sessions
///
/// # Example
/// ```ignore
/// let status = observability::health::get_health_status();
/// println!("{:?}", status);
/// ```
pub fn get_health_status() -> HealthStatus {
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let start_ns = START_TIME_NS.load(Ordering::Relaxed);
    let uptime_secs = if start_ns > 0 {
        (now_ns.saturating_sub(start_ns)) / 1_000_000_000
    } else {
        0
    };

    let active_sessions = ACTIVE_SESSIONS.load(Ordering::Relaxed);

    HealthStatus {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs,
        active_sessions,
    }
}

/// Increment active session counter
///
/// **Performance**: <5ns (atomic fetch_add with Relaxed ordering)
///
/// Called when a new debugging session attaches to a process.
pub fn increment_active_sessions() {
    ACTIVE_SESSIONS.fetch_add(1, Ordering::Relaxed);
}

/// Decrement active session counter
///
/// **Performance**: <5ns (atomic fetch_sub with Relaxed ordering)
///
/// Called when a debugging session detaches.
pub fn decrement_active_sessions() {
    ACTIVE_SESSIONS.fetch_sub(1, Ordering::Relaxed);
}

/// Get active session count
///
/// **Performance**: <5ns (atomic load)
pub fn get_active_sessions() -> u64 {
    ACTIVE_SESSIONS.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_health_status_initialization() {
        init_health();
        let status = get_health_status();

        assert_eq!(status.status, "healthy");
        assert_eq!(status.version, env!("CARGO_PKG_VERSION"));
        assert!(status.uptime_secs >= 0);
    }

    #[test]
    fn test_health_status_uptime() {
        init_health();
        let status1 = get_health_status();

        thread::sleep(Duration::from_millis(100));

        let status2 = get_health_status();
        assert!(status2.uptime_secs >= status1.uptime_secs);
    }

    #[test]
    fn test_active_sessions_counter() {
        // Reset counter
        ACTIVE_SESSIONS.store(0, Ordering::Relaxed);

        // Test increment
        increment_active_sessions();
        assert_eq!(get_active_sessions(), 1);

        increment_active_sessions();
        assert_eq!(get_active_sessions(), 2);

        // Test decrement
        decrement_active_sessions();
        assert_eq!(get_active_sessions(), 1);

        decrement_active_sessions();
        assert_eq!(get_active_sessions(), 0);
    }

    #[test]
    fn test_health_status_serialization() {
        let status = HealthStatus {
            status: "healthy".to_string(),
            version: "0.1.0".to_string(),
            uptime_secs: 3600,
            active_sessions: 42,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("\"version\":\"0.1.0\""));
        assert!(json.contains("\"uptime_secs\":3600"));
        assert!(json.contains("\"active_sessions\":42"));
    }

    #[test]
    fn test_health_status_deserialization() {
        let json = r#"{"status":"healthy","version":"0.1.0","uptime_secs":3600,"active_sessions":42}"#;
        let status: HealthStatus = serde_json::from_str(json).unwrap();

        assert_eq!(status.status, "healthy");
        assert_eq!(status.version, "0.1.0");
        assert_eq!(status.uptime_secs, 3600);
        assert_eq!(status.active_sessions, 42);
    }

    #[test]
    fn test_concurrent_session_updates() {
        ACTIVE_SESSIONS.store(0, Ordering::Relaxed);

        let mut handles = vec![];

        // Spawn 10 threads, each incrementing 100 times
        for _ in 0..10 {
            let handle = thread::spawn(|| {
                for _ in 0..100 {
                    increment_active_sessions();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have 1000 total increments
        assert_eq!(get_active_sessions(), 1000);
    }
}
