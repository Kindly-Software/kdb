//! Provider router with health-aware selection
//!
//! # UCE33 Q15: State Machine
//! - RoutingCapsule128 for provider selection
//! - Health tracking with circuit breaker
//! - Automatic failover to backup providers

use std::sync::Arc;

use crate::capsules::{ProviderState, RoutingCapsule128};
use crate::error::{ClapiError, ClapiResult};
use crate::proxy::{ChatCompletionRequest, ChatCompletionResponse, ProviderClient};

/// Provider router with health-aware selection
///
/// # Safety
/// - #ASSUME: RoutingCapsule128 provides lockfree provider selection
/// - #VERIFY: No Mutex in routing hot path
pub struct ProviderRouter {
    /// Routing capsule for provider selection
    routing_capsule: Arc<RoutingCapsule128>,

    /// Provider clients (indexed by provider_id)
    clients: Vec<ProviderClient>,
}

impl ProviderRouter {
    /// Create new provider router
    ///
    /// # Arguments
    /// - `clients`: Provider clients (must have at least 1)
    pub fn new(clients: Vec<ProviderClient>) -> ClapiResult<Self> {
        if clients.is_empty() {
            return Err(ClapiError::AllProvidersUnavailable);
        }

        // Create routing capsule with primary and fallback
        let primary_id = 0;
        let fallback_id = if clients.len() > 1 { 1 } else { 0 };

        let routing_capsule = Arc::new(RoutingCapsule128::new(primary_id, fallback_id));

        Ok(Self {
            routing_capsule,
            clients,
        })
    }

    /// Route request to provider (lockfree, <80ns)
    ///
    /// # Returns
    /// - Provider ID for routing
    /// - Generation counter for TOCTOU prevention
    ///
    /// # Performance
    /// - Fast path: <80ns (lockfree routing decision)
    /// - Automatic failover if primary unavailable
    pub async fn route_request(
        &self,
        req: &ChatCompletionRequest,
    ) -> ClapiResult<ChatCompletionResponse> {
        // Select provider (lockfree)
        let (provider_id, _generation) = self.routing_capsule.select_provider()?;

        // Get client
        let client = self
            .clients
            .get(provider_id as usize)
            .ok_or(ClapiError::InvalidProviderId(provider_id))?;

        // Execute request with timeout
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            client.chat_completion(req),
        )
        .await
        .map_err(|_| ClapiError::Timeout { timeout_ms: 30000 })??;

        Ok(response)
    }

    /// Update provider health
    ///
    /// Called after each request to track provider health.
    ///
    /// # Arguments
    /// - `provider_id`: Provider to update
    /// - `success`: Whether request succeeded
    /// - `latency_ms`: Request latency (milliseconds)
    pub fn update_health(&self, provider_id: u16, success: bool, latency_ms: u64) {
        let state = if success {
            if latency_ms < 1000 {
                ProviderState::Healthy
            } else if latency_ms < 5000 {
                ProviderState::Degraded
            } else {
                ProviderState::Unavailable
            }
        } else {
            ProviderState::CircuitOpen
        };

        let latency_p99 = (latency_ms as u16).min(16383); // 14-bit max
        self.routing_capsule
            .update_state(provider_id, state, latency_p99);
    }

    /// Get routing statistics
    pub fn get_stats(&self) -> RoutingStats {
        RoutingStats {
            request_count: self.routing_capsule.request_count(),
            failure_count: self.routing_capsule.failure_count(),
            primary_id: self.routing_capsule.get_primary_id(),
            fallback_id: self.routing_capsule.get_fallback_id(),
        }
    }

    /// Get number of registered providers
    ///
    /// # Performance
    /// - O(1): Returns cached count
    #[inline]
    pub fn provider_count(&self) -> u32 {
        self.clients.len() as u32
    }
}

/// Routing statistics
#[derive(Debug, Clone)]
pub struct RoutingStats {
    /// Total requests routed
    pub request_count: u64,
    /// Total failures
    pub failure_count: u64,
    /// Primary provider ID
    pub primary_id: u16,
    /// Fallback provider ID
    pub fallback_id: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::ProviderConfig;
    use std::time::Duration;

    fn create_test_client(id: u16) -> ProviderClient {
        let config = ProviderConfig {
            name: format!("provider{}", id),
            base_url: "https://api.test.com".to_string(),
            api_key: "test_key".to_string(),
            priority: id as u8,
            models: vec![],
        };

        ProviderClient::new(&config, id, Duration::from_secs(30)).unwrap()
    }

    #[test]
    fn test_new() {
        let clients = vec![create_test_client(0), create_test_client(1)];
        let router = ProviderRouter::new(clients);
        assert!(router.is_ok());
    }

    #[test]
    fn test_new_empty_clients() {
        let router = ProviderRouter::new(vec![]);
        assert!(router.is_err());
    }

    #[test]
    fn test_update_health() {
        let clients = vec![create_test_client(0), create_test_client(1)];
        let router = ProviderRouter::new(clients).unwrap();

        router.update_health(0, true, 500); // Healthy
        router.update_health(1, false, 0); // Circuit open

        let stats = router.get_stats();
        assert_eq!(stats.primary_id, 0);
        assert_eq!(stats.fallback_id, 1);
    }

    #[test]
    fn test_get_stats() {
        let clients = vec![create_test_client(0)];
        let router = ProviderRouter::new(clients).unwrap();

        let stats = router.get_stats();
        assert_eq!(stats.request_count, 0);
        assert_eq!(stats.failure_count, 0);
    }
}
