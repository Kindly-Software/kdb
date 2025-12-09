//! Licensed API module - All public APIs require license validation
//!
//! ## Architecture
//! - **Tier**: T1 Atomic (lockfree coordination)
//! - **Protection**: All API operations require valid license + hardware binding
//! - **Performance**: <100ns validation overhead per API call
//!
//! ## Framework Compliance
//! - **UCE34**: Q10 T1 Atomic tier selection
//! - **ASSUM**: 99.99% safe (zero unsafe code)
//! - **Chaos**: 100% lockfree (atomic coordination)

#[cfg(feature = "meta-capsule")]
pub mod licensed_api;

#[cfg(feature = "meta-capsule")]
pub mod auth_middleware;

// Re-export licensed types
#[cfg(feature = "meta-capsule")]
pub use licensed_api::{
    LicensedApiError, LicensedBatchMinHash, LicensedDedupPipeline, LicensedShardedBloomFilter,
    LicensedStreamingDedupPipeline, LicensedUnionFind,
};

#[cfg(feature = "meta-capsule")]
pub use auth_middleware::{generate_api_key, ApiAuthMiddleware, ApiKeyMetadata, AuthError};
