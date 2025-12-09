//! Health Check Capsule (P3-E7)
//!
//! **Tier**: T1 (Atomic)
//! **Purpose**: Multi-component health status for Kubernetes liveness/readiness probes
//! **Performance**: <20ns read, <50ns write (atomic bitmap operations)
//!
//! ## Architecture (UCE34 Q10-Q12)
//!
//! ### Q10: Tier 1 Atomic Capsule
//! - Atomic u64 bitmap (1 bit per component, 64 components max)
//! - Lockfree coordination (no mutex/RwLock)
//! - Cache-aligned (64B, single cache line)
//!
//! ### Q11: Rust Implementation
//! - AtomicU64 for bitmap storage
//! - Bitwise operations for component checks
//! - #[repr(C, align(64))] for cache alignment
//!
//! ### Q12: Nightly Enhancement
//! - const fn for compile-time component registration
//! - atomic_from_mut for zero-copy initialization (future)
//!
//! ## Component Bitmap Layout
//!
//! ```
//! Bit 0: BudgetRegistry        (critical)
//! Bit 1: ProviderRouter        (critical)
//! Bit 2: MetricsRegistry       (important)
//! Bit 3: AuditLog              (important)
//! Bit 4: CircuitBreaker        (important)
//! Bit 5: Database              (critical, if enabled)
//! Bit 6-63: Reserved for future components
//! ```
//!
//! ## Usage
//!
//! ```rust
//! use clapi_core::capsules::health_check::{HealthCheckCapsule64, Component};
//!
//! // Initialize capsule
//! let health = HealthCheckCapsule64::new();
//!
//! // Mark components healthy
//! health.set_healthy(Component::BudgetRegistry);
//! health.set_healthy(Component::ProviderRouter);
//!
//! // Check readiness (all critical components healthy)
//! assert!(health.is_ready());
//!
//! // Check liveness (process responsive)
//! assert!(health.is_live());
//!
//! // Deep health check (individual components)
//! let status = health.deep_check();
//! assert_eq!(status.budget_registry, true);
//! assert_eq!(status.provider_router, true);
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
//!
//! readinessProbe:
//!   httpGet:
//!     path: /health?deep=true
//!     port: 8080
//!   initialDelaySeconds: 10
//!   periodSeconds: 5
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use atomic_capsule::verify_capsule_properties;

/// Component health status bits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Component {
    BudgetRegistry = 0,
    ProviderRouter = 1,
    MetricsRegistry = 2,
    AuditLog = 3,
    CircuitBreaker = 4,
    Database = 5,
    OAuthProvider = 6,
    PaymentProcessor = 7,
    RateLimiter = 8,
}

impl Component {
    /// Get bitmask for this component
    #[inline(always)]
    pub const fn mask(self) -> u64 {
        1u64 << (self as u8)
    }

    /// Get all component names
    pub const fn all() -> &'static [Component] {
        &[
            Component::BudgetRegistry,
            Component::ProviderRouter,
            Component::MetricsRegistry,
            Component::AuditLog,
            Component::CircuitBreaker,
            Component::Database,
            Component::OAuthProvider,
            Component::PaymentProcessor,
            Component::RateLimiter,
        ]
    }

    /// Get component name as string
    pub const fn name(self) -> &'static str {
        match self {
            Component::BudgetRegistry => "budget_registry",
            Component::ProviderRouter => "provider_router",
            Component::MetricsRegistry => "metrics_registry",
            Component::AuditLog => "audit_log",
            Component::CircuitBreaker => "circuit_breaker",
            Component::Database => "database",
            Component::OAuthProvider => "oauth_provider",
            Component::PaymentProcessor => "payment_processor",
            Component::RateLimiter => "rate_limiter",
        }
    }

    /// Is this a critical component? (required for readiness)
    pub const fn is_critical(self) -> bool {
        matches!(
            self,
            Component::BudgetRegistry | Component::ProviderRouter | Component::Database
        )
    }
}

/// Health status for deep check
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthStatus {
    pub budget_registry: bool,
    pub provider_router: bool,
    pub metrics_registry: bool,
    pub audit_log: bool,
    pub circuit_breaker: bool,
    pub database: bool,
    pub oauth_provider: bool,
    pub payment_processor: bool,
    pub rate_limiter: bool,
}

impl HealthStatus {
    /// Check if all critical components are healthy
    pub fn is_ready(&self) -> bool {
        self.budget_registry && self.provider_router && self.database
    }

    /// Check if process is alive (any component healthy)
    pub fn is_live(&self) -> bool {
        self.budget_registry
            || self.provider_router
            || self.metrics_registry
            || self.audit_log
            || self.circuit_breaker
    }
}

/// Health Check Capsule (T1 Atomic)
///
/// **UCE34 Q10**: Tier 1 Atomic Capsule (lockfree coordination)
/// **UCE34 Q24**: 64B cache-aligned, single cache line
/// **UCE34 Q33**: Compile-time verification required
///
/// **ASSUM Safety**:
/// - #ASSUME: Atomic operations use Relaxed ordering (no synchronization needed)
/// - #VERIFY: Health status is informational only, no critical decisions
/// - #ASSUME: Bitmap fits in u64 (64 components max)
/// - #VERIFY: Component enum has ≤64 variants
#[repr(C, align(64))]
pub struct HealthCheckCapsule64 {
    /// Health bitmap: 1 bit per component (1 = healthy, 0 = unhealthy)
    ///
    /// **Memory Ordering**: Relaxed
    /// - Reads: Ordering::Relaxed (no synchronization)
    /// - Writes: Ordering::Relaxed (informational only)
    status: AtomicU64,

    /// Padding to complete 64-byte cache line
    _padding: [u8; 56],
}

// UCE34 Q33: Compile-time verification (MANDATORY)
verify_capsule_properties!(HealthCheckCapsule64, 64, 64);

impl HealthCheckCapsule64 {
    /// Create new health check capsule (all components unhealthy initially)
    ///
    /// **Performance**: <1ns (const initialization)
    pub const fn new() -> Self {
        Self {
            status: AtomicU64::new(0),
            _padding: [0u8; 56],
        }
    }

    /// Create new health check capsule (all components healthy initially)
    ///
    /// **Performance**: <1ns (const initialization)
    pub const fn new_all_healthy() -> Self {
        Self {
            status: AtomicU64::new(u64::MAX),
            _padding: [0u8; 56],
        }
    }

    /// Mark component as healthy
    ///
    /// **Performance**: <20ns (atomic OR operation)
    /// **ASSUM**: #ASSUME Relaxed ordering sufficient (informational)
    #[inline(always)]
    pub fn set_healthy(&self, component: Component) {
        // #ASSUME: Relaxed ordering sufficient for health status
        // #VERIFY: Health checks are informational, no critical decisions
        self.status.fetch_or(component.mask(), Ordering::Relaxed);
    }

    /// Mark component as unhealthy
    ///
    /// **Performance**: <20ns (atomic AND operation)
    /// **ASSUM**: #ASSUME Relaxed ordering sufficient (informational)
    #[inline(always)]
    pub fn set_unhealthy(&self, component: Component) {
        // #ASSUME: Relaxed ordering sufficient for health status
        // #VERIFY: Health checks are informational, no critical decisions
        self.status
            .fetch_and(!component.mask(), Ordering::Relaxed);
    }

    /// Check if component is healthy
    ///
    /// **Performance**: <10ns (single atomic load + bitwise AND)
    /// **ASSUM**: #ASSUME Relaxed ordering sufficient (informational)
    #[inline(always)]
    pub fn is_healthy(&self, component: Component) -> bool {
        // #ASSUME: Relaxed ordering sufficient for health status
        // #VERIFY: Health checks are informational, no critical decisions
        let status = self.status.load(Ordering::Relaxed);
        (status & component.mask()) != 0
    }

    /// Check if process is ready for traffic (all critical components healthy)
    ///
    /// **Performance**: <10ns (single atomic load + bitwise check)
    /// **Kubernetes**: Use for readiness probe
    #[inline(always)]
    pub fn is_ready(&self) -> bool {
        let status = self.status.load(Ordering::Relaxed);

        // Check all critical components
        let critical_mask = Component::BudgetRegistry.mask()
            | Component::ProviderRouter.mask()
            | Component::Database.mask();

        (status & critical_mask) == critical_mask
    }

    /// Check if process is alive (basic health check)
    ///
    /// **Performance**: <10ns (single atomic load + comparison)
    /// **Kubernetes**: Use for liveness probe
    #[inline(always)]
    pub fn is_live(&self) -> bool {
        // Process is alive if ANY component is healthy
        self.status.load(Ordering::Relaxed) != 0
    }

    /// Deep health check (all components)
    ///
    /// **Performance**: <20ns (single atomic load + multiple bitwise checks)
    /// **HTTP**: Use for /health?deep=true endpoint
    pub fn deep_check(&self) -> HealthStatus {
        let status = self.status.load(Ordering::Relaxed);

        HealthStatus {
            budget_registry: (status & Component::BudgetRegistry.mask()) != 0,
            provider_router: (status & Component::ProviderRouter.mask()) != 0,
            metrics_registry: (status & Component::MetricsRegistry.mask()) != 0,
            audit_log: (status & Component::AuditLog.mask()) != 0,
            circuit_breaker: (status & Component::CircuitBreaker.mask()) != 0,
            database: (status & Component::Database.mask()) != 0,
            oauth_provider: (status & Component::OAuthProvider.mask()) != 0,
            payment_processor: (status & Component::PaymentProcessor.mask()) != 0,
            rate_limiter: (status & Component::RateLimiter.mask()) != 0,
        }
    }

    /// Get raw bitmap status (for debugging)
    ///
    /// **Performance**: <5ns (single atomic load)
    #[inline(always)]
    pub fn raw_status(&self) -> u64 {
        self.status.load(Ordering::Relaxed)
    }

    /// Reset all components to unhealthy
    ///
    /// **Performance**: <10ns (single atomic store)
    /// **Use case**: Testing, graceful shutdown
    pub fn reset(&self) {
        self.status.store(0, Ordering::Relaxed);
    }
}

impl Default for HealthCheckCapsule64 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_all_unhealthy() {
        let health = HealthCheckCapsule64::new();
        assert_eq!(health.raw_status(), 0);
        assert!(!health.is_live());
        assert!(!health.is_ready());
    }

    #[test]
    fn test_new_all_healthy() {
        let health = HealthCheckCapsule64::new_all_healthy();
        assert_eq!(health.raw_status(), u64::MAX);
        assert!(health.is_live());
        assert!(health.is_ready());
    }

    #[test]
    fn test_set_healthy() {
        let health = HealthCheckCapsule64::new();
        health.set_healthy(Component::BudgetRegistry);
        assert!(health.is_healthy(Component::BudgetRegistry));
        assert!(!health.is_healthy(Component::ProviderRouter));
    }

    #[test]
    fn test_set_unhealthy() {
        let health = HealthCheckCapsule64::new_all_healthy();
        health.set_unhealthy(Component::BudgetRegistry);
        assert!(!health.is_healthy(Component::BudgetRegistry));
        assert!(health.is_healthy(Component::ProviderRouter));
    }

    #[test]
    fn test_is_ready() {
        let health = HealthCheckCapsule64::new();

        // Not ready with only BudgetRegistry
        health.set_healthy(Component::BudgetRegistry);
        assert!(!health.is_ready());

        // Not ready with BudgetRegistry + ProviderRouter (missing Database)
        health.set_healthy(Component::ProviderRouter);
        assert!(!health.is_ready());

        // Ready with all critical components
        health.set_healthy(Component::Database);
        assert!(health.is_ready());
    }

    #[test]
    fn test_is_live() {
        let health = HealthCheckCapsule64::new();

        // Not live initially
        assert!(!health.is_live());

        // Live with any component healthy
        health.set_healthy(Component::BudgetRegistry);
        assert!(health.is_live());
    }

    #[test]
    fn test_deep_check() {
        let health = HealthCheckCapsule64::new();
        health.set_healthy(Component::BudgetRegistry);
        health.set_healthy(Component::ProviderRouter);

        let status = health.deep_check();
        assert_eq!(status.budget_registry, true);
        assert_eq!(status.provider_router, true);
        assert_eq!(status.metrics_registry, false);
        assert_eq!(status.audit_log, false);
    }

    #[test]
    fn test_reset() {
        let health = HealthCheckCapsule64::new_all_healthy();
        assert!(health.is_live());

        health.reset();
        assert!(!health.is_live());
        assert_eq!(health.raw_status(), 0);
    }

    #[test]
    fn test_component_mask() {
        assert_eq!(Component::BudgetRegistry.mask(), 0b1);
        assert_eq!(Component::ProviderRouter.mask(), 0b10);
        assert_eq!(Component::MetricsRegistry.mask(), 0b100);
    }

    #[test]
    fn test_component_names() {
        assert_eq!(Component::BudgetRegistry.name(), "budget_registry");
        assert_eq!(Component::ProviderRouter.name(), "provider_router");
    }

    #[test]
    fn test_critical_components() {
        assert!(Component::BudgetRegistry.is_critical());
        assert!(Component::ProviderRouter.is_critical());
        assert!(Component::Database.is_critical());
        assert!(!Component::MetricsRegistry.is_critical());
    }
}
