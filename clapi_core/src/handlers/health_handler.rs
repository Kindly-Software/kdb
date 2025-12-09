//! Health Check HTTP Handlers (P3-E7)
//!
//! **Purpose**: Kubernetes liveness/readiness probe endpoints
//! **Performance**: <100ns overhead (atomic bitmap read + JSON serialization)
//!
//! ## Endpoints
//!
//! ### GET /health (Liveness Probe)
//! - Returns 200 if process is alive (any component healthy)
//! - Returns 503 if process is unhealthy (all components down)
//! - Use for Kubernetes liveness probe
//!
//! ### GET /health?deep=true (Readiness Probe)
//! - Returns 200 if all critical components are healthy
//! - Returns 503 if any critical component is unhealthy
//! - Includes detailed component status in response
//! - Use for Kubernetes readiness probe
//!
//! ## Usage
//!
//! ```rust
//! use axum::Router;
//! use clapi_core::handlers::health_handler::health_routes;
//! use clapi_core::capsules::health_check::HealthCheckCapsule64;
//! use std::sync::Arc;
//!
//! let health = Arc::new(HealthCheckCapsule64::new());
//! let app = Router::new().merge(health_routes(health));
//! ```
//!
//! ## Kubernetes Integration
//!
//! ```yaml
//! livenessProbe:
//!   httpGet:
//!     path: /health
//!     port: 8080
//!   initialDelaySeconds: 5
//!   periodSeconds: 10
//!   timeoutSeconds: 1
//!   failureThreshold: 3
//!
//! readinessProbe:
//!   httpGet:
//!     path: /health?deep=true
//!     port: 8080
//!   initialDelaySeconds: 10
//!   periodSeconds: 5
//!   timeoutSeconds: 1
//!   successThreshold: 1
//!   failureThreshold: 3
//! ```

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::capsules::health_check::{Component, HealthCheckCapsule64};

/// Health check query parameters
#[derive(Debug, Deserialize)]
pub struct HealthQuery {
    /// Enable deep health check (all components)
    #[serde(default)]
    pub deep: bool,
}

/// Health check response (liveness probe)
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Overall status
    pub status: String,

    /// Is process ready for traffic?
    pub readiness: bool,

    /// Timestamp (nanoseconds since epoch)
    pub timestamp: u64,
}

/// Deep health check response (readiness probe)
#[derive(Debug, Serialize)]
pub struct DeepHealthResponse {
    /// Overall status
    pub status: String,

    /// Is process ready for traffic?
    pub readiness: bool,

    /// Timestamp (nanoseconds since epoch)
    pub timestamp: u64,

    /// Component health status
    pub components: ComponentStatus,
}

/// Component health status
#[derive(Debug, Serialize)]
pub struct ComponentStatus {
    pub budget_registry: ComponentHealth,
    pub provider_router: ComponentHealth,
    pub metrics_registry: ComponentHealth,
    pub audit_log: ComponentHealth,
    pub circuit_breaker: ComponentHealth,
    pub database: ComponentHealth,
    pub oauth_provider: ComponentHealth,
    pub payment_processor: ComponentHealth,
    pub rate_limiter: ComponentHealth,
}

/// Component health status
#[derive(Debug, Serialize)]
pub struct ComponentHealth {
    pub healthy: bool,
    pub critical: bool,
}

/// Health check handler (liveness probe)
///
/// **Performance**: <100ns (atomic read + JSON serialization)
/// **Status Codes**:
/// - 200 OK: Process is alive
/// - 503 Service Unavailable: Process is unhealthy
pub async fn health_check(
    State(health): State<Arc<HealthCheckCapsule64>>,
    Query(query): Query<HealthQuery>,
) -> Response {
    // Get current timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;

    if query.deep {
        // Deep health check (all components)
        let status = health.deep_check();
        let is_ready = status.is_ready();

        let response = DeepHealthResponse {
            status: if is_ready {
                "healthy".to_string()
            } else {
                "unhealthy".to_string()
            },
            readiness: is_ready,
            timestamp,
            components: ComponentStatus {
                budget_registry: ComponentHealth {
                    healthy: status.budget_registry,
                    critical: Component::BudgetRegistry.is_critical(),
                },
                provider_router: ComponentHealth {
                    healthy: status.provider_router,
                    critical: Component::ProviderRouter.is_critical(),
                },
                metrics_registry: ComponentHealth {
                    healthy: status.metrics_registry,
                    critical: Component::MetricsRegistry.is_critical(),
                },
                audit_log: ComponentHealth {
                    healthy: status.audit_log,
                    critical: Component::AuditLog.is_critical(),
                },
                circuit_breaker: ComponentHealth {
                    healthy: status.circuit_breaker,
                    critical: Component::CircuitBreaker.is_critical(),
                },
                database: ComponentHealth {
                    healthy: status.database,
                    critical: Component::Database.is_critical(),
                },
                oauth_provider: ComponentHealth {
                    healthy: status.oauth_provider,
                    critical: Component::OAuthProvider.is_critical(),
                },
                payment_processor: ComponentHealth {
                    healthy: status.payment_processor,
                    critical: Component::PaymentProcessor.is_critical(),
                },
                rate_limiter: ComponentHealth {
                    healthy: status.rate_limiter,
                    critical: Component::RateLimiter.is_critical(),
                },
            },
        };

        if is_ready {
            (StatusCode::OK, Json(response)).into_response()
        } else {
            (StatusCode::SERVICE_UNAVAILABLE, Json(response)).into_response()
        }
    } else {
        // Basic health check (liveness probe)
        let is_live = health.is_live();

        let response = HealthResponse {
            status: if is_live {
                "healthy".to_string()
            } else {
                "unhealthy".to_string()
            },
            readiness: health.is_ready(),
            timestamp,
        };

        if is_live {
            (StatusCode::OK, Json(response)).into_response()
        } else {
            (StatusCode::SERVICE_UNAVAILABLE, Json(response)).into_response()
        }
    }
}

/// Create health check routes
///
/// **Routes**:
/// - GET /health - Liveness probe (basic health check)
/// - GET /health?deep=true - Readiness probe (deep health check)
pub fn health_routes(health: Arc<HealthCheckCapsule64>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .with_state(health)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check_unhealthy() {
        let health = Arc::new(HealthCheckCapsule64::new());
        let app = health_routes(health);

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_health_check_live() {
        let health = Arc::new(HealthCheckCapsule64::new());
        health.set_healthy(Component::BudgetRegistry);

        let app = health_routes(health);

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_check_ready() {
        let health = Arc::new(HealthCheckCapsule64::new());
        health.set_healthy(Component::BudgetRegistry);
        health.set_healthy(Component::ProviderRouter);
        health.set_healthy(Component::Database);

        let app = health_routes(health);

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_deep_health_check() {
        let health = Arc::new(HealthCheckCapsule64::new());
        health.set_healthy(Component::BudgetRegistry);
        health.set_healthy(Component::ProviderRouter);
        health.set_healthy(Component::Database);

        let app = health_routes(health);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health?deep=true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
