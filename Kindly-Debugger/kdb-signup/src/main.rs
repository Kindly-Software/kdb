//! KDB Signup Service - Main Entry Point
//!
//! Axum server for KDB Hobby Tier signups.
//! Listens on 0.0.0.0:8091 with CORS enabled for kindly.software.
//!
//! # Configuration (Environment Variables)
//!
//! - `PORT` - HTTP port (default: 8091)
//! - `SIGNING_KEY_PATH` - Path to Ed25519 private key (default: /etc/kdb/signing.key)
//! - `VERIFICATION_BASE_URL` - Base URL for verification links (default: https://api.kindly.software)
//! - `FROM_EMAIL` - Sender email address (default: noreply@kindly.software)
//!
//! # Framework Compliance
//!
//! - UCE34: Q10 T1 Atomic tier, Q33 lockfree
//! - Chaos: Arc<AppState> for shared state, no mutex in router setup
//! - T28: Graceful shutdown, proper error handling

use std::sync::Arc;

use axum::{routing::get, Router};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use kdb_signup::metrics::{describe_metrics, init_prometheus, update_prometheus_metrics};
use kdb_signup::routes::{self, signup_router, AppState};
use kdb_signup::{SERVICE_NAME, VERSION};

/// Server configuration
const LISTEN_ADDR: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 8091;
const DEFAULT_SIGNING_KEY_PATH: &str = "/etc/kdb/signing.key";
const DEFAULT_BASE_URL: &str = "https://api.kindly.software";
const DEFAULT_FROM_EMAIL: &str = "noreply@kindly.software";

/// Load Ed25519 signing key from file
///
/// Reads 32-byte key from path specified by SIGNING_KEY_PATH env var
/// or falls back to default path.
///
/// # Errors
/// - File not found or unreadable
/// - Invalid key length (must be exactly 32 bytes)
fn load_signing_key() -> anyhow::Result<[u8; 32]> {
    let path = std::env::var("SIGNING_KEY_PATH")
        .unwrap_or_else(|_| DEFAULT_SIGNING_KEY_PATH.to_string());

    tracing::info!(path = %path, "Loading Ed25519 signing key");

    let key_bytes = std::fs::read(&path)
        .map_err(|e| anyhow::anyhow!("Failed to read signing key from {}: {}", path, e))?;

    if key_bytes.len() != 32 {
        return Err(anyhow::anyhow!(
            "Invalid signing key length: expected 32 bytes, got {}",
            key_bytes.len()
        ));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&key_bytes);

    tracing::info!("Signing key loaded successfully");
    Ok(key)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing subscriber
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kdb_signup=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!(
        service = SERVICE_NAME,
        version = VERSION,
        "Starting KDB Signup Service"
    );

    // Initialize Prometheus metrics (must be before any metrics are recorded)
    let prometheus_handle = init_prometheus();
    describe_metrics();
    tracing::info!("Prometheus metrics initialized");

    // Load signing key
    let signing_key = load_signing_key()?;

    // Load configuration from environment
    let base_url = std::env::var("VERIFICATION_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
    let from_email = std::env::var("FROM_EMAIL")
        .unwrap_or_else(|_| DEFAULT_FROM_EMAIL.to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    tracing::info!(
        base_url = %base_url,
        from_email = %from_email,
        port = %port,
        "Configuration loaded"
    );

    // Create AppState with all capsules (UCE34: Arc for shared state, Chaos: no mutex)
    let state = Arc::new(AppState::new(signing_key, base_url, from_email));

    tracing::info!(
        registration_gen = state.registration.generation(),
        verification_gen = state.verification.generation(),
        license_gen_gen = state.license_gen.generation(),
        "AppState initialized with T1 Atomic capsules"
    );

    // Build CORS layer
    // Production: kindly.software, kindly.services (all variants)
    // Development: localhost:3000, 127.0.0.1:3000, localhost:8080
    let cors = CorsLayer::new()
        .allow_origin([
            "https://kindly.software".parse().expect("valid origin"),
            "https://www.kindly.software".parse().expect("valid origin"),
            "https://kindly.services".parse().expect("valid origin"),
            "https://www.kindly.services".parse().expect("valid origin"),
            "http://localhost:3000".parse().expect("valid origin"),
            "http://127.0.0.1:3000".parse().expect("valid origin"),
            "http://localhost:8080".parse().expect("valid origin"),
        ])
        .allow_methods(Any)
        .allow_headers(Any);

    // Create metrics handler closure that captures state and prometheus_handle
    let state_for_metrics = Arc::clone(&state);
    let metrics_handler = move || {
        let state = Arc::clone(&state_for_metrics);
        let handle = prometheus_handle.clone();
        async move {
            // Update metrics from capsule state
            update_prometheus_metrics(&state);

            // Render Prometheus format
            let metrics = handle.render();

            (
                axum::http::StatusCode::OK,
                [(
                    axum::http::header::CONTENT_TYPE,
                    "text/plain; charset=utf-8",
                )],
                metrics,
            )
        }
    };

    // Build router with signup routes and metrics
    let app = Router::new()
        .route("/health", get(routes::health_check))
        .route("/metrics", get(metrics_handler))
        .merge(signup_router())
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    // Bind listener
    let addr = SocketAddr::from((
        LISTEN_ADDR.parse::<std::net::IpAddr>().expect("valid IP"),
        port,
    ));
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(
        address = %addr,
        endpoints = "/health, /metrics, /api/v1/signup, /api/v1/verify/:token, /api/v1/resend-verification",
        "Server listening"
    );

    // Serve with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, initiating graceful shutdown");
        }
        _ = terminate => {
            tracing::info!("Received SIGTERM, initiating graceful shutdown");
        }
    }
}
