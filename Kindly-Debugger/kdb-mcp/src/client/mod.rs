//! MCP Client Module
//!
//! Feature-rich stdio->HTTP MCP client with:
//! - Response caching (LRU + TTL)
//! - Retry with exponential backoff
//! - Circuit breaker fault tolerance
//! - Request deduplication
//! - Request batching
//! - Offline queue
//! - Q34 audit trail
//! - 18-layer protection

// Phase 1: Essential Resilience
pub mod metrics;
#[cfg(feature = "client-retry")]
pub mod retry_integration;
#[cfg(feature = "client-circuit-breaker")]
pub mod circuit_breaker_integration;

// Phase 2: Performance
#[cfg(feature = "client-cache")]
pub mod response_cache;
#[cfg(feature = "client-dedup")]
pub mod idempotency;

// Phase 3: Advanced Resilience
#[cfg(feature = "client-offline")]
pub mod offline_queue;
#[cfg(feature = "client-batching")]
pub mod request_batcher;

// Phase 4: Protection
#[cfg(feature = "client-protection")]
pub mod protection_integration;
#[cfg(feature = "client-protection")]
pub mod self_destruct_handler;

// Re-exports
pub use metrics::McpMetricsCapsule;

#[cfg(feature = "client-retry")]
pub use retry_integration::{
    BackoffStrategy, MutableRetryConfig, RetryPolicy, is_retryable_error, retry_http_request,
};

#[cfg(feature = "client-circuit-breaker")]
pub use circuit_breaker_integration::{
    CircuitBreaker, CircuitBreakerError, CircuitBreakerState, MutableCircuitBreaker,
};

#[cfg(feature = "client-cache")]
pub use response_cache::{
    MutableResponseCache, ResponseCacheConfig, cache_key_for_request,
    CacheSlot, LockfreeCacheCapsule, const_fast_hash,
};

#[cfg(feature = "client-dedup")]
pub use idempotency::{
    IdempotencyCacheCapsule, IdempotencyStats, hash_request, fnv1a_hash,
};

#[cfg(feature = "client-offline")]
pub use offline_queue::{
    OfflineQueueCapsule, OfflineError, OverflowPolicy, QueuedRequest, QueueStats,
};

#[cfg(feature = "client-batching")]
pub use request_batcher::{
    RequestBatcherCapsule, BatchableRequest, BatcherError, BatcherStats,
};

#[cfg(feature = "client-protection")]
pub use protection_integration::{
    P0ProtectionLayer, ProtectionError, ProtectionStats,
};

#[cfg(feature = "client-protection")]
pub use self_destruct_handler::{
    SelfDestructHandler, TamperReason, cascade_level_for_priority, should_cascade,
};
