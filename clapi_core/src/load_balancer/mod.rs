//! Advanced Multi-Factor Load Balancer with SIMD-Accelerated Provider Selection
//!
//! # UCE34 Q10-Q12: Tier 6 Mixed Capsule (T1 Atomic + T2 SIMD f32x8)
//! - **Tier 1 (Atomic)**: Lockfree quota tracking, circuit breaker integration
//! - **Tier 2 (SIMD)**: f32x8 parallel scoring for 8 providers
//! - **Target Performance**: <500ns provider selection, 10-20% cost reduction
//! - **Compound Speedup**: 12× potential (3× atomic + 4× SIMD)
//!
//! # Architecture
//! - `ProviderScoreCapsule` (256B): SIMD-aligned capsule for 8 providers
//! - Multi-factor scoring: latency (p50) + cost optimization
//! - Circuit breaker integration: Skip providers with Open circuits
//! - Quota enforcement: Per-provider atomic quota tracking
//! - Nightly feature: `portable_simd` (optional, stable fallback)
//!
//! # Performance Targets (B32 Framework)
//! - Provider selection: <500ns (SIMD f32x8 vs scalar)
//! - Cost reduction: 10-20% via dynamic routing
//! - Latency optimization: 10-20% reduction
//! - Fairness: Even distribution across healthy providers
//!
//! # Safety (ASSUM Framework)
//! - #ASSUME: AtomicU64 quota operations are lockfree
//! - #VERIFY: Property tests validate quota consistency (1000 threads)
//! - #ASSUME: f32x8 SIMD requires 32B alignment
//! - #VERIFY: verify_simd_capsule! enforces alignment at compile-time
//! - #ASSUME: Circuit breaker state loads are eventually consistent
//! - #VERIFY: Integration tests validate failover semantics
//!
//! # Integration
//! - Works with existing `ProviderCircuitArray` (Week 2)
//! - Respects circuit breaker state (Closed/HalfOpen/Open)
//! - Updates latency metrics automatically
//! - Zero breaking changes to existing API

pub mod capsule;
pub mod scoring;

use std::sync::Arc;
use std::time::Instant;

use crate::capsules::ProviderCircuitArray;
use crate::error::{ClapiError, ClapiResult};

pub use capsule::ProviderScoreCapsule;
pub use scoring::{LoadBalancer, ScoringWeights};

/// Provider selection result
#[derive(Debug, Clone, Copy)]
pub struct ProviderSelection {
    /// Selected provider ID (0-7)
    pub provider_id: u8,
    /// Provider score (higher = better)
    pub score: f32,
    /// Selection latency (nanoseconds)
    pub selection_latency_ns: u64,
}

/// Load balancing statistics
#[derive(Debug, Clone, Default)]
pub struct LoadBalancingStats {
    /// Total requests routed
    pub total_requests: u64,
    /// Requests per provider
    pub requests_per_provider: [u64; 8],
    /// Average selection latency (nanoseconds)
    pub avg_selection_latency_ns: u64,
    /// Cost savings (basis points, 10000 = 100%)
    pub cost_savings_bp: u64,
}

/// Create load balancer with default weights
///
/// # Arguments
/// - `circuit_array`: Existing circuit breaker array (from Week 2)
///
/// # Returns
/// - Configured load balancer with latency-optimized weights
///
/// # Example
/// ```no_run
/// use clapi_core::load_balancer::create_default_balancer;
/// use clapi_core::capsules::ProviderCircuitArray;
/// use std::sync::Arc;
///
/// let circuits = Arc::new(ProviderCircuitArray::new());
/// let balancer = create_default_balancer(circuits);
/// ```
pub fn create_default_balancer(
    circuit_array: Arc<ProviderCircuitArray>,
) -> LoadBalancer {
    // Default: Optimize for latency (70%) with cost awareness (30%)
    let weights = ScoringWeights {
        latency_weight: 0.7,
        cost_weight: 0.3,
    };

    LoadBalancer::new(circuit_array, weights)
}

/// Create cost-optimized load balancer
///
/// # Arguments
/// - `circuit_array`: Existing circuit breaker array
///
/// # Returns
/// - Configured load balancer with cost-optimized weights
///
/// # Example
/// ```no_run
/// use clapi_core::load_balancer::create_cost_optimized_balancer;
/// use clapi_core::capsules::ProviderCircuitArray;
/// use std::sync::Arc;
///
/// let circuits = Arc::new(ProviderCircuitArray::new());
/// let balancer = create_cost_optimized_balancer(circuits);
/// ```
pub fn create_cost_optimized_balancer(
    circuit_array: Arc<ProviderCircuitArray>,
) -> LoadBalancer {
    // Cost-optimized: Prioritize cost (60%) with latency fallback (40%)
    let weights = ScoringWeights {
        latency_weight: 0.4,
        cost_weight: 0.6,
    };

    LoadBalancer::new(circuit_array, weights)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_balancer() {
        let circuits = Arc::new(ProviderCircuitArray::new());
        let balancer = create_default_balancer(circuits);
        let weights = balancer.weights();

        assert_eq!(weights.latency_weight, 0.7);
        assert_eq!(weights.cost_weight, 0.3);
    }

    #[test]
    fn test_create_cost_optimized_balancer() {
        let circuits = Arc::new(ProviderCircuitArray::new());
        let balancer = create_cost_optimized_balancer(circuits);
        let weights = balancer.weights();

        assert_eq!(weights.latency_weight, 0.4);
        assert_eq!(weights.cost_weight, 0.6);
    }

    #[test]
    fn test_provider_selection_debug() {
        let selection = ProviderSelection {
            provider_id: 3,
            score: 0.85,
            selection_latency_ns: 450,
        };

        let debug_str = format!("{:?}", selection);
        assert!(debug_str.contains("provider_id: 3"));
        assert!(debug_str.contains("score: 0.85"));
    }

    #[test]
    fn test_load_balancing_stats_default() {
        let stats = LoadBalancingStats::default();

        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.avg_selection_latency_ns, 0);
        assert_eq!(stats.cost_savings_bp, 0);

        for count in stats.requests_per_provider.iter() {
            assert_eq!(*count, 0);
        }
    }
}
