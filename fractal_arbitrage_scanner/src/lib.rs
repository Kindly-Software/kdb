//! Fractal Arbitrage Scanner with Advanced Mathematical Analysis
//!
//! A comprehensive fractal arbitrage toolkit following UCE32 framework principles
//! with lockfree coordination and sub-microsecond latency guarantees.
//!
//! # Features
//! - **Fractal Mathematics**: MF-DFA, Williams fractals, wavelet leaders
//! - **CAKES Manifold**: O(1) k-NN with local fractal dimension
//! - **Fractal Memory**: √N storage with 3-level cache hierarchy
//! - **Williams Multiscale**: 16 timeframe coordinated analysis
//! - **Hydra Coordination**: Unified lockfree arbitrage coordination
//! - **AID-96 Identifiers**: Unique opportunity tracking
//! - **Zero-Cost Abstractions**: Complex analysis with simple interfaces

#![forbid(unsafe_code)]
#![warn(clippy::all)]
#![allow(dead_code)]
#![allow(clippy::approx_constant)]
#![cfg_attr(feature = "portable_simd", feature(portable_simd))]
#![cfg_attr(feature = "atomic_from_mut", feature(atomic_from_mut))]
#![cfg_attr(feature = "const_trait_impl", feature(const_trait_impl))]
#![cfg_attr(feature = "const_fn_floating_point_arithmetic", feature(const_fn_floating_point_arithmetic))]
#![cfg_attr(feature = "generic_const_exprs", feature(generic_const_exprs))]

// Core fractal analysis modules
pub mod fractal_mathematics;
pub mod cakes_manifold;
pub mod fractal_memory;
pub mod williams_multiscale;
pub mod hydra;

// Revolutionary 2025 algorithms (UCE32-analyzed)
pub mod levy_flight_detector;
pub mod topological_arbitrage;
pub mod recurrence_analyzer;

// Original scanner modules
pub mod scanner;
pub mod temporal;
pub mod tunneling_integration;
pub mod types;

// Advanced 2025 fractal module
pub mod advanced_fractal_2025;

// Fractal protection system
pub mod fractal_protection;

// Protection system initialization and wiring
pub mod protection_init;

// Test suite with ASSUM framework validation
#[cfg(test)]
pub mod tests;

pub use aid_96::{class as aid_class, Aid96};

// Core scanner API
pub use scanner::FractalArbitrageScanner;
pub type QuantumArbitrageScanner = scanner::FractalArbitrageScanner;
pub use temporal::TemporalArbitrageOpportunity;
pub use tunneling_integration::{BarrierType, TunnelingOpportunity, TunnelingScanner};
pub use types::{ArbitrageError, ArbitrageOpportunity, OpportunityParams};

// Fractal analysis API
pub use fractal_mathematics::{MultifractalDFA, WilliamsFractal, WaveletLeaders, PHI};
pub use cakes_manifold::{CakesManifoldEngine, MarketPoint, DualAtomicU64};
pub use fractal_memory::{FractalMemoryManager, FractalCacheKey, FractalAnalysisType, FractalCacheTier};
pub use williams_multiscale::{WilliamsMultiscaleDetector, WilliamsFractal as WilliamsFractalMultiscale};
pub use hydra::{HydraCoordinationEngine, HydraArbitrageOpportunity};

// Revolutionary 2025 algorithms API
pub use levy_flight_detector::{LevyFlightDetector, JumpType, JumpArbitrageOpportunity};
pub use topological_arbitrage::{TopologicalArbitrageDetector, TopologicalArbitrage, PersistencePair};
pub use recurrence_analyzer::{RecurrenceAnalyzer, MarketRegime, RQAMeasures, RegimeArbitrageStrategy};

// Fractal protection API
pub use fractal_protection::{
    FractalProtected, AdaptiveParameters, InstanceOptimized,
    ProtectionTier, PerformanceMetrics, DefaultAdaptiveParams,
    FractalProtectionSystem, ProtectionError as FractalProtectionError
};

// Protection system initialization API
pub use protection_init::{
    FractalProtectionManager, ProtectionConfig, ProtectionInitResults,
    PerformanceRequirements, ProtectionFeatureFlags, ProtectionSystemStatus,
    init_basic_protection, init_military_protection, init_performance_protection,
    ProtectionError as ProtectionInitError
};

// Enhanced API exports with adaptive capabilities
pub use levy_flight_detector::{AlphaLearningStats};

// Feature flags for conditional compilation
/// Enable fractal protection features
#[cfg(feature = "fractal_protection")]
pub const FRACTAL_PROTECTION_ENABLED: bool = true;

#[cfg(not(feature = "fractal_protection"))]
pub const FRACTAL_PROTECTION_ENABLED: bool = false;

/// Enable adaptive parameters
#[cfg(feature = "adaptive_parameters")]
pub const ADAPTIVE_PARAMETERS_ENABLED: bool = true;

#[cfg(not(feature = "adaptive_parameters"))]
pub const ADAPTIVE_PARAMETERS_ENABLED: bool = false;

/// Enable alpha learning in Levy flight detection
#[cfg(feature = "alpha_learning")]
pub const ALPHA_LEARNING_ENABLED: bool = true;

#[cfg(not(feature = "alpha_learning"))]
pub const ALPHA_LEARNING_ENABLED: bool = false;
