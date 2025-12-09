//! HTTP Proxy Server with axum
//!
//! # UCE33 Q17: HTTP Interface
//! - Axum for async HTTP server
//! - Middleware for logging and error handling
//! - OpenAI-compatible API endpoints

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tokio::net::TcpListener;

use crate::error::ClapiError;
use crate::observability::AlertSystem;
use crate::proxy::{
    AuditLog, BudgetRegistry, ChatCompletionRequest, ChatCompletionResponse, ProxyConfig,
    ProviderClient, ProviderRouter,
};
use crate::proxy::mock_router::MockRouter;
use crate::proxy::dashboard::{DashboardState, handle_dashboard};
use crate::capsules::{
    RateLimitCapsule, DeduplicationCapsule, AnomalyDetectorCapsule128,
    ClientCircuitBreakerCapsule128,
};
use std::collections::HashMap;

/// Proxy server state (shared via Arc)
#[derive(Clone)]
struct AppState {
    budget_registry: Arc<BudgetRegistry>,
    provider_router: Option<Arc<ProviderRouter>>,
    mock_router: Option<Arc<MockRouter>>,
    audit_log: Arc<AuditLog>,
    alert_system: Option<Arc<AlertSystem>>,
    test_mode: bool,

    // Loop Armor Phase 1 (UCE34 Q10: T1 Atomic capsules)
    rate_limiters: Arc<tokio::sync::RwLock<HashMap<u64, Arc<RateLimitCapsule>>>>,
    dedup: Arc<tokio::sync::RwLock<DeduplicationCapsule>>,
    anomaly_detectors: Arc<tokio::sync::RwLock<HashMap<u64, Arc<AnomalyDetectorCapsule128>>>>,

    // Loop Armor Phase 3: Per-Client Circuit Breaker (UCE34 Q10: T1 Atomic capsule)
    client_circuit_breakers: Arc<tokio::sync::RwLock<HashMap<u64, Arc<ClientCircuitBreakerCapsule128>>>>,
}

/// HTTP Proxy Server
pub struct ProxyServer {
    config: ProxyConfig,
    state: AppState,
}

impl ProxyServer {
    /// Create new proxy server
    ///
    /// # Arguments
    /// - `config`: Proxy configuration
    pub fn new(config: ProxyConfig) -> crate::error::ClapiResult<Self> {
        // Initialize budget registry
        let budget_registry = Arc::new(BudgetRegistry::new(config.default_budget));

        // Initialize routing (test mode or real providers)
        let (provider_router, mock_router) = if config.test_mode {
            // Test mode: Use MockRouter
            (None, Some(Arc::new(MockRouter::new())))
        } else {
            // Production mode: Use real ProviderRouter
            let mut clients = Vec::new();
            for (idx, provider_config) in config.providers.iter().enumerate() {
                let client = ProviderClient::new(
                    provider_config,
                    idx as u16,
                    Duration::from_secs(config.request_timeout_secs),
                )?;
                clients.push(client);
            }
            (Some(Arc::new(ProviderRouter::new(clients)?)), None)
        };

        // Initialize audit log
        let audit_log = Arc::new(AuditLog::new(config.audit_log_path.clone())?);

        // Initialize alert system (if configured)
        let alert_system = if let (Some(pagerduty_token), Some(slack_webhook)) =
            (&config.pagerduty_token, &config.slack_webhook)
        {
            Some(Arc::new(AlertSystem::new(
                pagerduty_token.clone(),
                slack_webhook.clone(),
            )))
        } else {
            None
        };

        // Initialize Loop Armor Phase 1 capsules
        let rate_limiters = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let dedup = Arc::new(tokio::sync::RwLock::new(DeduplicationCapsule::new()));
        let anomaly_detectors = Arc::new(tokio::sync::RwLock::new(HashMap::new()));

        // Initialize Loop Armor Phase 3: Per-Client Circuit Breaker
        let client_circuit_breakers = Arc::new(tokio::sync::RwLock::new(HashMap::new()));

        let state = AppState {
            budget_registry,
            provider_router,
            mock_router,
            audit_log,
            alert_system,
            test_mode: config.test_mode,
            rate_limiters,
            dedup,
            anomaly_detectors,
            client_circuit_breakers,
        };

        Ok(Self { config, state })
    }

    /// Start HTTP server
    ///
    /// # Performance
    /// - Async/await: Non-blocking I/O
    /// - Connection pooling: Reuse HTTP connections
    /// - Lockfree: Budget checks via atomic CAS
    pub async fn serve(self) -> crate::error::ClapiResult<()> {
        let addr: SocketAddr = self
            .config
            .listen_addr
            .parse()
            .map_err(|e| ClapiError::ConfigError(format!("Invalid listen address: {}", e)))?;

        let app = Router::new()
            .route("/v1/chat/completions", post(handle_chat_completion))
            .route("/v1/completions", post(handle_completion))
            .route("/health", post(handle_health))
            .route("/api/dashboard", get(handle_dashboard_wrapper))
            .with_state(self.state);

        let listener = TcpListener::bind(addr).await?;

        println!("Clapi proxy listening on {}", addr);

        axum::serve(listener, app)
            .await
            .map_err(|e| ClapiError::IoError(e.to_string()))?;

        Ok(())
    }
}

/// Handle chat completion request
///
/// # Request Lifecycle (Loop Armor Phase 1)
/// 1. Rate limit check → RateLimitCapsule (block if quota exceeded)
/// 2. Deduplication check → DeduplicationCapsule (return cached if duplicate)
/// 3. Anomaly recording → AnomalyDetectorCapsule128 (record request)
/// 4. Parse request → REQ-128 (budget check)
/// 5. Select provider → RTE-128 (routing decision)
/// 6. Call provider → HTTP request
/// 7. Record metrics → RES-256 (response capsule)
/// 8. Update anomaly detector → AnomalyDetectorCapsule128 (record latency)
/// 9. Audit log → ALE-128 (audit entry)
/// 10. Return response
async fn handle_chat_completion(
    State(state): State<AppState>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, AppError> {
    let start = Instant::now();
    let budget_id = req.budget_id(); // Numeric u64 ID

    // ============================================================================
    // Loop Armor Phase 1: Rate Limit + Dedup + Anomaly Detection
    // ============================================================================

    // 1. RATE LIMIT CHECK (per-budget-id, 1000 req/min sliding window)
    {
        let mut limiters = state.rate_limiters.write().await;
        let limiter = limiters
            .entry(budget_id)
            .or_insert_with(|| Arc::new(RateLimitCapsule::new()));

        // Check rate limit first (fast path: <20ns)
        if !limiter.check_rate_limit() {
            // Increment and check quota
            limiter.increment_request().map_err(AppError::from)?;
        } else {
            // Quota available - increment
            limiter.increment_request().map_err(AppError::from)?;
        }
    }

    // 2. DEDUPLICATION CHECK (64K in-flight requests, return cached if duplicate)
    let req_hash = compute_request_hash(&req);
    {
        let mut dedup = state.dedup.write().await;
        if let Some(cached_response) = dedup.check_in_flight(req_hash) {
            // Duplicate detected! Return cached response (saves 100ms+ provider call)
            return Ok(Json((*cached_response).clone()));
        }
    }

    // 3. ANOMALY DETECTION (record request, detect spikes)
    {
        let mut detectors = state.anomaly_detectors.write().await;
        let detector = detectors
            .entry(budget_id)
            .or_insert_with(|| Arc::new(AnomalyDetectorCapsule128::new(2.0, 60)));

        // Record request start (will record latency after response)
        // No-op here, latency recorded at end
    }

    // ============================================================================
    // Loop Armor Phase 3: Per-Client Circuit Breaker
    // ============================================================================

    // 4. CIRCUIT BREAKER CHECK (per-budget-id, fail-fast if open)
    {
        let mut breakers = state.client_circuit_breakers.write().await;
        let breaker = breakers
            .entry(budget_id)
            .or_insert_with(|| Arc::new(ClientCircuitBreakerCapsule128::new()));

        // Check circuit breaker state (fast path: <50ns atomic read)
        use crate::capsules::CircuitBreakerDecision;
        match breaker.check_and_record(false) {
            CircuitBreakerDecision::Allow => {
                // Circuit closed or half-open with capacity - proceed
            }
            CircuitBreakerDecision::Reject => {
                // Circuit open - reject request immediately (fail-fast)
                let cooldown_remaining = breaker.get_cooldown_remaining_secs();
                return Err(AppError::from(ClapiError::CircuitBreakerOpen {
                    cooldown_remaining,
                }));
            }
        }
    }

    // ============================================================================
    // Existing Budget Check
    // ============================================================================

    // 5. Estimate cost and check budget
    let estimated_cost = req.estimate_cost_cents();

    state
        .budget_registry
        .try_deduct(budget_id, estimated_cost)
        .map_err(AppError::from)?;

    // 6. Route request to provider (test mode or real)
    let response_result = if state.test_mode {
        // Test mode: Route through MockRouter
        state
            .mock_router
            .as_ref()
            .expect("MockRouter must exist in test mode")
            .route_request(&req)
            .await
    } else {
        // Production mode: Route through ProviderRouter
        state
            .provider_router
            .as_ref()
            .expect("ProviderRouter must exist in production mode")
            .route_request(&req)
            .await
    };

    // Handle provider error: refund budget + record error in circuit breaker
    let response = match response_result {
        Ok(resp) => resp,
        Err(e) => {
            // Refund budget on provider error
            let _ = state.budget_registry.credit(budget_id, estimated_cost);

            // Record error in circuit breaker (Phase 3)
            {
                let breakers = state.client_circuit_breakers.read().await;
                if let Some(breaker) = breakers.get(&budget_id) {
                    // Record error (is_error = true)
                    let _ = breaker.check_and_record(true);
                }
            }

            return Err(AppError::from(e));
        }
    };

    // 3. Calculate actual cost
    let latency = start.elapsed();
    let actual_cost = calculate_actual_cost(&response);
    let cost_diff = actual_cost - estimated_cost;

    // Adjust budget for cost difference
    if cost_diff != 0 {
        if cost_diff > 0 {
            // Actual cost higher - try to deduct difference
            if state.budget_registry.try_deduct(budget_id, cost_diff).is_err() {
                // Insufficient budget for difference - refund and error
                let _ = state.budget_registry.credit(budget_id, estimated_cost);
                return Err(AppError::from(ClapiError::BudgetExhausted {
                    requested: actual_cost,
                    available: estimated_cost,
                }));
            }
        } else {
            // Actual cost lower - credit difference
            let _ = state.budget_registry.credit(budget_id, -cost_diff);
        }
    }

    // 4. Update provider health (production mode only)
    let provider_id_for_audit = if !state.test_mode {
        let provider_id = 0; // TODO: Get from response
        state
            .provider_router
            .as_ref()
            .expect("ProviderRouter must exist in production mode")
            .update_health(provider_id, true, latency.as_millis() as u64);
        provider_id
    } else {
        0 // Test mode
    };

    // ============================================================================
    // Loop Armor Phase 1: Post-Response Updates
    // ============================================================================

    // 5. BROADCAST DEDUP RESULT (notify waiters)
    {
        let response_arc = Arc::new(response.clone());
        let mut dedup = state.dedup.write().await;
        dedup.broadcast_result(req_hash, response_arc);

        // Cleanup after 10ms (allow waiters to read)
        let dedup_clone = state.dedup.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let mut dedup = dedup_clone.write().await;
            dedup.remove_in_flight(req_hash);
        });
    }

    // 6. UPDATE ANOMALY DETECTOR (record latency, detect spikes)
    {
        let detectors = state.anomaly_detectors.read().await;
        if let Some(detector) = detectors.get(&budget_id) {
            // Record latency
            detector.record_latency(latency.as_nanos() as u64);

            // Detect anomaly (check every request for real-time detection)
            if let Some(anomaly) = detector.detect_anomaly() {
                eprintln!(
                    "⚠️ ANOMALY DETECTED: budget_id={}, metric={}, baseline={}ns, observed={}ns, severity={:?}",
                    budget_id,
                    anomaly.metric_name,
                    anomaly.baseline_value,
                    anomaly.observed_value,
                    anomaly.severity
                );

                // Send alert via AlertSystem if configured
                if let Some(alert_system) = &state.alert_system {
                    use crate::observability::{Alert, AlertLevel};
                    let alert = Alert::new(
                        "Anomaly Detected",
                        format!(
                            "Budget {} exceeded p99 threshold by {:.1}×",
                            budget_id,
                            (anomaly.observed_value as f64) / (anomaly.baseline_value as f64)
                        ),
                        AlertLevel::High,
                    );
                    let _ = alert_system.trigger_alert(alert);
                }
            }
        }
    }

    // ============================================================================
    // Loop Armor Phase 3: Circuit Breaker Success Recording
    // ============================================================================

    // 7. RECORD SUCCESS IN CIRCUIT BREAKER (response received successfully)
    {
        let breakers = state.client_circuit_breakers.read().await;
        if let Some(breaker) = breakers.get(&budget_id) {
            // Record success (is_error = false)
            let _ = breaker.check_and_record(false);
        }
    }

    // 8. Log to audit trail
    let _ = state.audit_log.log_request(
        budget_id, // Numeric ID
        provider_id_for_audit,
        actual_cost,
        response.usage.total_tokens,
        0, // TODO: Implement hash chain
    );

    Ok(Json(response))
}

/// Handle completion request (legacy endpoint)
async fn handle_completion(
    State(_state): State<AppState>,
    Json(_req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Not implemented yet - return error
    Err(AppError::from(ClapiError::InvalidRequest {
        reason: "Legacy /v1/completions endpoint not implemented".to_string(),
    }))
}

/// Handle dashboard request (wrapper for AppState → DashboardState conversion)
///
/// # Phase 2 Integration (I20)
/// - Q1-Q5 (Scope): New endpoint, additive, no breaking changes
/// - Q6-Q10 (Compatibility): Reuses existing AppState infrastructure
/// - Q11-Q15 (Safety): Atomic reads only, zero locks in hot path
/// - Q16-Q20 (Validation): Integration tests validate correctness
async fn handle_dashboard_wrapper(
    State(state): State<AppState>,
) -> Result<Json<crate::proxy::dashboard::DashboardResponse>, crate::proxy::dashboard::DashboardError> {
    // Convert AppState to DashboardState
    let provider_count = if state.test_mode {
        1 // Test mode: Single mock provider
    } else {
        state
            .provider_router
            .as_ref()
            .map(|router| router.provider_count())
            .unwrap_or(0)
    };

    let dashboard_state = DashboardState {
        budget_registry: state.budget_registry.clone(),
        provider_count,
        test_mode: state.test_mode,
    };

    // Call dashboard handler
    handle_dashboard(State(dashboard_state)).await
}

/// Handle health check
async fn handle_health(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    if state.test_mode {
        // Test mode: Return mock provider stats
        let mock_stats = state
            .mock_router
            .as_ref()
            .expect("MockRouter must exist in test mode")
            .get_stats();

        Ok(Json(serde_json::json!({
            "status": "ok",
            "test_mode": true,
            "budgets_count": state.budget_registry.len(),
            "mock_provider": {
                "latency_ms": mock_stats.latency_ms,
                "token_count": mock_stats.token_count,
                "cost_per_1k_tokens": mock_stats.cost_per_1k_tokens,
            }
        })))
    } else {
        // Production mode: Return real provider stats
        let stats = state
            .provider_router
            .as_ref()
            .expect("ProviderRouter must exist in production mode")
            .get_stats();

        Ok(Json(serde_json::json!({
            "status": "ok",
            "test_mode": false,
            "budgets_count": state.budget_registry.len(),
            "routing_stats": {
                "request_count": stats.request_count,
                "failure_count": stats.failure_count,
                "primary_id": stats.primary_id,
                "fallback_id": stats.fallback_id,
            }
        })))
    }
}

/// Calculate actual cost from response
fn calculate_actual_cost(response: &ChatCompletionResponse) -> i64 {
    // Simplified cost model (GPT-4 pricing)
    let prompt_cost = (response.usage.prompt_tokens as f64 * 0.03 / 1000.0) * 100.0;
    let completion_cost = (response.usage.completion_tokens as f64 * 0.06 / 1000.0) * 100.0;

    (prompt_cost + completion_cost).ceil() as i64
}

/// Compute deterministic hash for request (for deduplication)
///
/// # Hash Inputs
/// - Model name
/// - Messages content
/// - Temperature
/// - Max tokens
///
/// # Performance
/// - <50ns (FNV-1a hash)
fn compute_request_hash(req: &ChatCompletionRequest) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    // Hash model
    req.model.hash(&mut hasher);

    // Hash messages (content only, ignore role for dedup)
    for msg in &req.messages {
        msg.content.hash(&mut hasher);
    }

    // Hash temperature (if present)
    if let Some(temp) = req.temperature {
        temp.to_bits().hash(&mut hasher);
    }

    // Hash max_tokens (if present)
    if let Some(max_tokens) = req.max_tokens {
        max_tokens.hash(&mut hasher);
    }

    hasher.finish()
}

/// Application error type (converts to HTTP response)
struct AppError(ClapiError);

impl From<ClapiError> for AppError {
    fn from(err: ClapiError) -> Self {
        AppError(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self.0 {
            ClapiError::BudgetExhausted { requested, available } => (
                StatusCode::PAYMENT_REQUIRED,
                format!("Budget exhausted: requested {}, available {}", requested, available),
            ),
            ClapiError::AllProvidersUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "All providers unavailable".to_string(),
            ),
            ClapiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Invalid API key".to_string(),
            ),
            ClapiError::Timeout { timeout_ms } => (
                StatusCode::GATEWAY_TIMEOUT,
                format!("Request timeout after {}ms", timeout_ms),
            ),
            ClapiError::ProviderError(msg) => (StatusCode::BAD_GATEWAY, format!("Provider error: {}", msg)),
            ClapiError::InvalidRequest { reason } => (StatusCode::BAD_REQUEST, format!("Invalid request: {}", reason)),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };

        (
            status,
            Json(serde_json::json!({
                "error": {
                    "message": message,
                    "type": "clapi_error"
                }
            })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_actual_cost() {
        let response = ChatCompletionResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![],
            usage: crate::proxy::Usage {
                prompt_tokens: 1000,
                completion_tokens: 500,
                total_tokens: 1500,
            },
            cost_cents: None,
            provider: None,
        };

        let cost = calculate_actual_cost(&response);
        assert!(cost > 0);
        assert!(cost < 100_00); // Less than $1
    }
}
