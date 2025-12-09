//! Atomic Portfolio Map (APM-1024)
//!
//! This crate provides helpers to build and consume a packed 1024-bit snapshot
//! that describes portfolio-wide headroom and per-symbol risk affordances.
//!
//! ## Atomic Breaker Integration
//!
//! This crate integrates with the `atomic_breaker` primitive to provide unified
//! risk management across portfolio snapshots and real-time circuit breaking.
//!
//! ### Key Features
//!
//! - **Unified Breaker Levels**: Portfolio breaker levels are automatically
//!   synchronized with atomic circuit breaker levels (L0-L3)
//! - **State-Aware Flags**: Portfolio flags automatically reflect breaker state
//!   (PAUSED flag when breaker is Open/ForcedOpen)
//! - **Lockfree Operations**: All breaker operations maintain lockfree guarantees
//! - **Backward Compatibility**: Existing portfolio functionality is preserved
//!
//! ### Usage Example
//!
//! ```rust
//! use atomic_portfolio_map::{ApmSlot, BreakerLevel};
//!
//! // Create slot with specific breaker level
//! let slot = ApmSlot::new_with_breaker_level(BreakerLevel::L2);
//!
//! // Check current breaker state
//! if slot.is_breaker_allowing() {
//!     // Process operations normally
//! }
//!
//! // Force emergency stop
//! slot.force_breaker_open();
//! assert!(slot.is_effectively_paused());
//! ```
//!
//! See [`ApmSlot`] for detailed integration API.

pub mod adapters;
pub mod aggregator;
pub mod controller;
pub mod feed;
pub mod fractal_integration;
pub mod inputs;
pub mod layout;
pub mod risk_correlation;
pub mod runtime;
pub mod slot;
pub mod writer;

pub use adapters::{
    ActSlotFeed, CapsuleApcFeed, CapsuleAvsFeed, SharedActFeed, SharedApcFeed, SharedAvsFeed,
    SharedEcoFeed,
};
pub use aggregator::{AggregationInput, AggregationResult, SymbolState, aggregate};
pub use controller::{AccountSnapshot, PortfolioController};
pub use feed::{
    ActEdge, ActFeed, ApcFeed, ApcSnapshot, AvsFeed, AvsSnapshot, EcoFeed, FeedAssembler,
    FeedSnapshot, SymbolGates, SymbolPolicy, build_symbol_inputs,
};
pub use inputs::{PortfolioInputs, SymbolInputs};
pub use layout::{
    ApmHeader, ApmSnapshot, ApmSymbolSlice, ApmTail, ApmWords, BreakerLevel, MAX_SYMBOL_SLICES,
    PortfolioFlags, SymbolFlags,
};
pub use risk_correlation::{
    RiskCorrelationEngine, CorrelationMatrix, DualAtomicU64, PortfolioRiskAssessment,
    CorrelationStats, MAX_ASSETS,
};
pub use fractal_integration::{
    PortfolioCrossVenueCoordinator, CrossVenueArbitrageOpportunity, VenuePriceFeed,
    WilliamsMultiscalePattern, ProtectionTier, CrossVenuePerformanceStats, MAX_VENUES,
};
pub use runtime::PortfolioRuntime;
pub use slot::ApmSlot;
pub use writer::PortfolioMapWriter;
