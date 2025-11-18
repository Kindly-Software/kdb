//! Startup configuration validation
//!
//! Pre-flight checks for production deployment: feature compatibility, resource availability,
//! platform requirements.
//!
//! ## Design Philosophy
//!
//! - **Fail-fast**: Validate configuration before any resource allocation
//! - **Actionable errors**: Clear guidance on how to fix configuration issues
//! - **Platform-aware**: Detect and warn about platform-specific limitations
//! - **Zero runtime cost**: All checks run once at startup
//!
//! ## Example
//!
//! ```rust
//! use kindly_dedup::config_validation::validate_deployment_config;
//!
//! // Call before pipeline initialization
//! validate_deployment_config()?;
//! # Ok::<(), kindly_dedup::config_validation::ConfigError>(())
//! ```
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q32 (Constraints validation), Q33 (Feature compatibility)
//! - **ASSUM**: #ASSUME feature flags accurate, #VERIFY with compile-time checks
//! - **T28**: Unit tests for all validation paths

use crate::resource_limits::ResourceLimits;

/// Configuration validation error
///
/// Actionable errors with clear remediation guidance.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Insufficient memory for requested operation
    #[error("Insufficient memory: required {required} bytes, available {available} bytes. Reduce document count or increase memory.")]
    InsufficientMemory {
        /// Minimum required memory
        required: usize,
        /// Available memory
        available: usize,
    },

    /// Incompatible feature combination
    #[error("Incompatible features: {reason}. {remediation}")]
    IncompatibleFeatures {
        /// Reason for incompatibility
        reason: String,
        /// How to fix the issue
        remediation: String,
    },

    /// Platform requirement not met
    #[error("Platform requirement not met: {requirement}. {remediation}")]
    PlatformRequirementNotMet {
        /// Missing requirement
        requirement: String,
        /// How to fix the issue
        remediation: String,
    },

    /// Nightly feature on stable Rust
    #[error(
        "Nightly feature '{feature}' requires nightly Rust. Run with: rustup default nightly && cargo +nightly build"
    )]
    NightlyFeatureOnStable {
        /// Feature requiring nightly
        feature: String,
    },
}

/// Validate deployment configuration
///
/// Performs comprehensive pre-flight checks before pipeline initialization:
/// 1. Feature compatibility (e.g., SIMD requires nightly)
/// 2. Resource availability (memory, CPU threads)
/// 3. Platform requirements (CPU features for SIMD)
///
/// Call this function at program startup before any pipeline operations.
///
/// # Errors
///
/// Returns `ConfigError` if configuration is invalid or requirements are unmet.
///
/// # Examples
///
/// ```
/// use kindly_dedup::config_validation::validate_deployment_config;
///
/// // Call before DedupPipeline::new()
/// validate_deployment_config()?;
/// # Ok::<(), kindly_dedup::config_validation::ConfigError>(())
/// ```
///
/// # Framework Compliance
///
/// - **UCE34 Q32**: Constraint validation (memory, features, platform)
/// - **ASSUM**: #ASSUME compile_time_checks accurate, #VERIFY with tests
pub fn validate_deployment_config() -> Result<(), ConfigError> {
    // Check 1: Feature compatibility
    validate_feature_compatibility()?;

    // Check 2: Memory availability
    validate_memory_availability()?;

    // Check 3: CPU requirements (for SIMD features)
    validate_cpu_requirements()?;

    // Check 4: Nightly features on stable Rust
    validate_nightly_features()?;

    Ok(())
}

/// Validate feature flag compatibility
///
/// Checks for incompatible or suboptimal feature combinations.
fn validate_feature_compatibility() -> Result<(), ConfigError> {
    // Warning: persistent-dedup works best with parallel-dedup
    #[cfg(all(feature = "persistent-dedup", not(feature = "parallel-dedup")))]
    {
        eprintln!("WARNING: persistent-dedup works best with parallel-dedup enabled");
        eprintln!("         Consider: cargo build --features 'persistent-dedup,parallel-dedup'");
    }

    // Warning: simd-text-hashing requires simd-minhash
    #[cfg(all(feature = "simd-text-hashing", not(feature = "simd-minhash")))]
    {
        return Err(ConfigError::IncompatibleFeatures {
            reason: "simd-text-hashing requires simd-minhash".to_string(),
            remediation: "Enable simd-minhash: cargo build --features 'simd-minhash,simd-text-hashing'".to_string(),
        });
    }

    // Warning: cache-optimized-minhash requires simd-minhash
    #[cfg(all(feature = "cache-optimized-minhash", not(feature = "simd-minhash")))]
    {
        return Err(ConfigError::IncompatibleFeatures {
            reason: "cache-optimized-minhash requires simd-minhash".to_string(),
            remediation: "Enable simd-minhash: cargo build --features 'simd-minhash,cache-optimized-minhash'"
                .to_string(),
        });
    }

    // Warning: avx512-minhash requires simd-minhash
    #[cfg(all(feature = "avx512-minhash", not(feature = "simd-minhash")))]
    {
        return Err(ConfigError::IncompatibleFeatures {
            reason: "avx512-minhash requires simd-minhash".to_string(),
            remediation: "Enable simd-minhash: cargo build --features 'simd-minhash,avx512-minhash'".to_string(),
        });
    }

    Ok(())
}

/// Validate memory availability
///
/// Ensures system has minimum memory for basic operation (2GB).
fn validate_memory_availability() -> Result<(), ConfigError> {
    let limits = ResourceLimits::detect();
    const MIN_MEMORY: usize = 2 * 1024 * 1024 * 1024; // 2GB minimum

    if limits.max_memory_bytes < MIN_MEMORY {
        return Err(ConfigError::InsufficientMemory {
            required: MIN_MEMORY,
            available: limits.max_memory_bytes,
        });
    }

    Ok(())
}

/// Validate CPU requirements (for SIMD features)
///
/// Checks CPU capabilities for SIMD features. Note: Runtime CPU detection
/// is performed by atomic_capsule, this only validates compile-time requirements.
fn validate_cpu_requirements() -> Result<(), ConfigError> {
    // AVX-512 requires x86_64 target
    #[cfg(all(feature = "avx512-minhash", not(target_arch = "x86_64")))]
    {
        return Err(ConfigError::PlatformRequirementNotMet {
            requirement: "avx512-minhash requires x86_64 architecture".to_string(),
            remediation: "Disable avx512-minhash or build for x86_64 target".to_string(),
        });
    }

    // SIMD MinHash works on all architectures (portable_simd)
    // No validation needed - runtime dispatch handles this

    Ok(())
}

/// Validate nightly feature usage
///
/// Checks that nightly-only features are not enabled on stable Rust.
fn validate_nightly_features() -> Result<(), ConfigError> {
    // simd-minhash requires nightly
    #[cfg(all(feature = "simd-minhash", not(feature = "nightly")))]
    {
        return Err(ConfigError::NightlyFeatureOnStable {
            feature: "simd-minhash".to_string(),
        });
    }

    // cache-optimized-minhash requires nightly
    #[cfg(all(feature = "cache-optimized-minhash", not(feature = "nightly")))]
    {
        return Err(ConfigError::NightlyFeatureOnStable {
            feature: "cache-optimized-minhash".to_string(),
        });
    }

    // avx512-minhash requires nightly
    #[cfg(all(feature = "avx512-minhash", not(feature = "nightly")))]
    {
        return Err(ConfigError::NightlyFeatureOnStable {
            feature: "avx512-minhash".to_string(),
        });
    }

    Ok(())
}

/// Validate configuration for specific document count
///
/// Checks that system resources can handle the requested document count.
///
/// # Examples
///
/// ```
/// use kindly_dedup::config_validation::validate_for_document_count;
///
/// // Validate before creating pipeline with 10M documents
/// validate_for_document_count(10_000_000)?;
/// # Ok::<(), kindly_dedup::config_validation::ConfigError>(())
/// ```
pub fn validate_for_document_count(num_documents: usize) -> Result<(), ConfigError> {
    let limits = ResourceLimits::detect();

    // Check document count limit
    limits
        .check_document_count(num_documents)
        .map_err(|e| ConfigError::InsufficientMemory {
            required: num_documents,
            available: limits.max_documents,
        })?;

    // Check estimated memory usage
    limits.check_memory_estimate(num_documents).map_err(|e| {
        let estimated = limits.estimate_memory_usage(num_documents);
        ConfigError::InsufficientMemory {
            required: estimated,
            available: limits.max_memory_bytes,
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_deployment_config() {
        // Should pass with default configuration
        let result = validate_deployment_config();
        assert!(result.is_ok(), "Default config should be valid: {:?}", result);
    }

    #[test]
    fn test_validate_memory_availability() {
        // Should pass if system has >2GB
        let result = validate_memory_availability();
        assert!(result.is_ok(), "System should have >2GB memory: {:?}", result);
    }

    #[test]
    fn test_validate_for_document_count_ok() {
        // Small document count should always pass
        let result = validate_for_document_count(100_000);
        assert!(result.is_ok(), "100K documents should be valid: {:?}", result);
    }

    #[test]
    fn test_validate_for_document_count_exceeds() {
        // Exceeds default 50M limit
        let result = validate_for_document_count(100_000_000);
        assert!(result.is_err(), "100M documents should exceed limit");
    }

    #[test]
    fn test_feature_compatibility() {
        // Should not panic or error with current feature configuration
        let result = validate_feature_compatibility();
        assert!(result.is_ok(), "Feature compatibility should pass: {:?}", result);
    }

    #[test]
    fn test_cpu_requirements() {
        // Should pass on all platforms (runtime dispatch)
        let result = validate_cpu_requirements();
        assert!(result.is_ok(), "CPU requirements should pass: {:?}", result);
    }

    #[test]
    fn test_nightly_features() {
        // Should match compile-time feature configuration
        let result = validate_nightly_features();
        assert!(result.is_ok(), "Nightly feature validation should pass: {:?}", result);
    }
}
