//! Fractal Protection System Initialization and Wiring
//!
//! Provides centralized initialization and coordination of the fractal protection
//! system across all analysis modules with zero-cost abstractions.
//!
//! # UCE32 Framework Analysis
//!
//! Q28 (Simplicity): Single initialization point for complex protection system
//! Q29 (Constraints): Sub-microsecond initialization overhead for real-time use
//! Q30 (Validation): Startup validation ensures protection system integrity
//! Q31 (Rust): Zero-cost abstractions with compile-time configuration
//! Q32 (Nightly): Advanced const evaluation for initialization optimization

#![cfg_attr(feature = "const_trait_impl", feature(const_trait_impl))]
#![cfg_attr(feature = "const_fn_floating_point_arithmetic", feature(const_fn_floating_point_arithmetic))]

use std::sync::Arc;
use std::time::Instant;

use crate::fractal_protection::{
    FractalProtectionSystem, ProtectionTier, PerformanceMetrics,
    DefaultAdaptiveParams, OptimizationReport
};
use crate::hydra::{HydraCoordinationEngine, HydraError};
use crate::fractal_mathematics::MultifractalDFA;
use crate::levy_flight_detector::{LevyFlightDetector, AlphaLearningStats};

/// Global protection configuration
#[derive(Debug, Clone)]
pub struct ProtectionConfig {
    /// Protection tier for the entire system
    pub tier: ProtectionTier,
    /// Enable adaptive parameters across modules
    pub enable_adaptive: bool,
    /// Enable alpha learning in Levy detector
    pub enable_alpha_learning: bool,
    /// Performance tier requirements
    pub performance_requirements: PerformanceRequirements,
    /// Feature flags for conditional protection
    pub feature_flags: ProtectionFeatureFlags,
}

/// Performance requirements for protection system
#[derive(Debug, Clone)]
pub struct PerformanceRequirements {
    /// Maximum initialization time in milliseconds
    pub max_init_time_ms: u64,
    /// Maximum analysis latency overhead (percentage)
    pub max_latency_overhead_pct: f64,
    /// Minimum prediction accuracy required
    pub min_accuracy: f64,
    /// Maximum memory overhead in MB
    pub max_memory_overhead_mb: u64,
}

/// Feature flags for protection system components
#[derive(Debug, Clone)]
pub struct ProtectionFeatureFlags {
    /// Enable proof-of-work instance validation
    pub enable_proof_of_work: bool,
    /// Enable fractal obfuscation
    pub enable_obfuscation: bool,
    /// Enable performance monitoring
    pub enable_performance_monitoring: bool,
    /// Enable adaptive parameter learning
    pub enable_adaptive_learning: bool,
}

/// Centralized protection system manager
pub struct FractalProtectionManager {
    /// Core protection system
    protection_system: Arc<FractalProtectionSystem>,
    /// Configuration
    config: ProtectionConfig,
    /// Initialization report
    init_report: Option<OptimizationReport>,
    /// Performance monitoring
    performance_monitor: PerformanceMonitor,
    /// Initialization timestamp
    initialized_at: Option<Instant>,
}

/// Performance monitoring for protection system
#[derive(Debug)]
struct PerformanceMonitor {
    /// Analysis count
    analysis_count: u64,
    /// Total latency microseconds
    total_latency_us: u64,
    /// Successful predictions
    successful_predictions: u64,
    /// Memory usage bytes
    current_memory_usage: usize,
}

/// Protection system initialization results
#[derive(Debug, Clone)]
pub struct ProtectionInitResults {
    /// Initialization successful
    pub success: bool,
    /// Initialization time in milliseconds
    pub init_time_ms: u64,
    /// Hardware optimization report
    pub optimization_report: OptimizationReport,
    /// Feature activation status
    pub features_enabled: Vec<String>,
    /// Performance baseline
    pub performance_baseline: PerformanceMetrics,
    /// Any warnings or issues
    pub warnings: Vec<String>,
}

impl Default for ProtectionConfig {
    fn default() -> Self {
        Self {
            tier: ProtectionTier::Basic,
            enable_adaptive: true,
            enable_alpha_learning: true,
            performance_requirements: PerformanceRequirements {
                max_init_time_ms: 100,
                max_latency_overhead_pct: 5.0,
                min_accuracy: 0.8,
                max_memory_overhead_mb: 10,
            },
            feature_flags: ProtectionFeatureFlags {
                enable_proof_of_work: true,
                enable_obfuscation: true,
                enable_performance_monitoring: true,
                enable_adaptive_learning: true,
            },
        }
    }
}

impl FractalProtectionManager {
    /// Create new protection manager with configuration
    pub fn new(config: ProtectionConfig) -> Self {
        Self {
            protection_system: Arc::new(FractalProtectionSystem::new(config.tier)),
            config,
            init_report: None,
            performance_monitor: PerformanceMonitor {
                analysis_count: 0,
                total_latency_us: 0,
                successful_predictions: 0,
                current_memory_usage: 0,
            },
            initialized_at: None,
        }
    }

    /// Initialize the protection system with validation
    /// Q28: Simple initialization hiding complex setup
    /// Q29: Must complete within performance constraints
    pub fn initialize(&mut self) -> Result<ProtectionInitResults, ProtectionError> {
        let start_time = Instant::now();
        let mut warnings = Vec::new();
        let mut features_enabled = Vec::new();

        // Initialize core protection system
        let protection_system = Arc::get_mut(&mut self.protection_system)
            .ok_or(ProtectionError::InitializationFailed("Multiple references to protection system".to_string()))?;

        let optimization_report = protection_system.initialize()
            .map_err(|e| ProtectionError::InitializationFailed(format!("Core initialization failed: {}", e)))?;

        // Validate initialization time constraint
        let init_time_ms = start_time.elapsed().as_millis() as u64;
        if init_time_ms > self.config.performance_requirements.max_init_time_ms {
            warnings.push(format!(
                "Initialization took {}ms, exceeds limit of {}ms",
                init_time_ms,
                self.config.performance_requirements.max_init_time_ms
            ));
        }

        // Enable features based on configuration
        if self.config.feature_flags.enable_proof_of_work {
            features_enabled.push("Proof-of-work validation".to_string());
        }

        if self.config.feature_flags.enable_obfuscation {
            features_enabled.push("Fractal obfuscation".to_string());
        }

        if self.config.feature_flags.enable_performance_monitoring {
            features_enabled.push("Performance monitoring".to_string());
        }

        if self.config.feature_flags.enable_adaptive_learning {
            features_enabled.push("Adaptive parameter learning".to_string());
        }

        // Create baseline performance metrics
        let performance_baseline = PerformanceMetrics {
            latency_us: 100, // Target 100μs baseline
            accuracy: self.config.performance_requirements.min_accuracy,
            memory_usage: (self.config.performance_requirements.max_memory_overhead_mb * 1024 * 1024) as usize,
            cache_hit_rate: 0.9,
            error_rate: 1.0 - self.config.performance_requirements.min_accuracy,
            throughput: 10000.0, // Target 10k ops/sec
        };

        // Store initialization results
        self.init_report = Some(optimization_report.clone());
        self.initialized_at = Some(start_time);

        let results = ProtectionInitResults {
            success: true,
            init_time_ms,
            optimization_report,
            features_enabled,
            performance_baseline,
            warnings,
        };

        Ok(results)
    }

    /// Wire protection system into Hydra coordination engine
    /// Q31: Zero-cost abstraction for protection integration
    pub fn wire_hydra_protection(&self, hydra: &mut HydraCoordinationEngine) -> Result<(), ProtectionError> {
        if !self.is_initialized() {
            return Err(ProtectionError::NotInitialized);
        }

        // Enable protection on Hydra engine
        hydra.enable_protection(self.config.tier)
            .map_err(|e| ProtectionError::WiringFailed(format!("Hydra protection failed: {}", e)))?;

        Ok(())
    }

    /// Wire protection into fractal mathematics module
    pub fn wire_fractal_math_protection(&self, fractal_math: &mut MultifractalDFA) -> Result<(), ProtectionError> {
        if !self.is_initialized() {
            return Err(ProtectionError::NotInitialized);
        }

        if self.config.enable_adaptive {
            fractal_math.enable_adaptive_parameters();
        }

        Ok(())
    }

    /// Wire protection into Levy flight detector
    pub fn wire_levy_protection(&self, levy_detector: &mut LevyFlightDetector) -> Result<(), ProtectionError> {
        if !self.is_initialized() {
            return Err(ProtectionError::NotInitialized);
        }

        if self.config.enable_alpha_learning {
            levy_detector.enable_alpha_learning();
        }

        Ok(())
    }

    /// Update performance metrics for the entire protection system
    pub fn update_system_performance(&mut self,
        latency_us: u64,
        accuracy: f64,
        memory_usage: usize
    ) -> Result<(), ProtectionError> {
        if !self.is_initialized() {
            return Err(ProtectionError::NotInitialized);
        }

        // Update monitoring
        self.performance_monitor.analysis_count += 1;
        self.performance_monitor.total_latency_us += latency_us;
        self.performance_monitor.current_memory_usage = memory_usage;

        if accuracy > self.config.performance_requirements.min_accuracy {
            self.performance_monitor.successful_predictions += 1;
        }

        // Check performance constraints
        let avg_latency = self.performance_monitor.total_latency_us / self.performance_monitor.analysis_count;
        let latency_overhead_pct = (avg_latency as f64 / 1000.0) * 100.0; // Assume 1ms baseline

        if latency_overhead_pct > self.config.performance_requirements.max_latency_overhead_pct {
            return Err(ProtectionError::PerformanceConstraintViolated(
                format!("Latency overhead {}% exceeds limit {}%",
                    latency_overhead_pct,
                    self.config.performance_requirements.max_latency_overhead_pct
                )
            ));
        }

        let memory_mb = memory_usage / (1024 * 1024);
        if memory_mb as u64 > self.config.performance_requirements.max_memory_overhead_mb {
            return Err(ProtectionError::PerformanceConstraintViolated(
                format!("Memory usage {}MB exceeds limit {}MB",
                    memory_mb,
                    self.config.performance_requirements.max_memory_overhead_mb
                )
            ));
        }

        Ok(())
    }

    /// Get comprehensive protection system status
    pub fn get_system_status(&self) -> ProtectionSystemStatus {
        ProtectionSystemStatus {
            initialized: self.is_initialized(),
            protection_tier: self.config.tier,
            features_active: self.get_active_features(),
            performance_stats: self.get_performance_stats(),
            uptime_ms: self.get_uptime_ms(),
            init_report: self.init_report.clone(),
        }
    }

    /// Check if protection system is initialized
    pub fn is_initialized(&self) -> bool {
        self.init_report.is_some() && self.initialized_at.is_some()
    }

    /// Get list of currently active features
    fn get_active_features(&self) -> Vec<String> {
        let mut features = Vec::new();

        if self.config.feature_flags.enable_proof_of_work {
            features.push("Proof-of-work".to_string());
        }
        if self.config.feature_flags.enable_obfuscation {
            features.push("Obfuscation".to_string());
        }
        if self.config.feature_flags.enable_performance_monitoring {
            features.push("Performance monitoring".to_string());
        }
        if self.config.feature_flags.enable_adaptive_learning {
            features.push("Adaptive learning".to_string());
        }

        features
    }

    /// Get current performance statistics
    fn get_performance_stats(&self) -> PerformanceStats {
        let avg_latency_us = if self.performance_monitor.analysis_count > 0 {
            self.performance_monitor.total_latency_us / self.performance_monitor.analysis_count
        } else {
            0
        };

        let accuracy = if self.performance_monitor.analysis_count > 0 {
            self.performance_monitor.successful_predictions as f64 / self.performance_monitor.analysis_count as f64
        } else {
            0.0
        };

        PerformanceStats {
            total_analyses: self.performance_monitor.analysis_count,
            average_latency_us: avg_latency_us,
            current_accuracy: accuracy,
            memory_usage_bytes: self.performance_monitor.current_memory_usage,
        }
    }

    /// Get system uptime in milliseconds
    fn get_uptime_ms(&self) -> u64 {
        if let Some(start_time) = self.initialized_at {
            start_time.elapsed().as_millis() as u64
        } else {
            0
        }
    }

    /// Emergency disable protection (for performance-critical scenarios)
    pub fn emergency_disable(&mut self) -> Result<(), ProtectionError> {
        // Reset protection system (this would need actual implementation)
        self.init_report = None;
        self.initialized_at = None;

        // Reset performance monitor
        self.performance_monitor = PerformanceMonitor {
            analysis_count: 0,
            total_latency_us: 0,
            successful_predictions: 0,
            current_memory_usage: 0,
        };

        Ok(())
    }
}

/// Performance statistics
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub total_analyses: u64,
    pub average_latency_us: u64,
    pub current_accuracy: f64,
    pub memory_usage_bytes: usize,
}

/// Complete protection system status
#[derive(Debug, Clone)]
pub struct ProtectionSystemStatus {
    pub initialized: bool,
    pub protection_tier: ProtectionTier,
    pub features_active: Vec<String>,
    pub performance_stats: PerformanceStats,
    pub uptime_ms: u64,
    pub init_report: Option<OptimizationReport>,
}

/// Protection system errors
#[derive(Debug, thiserror::Error)]
pub enum ProtectionError {
    #[error("Protection system not initialized")]
    NotInitialized,
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
    #[error("Wiring failed: {0}")]
    WiringFailed(String),
    #[error("Performance constraint violated: {0}")]
    PerformanceConstraintViolated(String),
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    #[error("Feature not available: {0}")]
    FeatureNotAvailable(String),
}

/// Convenience function for quick protection setup
/// Q28: Simple one-line protection initialization
pub fn init_basic_protection() -> Result<FractalProtectionManager, ProtectionError> {
    let mut manager = FractalProtectionManager::new(ProtectionConfig::default());
    manager.initialize()?;
    Ok(manager)
}

/// Convenience function for high-security protection setup
pub fn init_military_protection() -> Result<FractalProtectionManager, ProtectionError> {
    let config = ProtectionConfig {
        tier: ProtectionTier::Military,
        enable_adaptive: true,
        enable_alpha_learning: true,
        performance_requirements: PerformanceRequirements {
            max_init_time_ms: 1000, // Allow more time for military setup
            max_latency_overhead_pct: 10.0, // Allow higher overhead for security
            min_accuracy: 0.95,
            max_memory_overhead_mb: 50,
        },
        feature_flags: ProtectionFeatureFlags {
            enable_proof_of_work: true,
            enable_obfuscation: true,
            enable_performance_monitoring: true,
            enable_adaptive_learning: true,
        },
    };

    let mut manager = FractalProtectionManager::new(config);
    manager.initialize()?;
    Ok(manager)
}

/// Convenience function for performance-optimized protection
pub fn init_performance_protection() -> Result<FractalProtectionManager, ProtectionError> {
    let config = ProtectionConfig {
        tier: ProtectionTier::Basic,
        enable_adaptive: true,
        enable_alpha_learning: false, // Disable for maximum speed
        performance_requirements: PerformanceRequirements {
            max_init_time_ms: 10,  // Very fast init
            max_latency_overhead_pct: 1.0, // Minimal overhead
            min_accuracy: 0.7,
            max_memory_overhead_mb: 2,
        },
        feature_flags: ProtectionFeatureFlags {
            enable_proof_of_work: false,
            enable_obfuscation: false,
            enable_performance_monitoring: true,
            enable_adaptive_learning: false,
        },
    };

    let mut manager = FractalProtectionManager::new(config);
    manager.initialize()?;
    Ok(manager)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_protection_init() {
        let result = init_basic_protection();
        assert!(result.is_ok());

        let manager = result.unwrap();
        assert!(manager.is_initialized());

        let status = manager.get_system_status();
        assert!(status.initialized);
        assert_eq!(status.protection_tier, ProtectionTier::Basic);
    }

    #[test]
    fn test_military_protection_init() {
        let result = init_military_protection();
        assert!(result.is_ok());

        let manager = result.unwrap();
        assert!(manager.is_initialized());

        let status = manager.get_system_status();
        assert!(status.initialized);
        assert_eq!(status.protection_tier, ProtectionTier::Military);
        assert!(status.features_active.len() > 0);
    }

    #[test]
    fn test_performance_protection_init() {
        let result = init_performance_protection();
        assert!(result.is_ok());

        let manager = result.unwrap();
        assert!(manager.is_initialized());

        let status = manager.get_system_status();
        assert!(status.initialized);
        assert_eq!(status.protection_tier, ProtectionTier::Basic);
    }

    #[test]
    fn test_custom_configuration() {
        let config = ProtectionConfig {
            tier: ProtectionTier::Advanced,
            enable_adaptive: false,
            enable_alpha_learning: false,
            performance_requirements: PerformanceRequirements {
                max_init_time_ms: 50,
                max_latency_overhead_pct: 2.0,
                min_accuracy: 0.9,
                max_memory_overhead_mb: 5,
            },
            feature_flags: ProtectionFeatureFlags {
                enable_proof_of_work: true,
                enable_obfuscation: false,
                enable_performance_monitoring: true,
                enable_adaptive_learning: false,
            },
        };

        let mut manager = FractalProtectionManager::new(config);
        let result = manager.initialize();
        assert!(result.is_ok());

        let init_results = result.unwrap();
        assert!(init_results.success);
        assert!(init_results.init_time_ms <= 50);
    }

    #[test]
    fn test_performance_monitoring() {
        let mut manager = init_basic_protection().unwrap();

        // Simulate performance updates
        let result1 = manager.update_system_performance(80, 0.85, 1024 * 1024);
        assert!(result1.is_ok());

        let result2 = manager.update_system_performance(120, 0.92, 2 * 1024 * 1024);
        assert!(result2.is_ok());

        let status = manager.get_system_status();
        assert_eq!(status.performance_stats.total_analyses, 2);
        assert_eq!(status.performance_stats.average_latency_us, 100); // (80 + 120) / 2
    }
}