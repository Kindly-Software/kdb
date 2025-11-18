//! [TRADE SECRET] License tier definitions and feature sets
//!
//! Three production tiers + Trial tier:
//! 1. **Trial** (7 days): Pro features for evaluation
//! 2. **Starter** (1 year): Commercial use, 500 GB limit
//! 3. **Pro** (1 year): Unlimited, priority support
//! 4. **Enterprise** (Custom): Custom limits, dedicated support
//!
//! ## Tier Comparison
//!
//! | Feature | Trial | Starter | Pro | Enterprise |
//! |---------|-------|---------|-----|------------|
//! | Duration | 7 days | 1 year | 1 year | Custom |
//! | Document Limit | 1M | 10M | 100M | Custom |
//! | Thread Limit | 8 | 32 | 128 | Unlimited |
//! | SIMD MinHash | ✓ | ✓ | ✓ | ✓ |
//! | Audit Trail | ✓ | ✓ | ✓ | ✓ |
//! | Multi-threaded | ✓ | ✓ | ✓ | ✓ |
//! | Bloom Pre-filter | ✓ | ✓ | ✓ | ✓ |
//! | Batch LSH | ✓ | ✓ | ✓ | ✓ |
//! | Persistent Mode | ✗ | ✗ | ✓ | ✓ |
//! | HTTP API | ✗ | ✗ | ✗ | ✓ |
//! | Priority Support | ✗ | Email | ✓ | ✓ |
//! | SLA Guarantee | ✗ | ✗ | 99.5% | 99.9% |

use std::fmt;

/// License feature flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LicenseFeature {
    // Basic features (all tiers)
    /// Multi-threaded processing
    MultiThreaded,

    /// Audit trail logging (Q34 compliance)
    AuditTrail,

    /// SIMD MinHash (7.1× speedup)
    SimdMinHash,

    /// Bloom pre-filter (skip duplicates)
    BloomPrefilter,

    /// Batch LSH lookup (1.5× speedup)
    BatchLsh,

    // Advanced features (Pro+)
    /// Persistent deduplication (mmap-based)
    PersistentMode,

    /// AVX-512 MinHash (2× vs AVX2)
    Avx512MinHash,

    // Enterprise only
    /// HTTP API endpoint
    HttpApi,

    /// Compliance reports (SOX/SOC2/GDPR/HIPAA)
    ComplianceReports,

    /// Priority technical support (4hr response)
    PrioritySupport,

    /// SLA guarantee (99.9% uptime)
    SlaGuarantee,
}

impl fmt::Display for LicenseFeature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LicenseFeature::MultiThreaded => write!(f, "Multi-threaded"),
            LicenseFeature::AuditTrail => write!(f, "Audit Trail"),
            LicenseFeature::SimdMinHash => write!(f, "SIMD MinHash"),
            LicenseFeature::BloomPrefilter => write!(f, "Bloom Pre-filter"),
            LicenseFeature::BatchLsh => write!(f, "Batch LSH"),
            LicenseFeature::PersistentMode => write!(f, "Persistent Mode"),
            LicenseFeature::Avx512MinHash => write!(f, "AVX-512 MinHash"),
            LicenseFeature::HttpApi => write!(f, "HTTP API"),
            LicenseFeature::ComplianceReports => write!(f, "Compliance Reports"),
            LicenseFeature::PrioritySupport => write!(f, "Priority Support"),
            LicenseFeature::SlaGuarantee => write!(f, "SLA Guarantee"),
        }
    }
}

/// License tier configuration
#[derive(Debug, Clone)]
pub struct LicenseConfig {
    /// Document processing limit (0 = unlimited)
    pub document_limit: usize,

    /// Maximum concurrent threads
    pub max_threads: usize,

    /// Data processing limit (GB)
    pub data_limit_gb: Option<u64>,

    /// Enabled features
    pub features: Vec<LicenseFeature>,

    /// Price per year (cents)
    pub price_cents: u32,

    /// License duration (days)
    pub duration_days: u32,
}

impl LicenseConfig {
    /// Create configuration for a tier
    pub fn for_tier(tier: crate::license_capsule::LicenseTier) -> Self {
        use crate::license_capsule::LicenseTier;

        match tier {
            LicenseTier::Trial => Self {
                document_limit: 1_000_000, // 1M docs
                max_threads: 8,
                data_limit_gb: Some(100),
                features: vec![
                    LicenseFeature::MultiThreaded,
                    LicenseFeature::AuditTrail,
                    LicenseFeature::SimdMinHash,
                    LicenseFeature::BloomPrefilter,
                    LicenseFeature::BatchLsh,
                ],
                price_cents: 0, // Free
                duration_days: 7,
            },

            LicenseTier::Starter => Self {
                document_limit: 10_000_000, // 10M docs
                max_threads: 32,
                data_limit_gb: Some(500),
                features: vec![
                    LicenseFeature::MultiThreaded,
                    LicenseFeature::AuditTrail,
                    LicenseFeature::SimdMinHash,
                    LicenseFeature::BloomPrefilter,
                    LicenseFeature::BatchLsh,
                    LicenseFeature::PrioritySupport,
                ],
                price_cents: 50_000, // $500/year
                duration_days: 365,
            },

            LicenseTier::Pro => Self {
                document_limit: 100_000_000, // 100M docs
                max_threads: 128,
                data_limit_gb: None, // Unlimited
                features: vec![
                    LicenseFeature::MultiThreaded,
                    LicenseFeature::AuditTrail,
                    LicenseFeature::SimdMinHash,
                    LicenseFeature::BloomPrefilter,
                    LicenseFeature::BatchLsh,
                    LicenseFeature::PersistentMode,
                    LicenseFeature::Avx512MinHash,
                    LicenseFeature::PrioritySupport,
                    LicenseFeature::SlaGuarantee,
                ],
                price_cents: 150_000, // $1500/year
                duration_days: 365,
            },

            LicenseTier::Enterprise => Self {
                document_limit: usize::MAX,
                max_threads: usize::MAX,
                data_limit_gb: None,
                features: vec![
                    LicenseFeature::MultiThreaded,
                    LicenseFeature::AuditTrail,
                    LicenseFeature::SimdMinHash,
                    LicenseFeature::BloomPrefilter,
                    LicenseFeature::BatchLsh,
                    LicenseFeature::PersistentMode,
                    LicenseFeature::Avx512MinHash,
                    LicenseFeature::HttpApi,
                    LicenseFeature::ComplianceReports,
                    LicenseFeature::PrioritySupport,
                    LicenseFeature::SlaGuarantee,
                ],
                price_cents: 500_000, // $5000/year (starting)
                duration_days: 365,
            },
        }
    }

    /// Get price as formatted string
    pub fn price_display(&self) -> String {
        if self.price_cents == 0 {
            "Free".to_string()
        } else {
            format!("${:.2}", self.price_cents as f64 / 100.0)
        }
    }

    /// Get duration as formatted string
    pub fn duration_display(&self) -> String {
        match self.duration_days {
            7 => "7 days".to_string(),
            365 => "1 year".to_string(),
            9999 => "Custom".to_string(),
            n => format!("{} days", n),
        }
    }

    /// Check if feature is enabled
    pub fn has_feature(&self, feature: LicenseFeature) -> bool {
        self.features.contains(&feature)
    }

    /// Get all feature names as strings
    pub fn feature_names(&self) -> Vec<String> {
        self.features.iter().map(|f| f.to_string()).collect()
    }

    /// Get all features as display strings
    pub fn features_display(&self) -> String {
        self.feature_names().join(", ")
    }
}

/// Feature comparison matrix
pub struct FeatureMatrix;

impl FeatureMatrix {
    /// Build comparison table for all tiers
    pub fn build_table() -> Vec<(String, bool, bool, bool, bool)> {
        use crate::license_capsule::LicenseTier;

        let trial = LicenseConfig::for_tier(LicenseTier::Trial);
        let starter = LicenseConfig::for_tier(LicenseTier::Starter);
        let pro = LicenseConfig::for_tier(LicenseTier::Pro);
        let enterprise = LicenseConfig::for_tier(LicenseTier::Enterprise);

        vec![
            (
                "Multi-threaded".to_string(),
                trial.has_feature(LicenseFeature::MultiThreaded),
                starter.has_feature(LicenseFeature::MultiThreaded),
                pro.has_feature(LicenseFeature::MultiThreaded),
                enterprise.has_feature(LicenseFeature::MultiThreaded),
            ),
            (
                "Audit Trail".to_string(),
                trial.has_feature(LicenseFeature::AuditTrail),
                starter.has_feature(LicenseFeature::AuditTrail),
                pro.has_feature(LicenseFeature::AuditTrail),
                enterprise.has_feature(LicenseFeature::AuditTrail),
            ),
            (
                "SIMD MinHash".to_string(),
                trial.has_feature(LicenseFeature::SimdMinHash),
                starter.has_feature(LicenseFeature::SimdMinHash),
                pro.has_feature(LicenseFeature::SimdMinHash),
                enterprise.has_feature(LicenseFeature::SimdMinHash),
            ),
            (
                "Bloom Pre-filter".to_string(),
                trial.has_feature(LicenseFeature::BloomPrefilter),
                starter.has_feature(LicenseFeature::BloomPrefilter),
                pro.has_feature(LicenseFeature::BloomPrefilter),
                enterprise.has_feature(LicenseFeature::BloomPrefilter),
            ),
            (
                "Batch LSH".to_string(),
                trial.has_feature(LicenseFeature::BatchLsh),
                starter.has_feature(LicenseFeature::BatchLsh),
                pro.has_feature(LicenseFeature::BatchLsh),
                enterprise.has_feature(LicenseFeature::BatchLsh),
            ),
            (
                "Persistent Mode".to_string(),
                trial.has_feature(LicenseFeature::PersistentMode),
                starter.has_feature(LicenseFeature::PersistentMode),
                pro.has_feature(LicenseFeature::PersistentMode),
                enterprise.has_feature(LicenseFeature::PersistentMode),
            ),
            (
                "AVX-512 MinHash".to_string(),
                trial.has_feature(LicenseFeature::Avx512MinHash),
                starter.has_feature(LicenseFeature::Avx512MinHash),
                pro.has_feature(LicenseFeature::Avx512MinHash),
                enterprise.has_feature(LicenseFeature::Avx512MinHash),
            ),
            (
                "HTTP API".to_string(),
                trial.has_feature(LicenseFeature::HttpApi),
                starter.has_feature(LicenseFeature::HttpApi),
                pro.has_feature(LicenseFeature::HttpApi),
                enterprise.has_feature(LicenseFeature::HttpApi),
            ),
            (
                "Compliance Reports".to_string(),
                trial.has_feature(LicenseFeature::ComplianceReports),
                starter.has_feature(LicenseFeature::ComplianceReports),
                pro.has_feature(LicenseFeature::ComplianceReports),
                enterprise.has_feature(LicenseFeature::ComplianceReports),
            ),
            (
                "Priority Support".to_string(),
                trial.has_feature(LicenseFeature::PrioritySupport),
                starter.has_feature(LicenseFeature::PrioritySupport),
                pro.has_feature(LicenseFeature::PrioritySupport),
                enterprise.has_feature(LicenseFeature::PrioritySupport),
            ),
            (
                "SLA Guarantee".to_string(),
                trial.has_feature(LicenseFeature::SlaGuarantee),
                starter.has_feature(LicenseFeature::SlaGuarantee),
                pro.has_feature(LicenseFeature::SlaGuarantee),
                enterprise.has_feature(LicenseFeature::SlaGuarantee),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license_capsule::LicenseTier;

    #[test]
    fn test_tier_config_trial() {
        let config = LicenseConfig::for_tier(LicenseTier::Trial);
        assert_eq!(config.document_limit, 1_000_000);
        assert_eq!(config.max_threads, 8);
        assert_eq!(config.duration_days, 7);
        assert_eq!(config.price_cents, 0);
    }

    #[test]
    fn test_tier_config_pro() {
        let config = LicenseConfig::for_tier(LicenseTier::Pro);
        assert_eq!(config.document_limit, 100_000_000);
        assert_eq!(config.max_threads, 128);
        assert!(config.has_feature(LicenseFeature::PersistentMode));
        assert!(config.has_feature(LicenseFeature::SlaGuarantee));
    }

    #[test]
    fn test_feature_matrix_all_present() {
        let matrix = FeatureMatrix::build_table();
        assert!(matrix.len() > 0);
        // Verify at least one feature is present in enterprise
        let has_enterprise_features = matrix.iter().any(|(_, _, _, _, e)| *e);
        assert!(has_enterprise_features);
    }

    #[test]
    fn test_price_display() {
        let free = LicenseConfig::for_tier(LicenseTier::Trial);
        assert_eq!(free.price_display(), "Free");

        let pro = LicenseConfig::for_tier(LicenseTier::Pro);
        assert_eq!(pro.price_display(), "$1500.00");
    }
}
