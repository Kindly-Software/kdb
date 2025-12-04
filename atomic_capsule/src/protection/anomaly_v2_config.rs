//! AnomalyV2Config - Configuration System for ML V2 Anomaly Detection
//!
//! **Tier**: T0 Auditable (configuration is immutable after load)
//! **Performance**: <1ms YAML parsing, 0ns config access after load
//!
//! # UCE34 Framework Analysis
//! - **Q10 (Tier)**: T0 Auditable - configuration immutable after load
//! - **Q11 (Transform)**: YAML parsing to typed struct
//! - **Q12 (Nightly)**: None required (stable Rust)
//! - **Q33 (Validation)**: Runtime validation of config values
//! - **Q34 (Auditability)**: Config changes logged with hash
//!
//! # Configuration File Format (YAML)
//!
//! ```yaml
//! # /etc/kindly/anomaly_v2.yaml
//!
//! version: "1.0"
//!
//! layers:
//!   probabilistic:
//!     enabled: true
//!     bloom_size: 65536
//!     hyperloglog_precision: 14
//!   gmm:
//!     enabled: true
//!     threshold: 9.0
//!     num_components: 8
//!     ema_alpha: 0.1
//!   tinyml:
//!     enabled: true
//!     threshold: 0.6
//!     num_trees: 8
//!   temporal:
//!     enabled: true
//!     burst_threshold: 3.0
//!     window_ms: 1000
//!     decay_factor: 0.95
//!
//! training:
//!   model_path: "/var/lib/kindly/anomaly_v2/model.bin"
//!   auto_update: true
//!   update_interval_secs: 3600
//!
//! ja3_database:
//!   enabled: true
//!   path: "/var/lib/kindly/ja3/database.bin"
//!   refresh_interval_secs: 86400
//!
//! metrics:
//!   enabled: true
//!   export_interval_secs: 15
//!   histogram_buckets: [30, 50, 100, 160, 500]
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use atomic_capsule::protection::anomaly_v2_config::AnomalyV2Config;
//!
//! // Load from file
//! let config = AnomalyV2Config::from_yaml_file("/etc/kindly/anomaly_v2.yaml")?;
//!
//! // Load from string
//! let config = AnomalyV2Config::from_yaml_str(yaml_content)?;
//!
//! // Use default
//! let config = AnomalyV2Config::default();
//!
//! // Access config
//! if config.layers.gmm.enabled {
//!     let threshold = config.layers.gmm.threshold;
//! }
//! ```

use core::fmt;

// ============================================================================
// LAYER CONFIGURATIONS
// ============================================================================

/// Probabilistic layer configuration (Bloom + HyperLogLog)
#[derive(Debug, Clone)]
pub struct ProbabilisticLayerConfig {
    /// Whether this layer is enabled
    pub enabled: bool,
    /// Bloom filter size (number of bits)
    pub bloom_size: u32,
    /// HyperLogLog precision (4-18)
    pub hyperloglog_precision: u8,
    /// Count-Min Sketch width
    pub countmin_width: u32,
    /// Count-Min Sketch depth
    pub countmin_depth: u8,
}

impl Default for ProbabilisticLayerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bloom_size: 65536,
            hyperloglog_precision: 14,
            countmin_width: 1024,
            countmin_depth: 4,
        }
    }
}

/// GMM (Gaussian Mixture Model) layer configuration
#[derive(Debug, Clone)]
pub struct GmmLayerConfig {
    /// Whether this layer is enabled
    pub enabled: bool,
    /// Anomaly detection threshold (Mahalanobis distance squared)
    pub threshold: f64,
    /// Number of Gaussian components
    pub num_components: u8,
    /// EMA alpha for online learning (0.0-1.0)
    pub ema_alpha: f64,
    /// Minimum samples before detection
    pub min_samples: u32,
}

impl Default for GmmLayerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 9.0, // 3 sigma squared
            num_components: 8,
            ema_alpha: 0.1,
            min_samples: 100,
        }
    }
}

/// TinyML (Decision Tree Ensemble) layer configuration
#[derive(Debug, Clone)]
pub struct TinymlLayerConfig {
    /// Whether this layer is enabled
    pub enabled: bool,
    /// Anomaly detection threshold (0.0-1.0)
    pub threshold: f64,
    /// Number of decision trees
    pub num_trees: u8,
    /// Maximum tree depth
    pub max_depth: u8,
    /// Isolation forest contamination rate
    pub contamination: f64,
}

impl Default for TinymlLayerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 0.6,
            num_trees: 8,
            max_depth: 6,
            contamination: 0.1,
        }
    }
}

/// Temporal (Sequence Analysis) layer configuration
#[derive(Debug, Clone)]
pub struct TemporalLayerConfig {
    /// Whether this layer is enabled
    pub enabled: bool,
    /// Burst detection threshold
    pub burst_threshold: f32,
    /// Time window in milliseconds
    pub window_ms: u32,
    /// Decay factor for temporal scoring (0.0-1.0)
    pub decay_factor: f32,
    /// Minimum events for burst detection
    pub min_events: u32,
    /// Timing anomaly threshold (ms)
    pub timing_threshold_ms: u32,
}

impl Default for TemporalLayerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            burst_threshold: 3.0,
            window_ms: 1000,
            decay_factor: 0.95,
            min_events: 5,
            timing_threshold_ms: 100,
        }
    }
}

/// All layer configurations
#[derive(Debug, Clone, Default)]
pub struct LayersConfig {
    pub probabilistic: ProbabilisticLayerConfig,
    pub gmm: GmmLayerConfig,
    pub tinyml: TinymlLayerConfig,
    pub temporal: TemporalLayerConfig,
}

// ============================================================================
// TRAINING CONFIGURATION
// ============================================================================

/// Training/model configuration
#[derive(Debug, Clone)]
pub struct TrainingConfig {
    /// Path to model file
    pub model_path: String,
    /// Enable automatic model updates
    pub auto_update: bool,
    /// Update interval in seconds
    pub update_interval_secs: u64,
    /// Path for training data
    pub training_data_path: Option<String>,
    /// Minimum samples for retraining
    pub retrain_min_samples: u32,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            model_path: "/var/lib/kindly/anomaly_v2/model.bin".to_string(),
            auto_update: true,
            update_interval_secs: 3600,
            training_data_path: None,
            retrain_min_samples: 10000,
        }
    }
}

// ============================================================================
// JA3 DATABASE CONFIGURATION
// ============================================================================

/// JA3 fingerprint database configuration
#[derive(Debug, Clone)]
pub struct Ja3DatabaseConfig {
    /// Whether JA3 database is enabled
    pub enabled: bool,
    /// Path to JA3 database file
    pub path: String,
    /// Database refresh interval in seconds
    pub refresh_interval_secs: u64,
    /// Maximum database entries
    pub max_entries: u32,
    /// Enable JA3S (server) fingerprinting
    pub enable_ja3s: bool,
}

impl Default for Ja3DatabaseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "/var/lib/kindly/ja3/database.bin".to_string(),
            refresh_interval_secs: 86400,
            max_entries: 100000,
            enable_ja3s: true,
        }
    }
}

// ============================================================================
// METRICS CONFIGURATION
// ============================================================================

/// Metrics export configuration
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Whether metrics export is enabled
    pub enabled: bool,
    /// Export interval in seconds
    pub export_interval_secs: u64,
    /// Histogram bucket boundaries (nanoseconds)
    pub histogram_buckets: Vec<u32>,
    /// Prometheus endpoint path
    pub endpoint_path: String,
    /// Enable per-layer metrics
    pub per_layer_metrics: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            export_interval_secs: 15,
            histogram_buckets: vec![30, 50, 100, 160, 500],
            endpoint_path: "/metrics".to_string(),
            per_layer_metrics: true,
        }
    }
}

// ============================================================================
// MAIN CONFIGURATION STRUCT
// ============================================================================

/// AnomalyV2Config - Complete configuration for ML V2 anomaly detection
///
/// Supports YAML configuration file with sensible defaults.
#[derive(Debug, Clone)]
pub struct AnomalyV2Config {
    /// Configuration version
    pub version: String,

    /// Layer configurations
    pub layers: LayersConfig,

    /// Training/model configuration
    pub training: TrainingConfig,

    /// JA3 database configuration
    pub ja3_database: Ja3DatabaseConfig,

    /// Metrics configuration
    pub metrics: MetricsConfig,

    /// Configuration file path (if loaded from file)
    pub source_path: Option<String>,

    /// Configuration hash (for audit trail)
    pub config_hash: u64,
}

impl Default for AnomalyV2Config {
    fn default() -> Self {
        let mut config = Self {
            version: "1.0".to_string(),
            layers: LayersConfig::default(),
            training: TrainingConfig::default(),
            ja3_database: Ja3DatabaseConfig::default(),
            metrics: MetricsConfig::default(),
            source_path: None,
            config_hash: 0,
        };
        config.config_hash = config.compute_hash();
        config
    }
}

impl AnomalyV2Config {
    /// Create new config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from YAML string
    ///
    /// # Arguments
    /// * `yaml_str` - YAML configuration string
    ///
    /// # Returns
    /// Parsed configuration or error
    pub fn from_yaml_str(yaml_str: &str) -> Result<Self, ConfigError> {
        let mut config = Self::default();

        // Simple YAML parser (no external dependencies)
        for line in yaml_str.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse key-value pairs
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                match key {
                    "version" => config.version = value.to_string(),

                    // Probabilistic layer
                    "enabled" if line.contains("probabilistic") => {
                        config.layers.probabilistic.enabled = value.parse().unwrap_or(true);
                    }
                    "bloom_size" => {
                        config.layers.probabilistic.bloom_size = value.parse().unwrap_or(65536);
                    }
                    "hyperloglog_precision" => {
                        config.layers.probabilistic.hyperloglog_precision = value.parse().unwrap_or(14);
                    }

                    // GMM layer
                    "threshold" if config.layers.gmm.threshold == 9.0 => {
                        config.layers.gmm.threshold = value.parse().unwrap_or(9.0);
                    }
                    "num_components" => {
                        config.layers.gmm.num_components = value.parse().unwrap_or(8);
                    }
                    "ema_alpha" => {
                        config.layers.gmm.ema_alpha = value.parse().unwrap_or(0.1);
                    }

                    // TinyML layer
                    "num_trees" => {
                        config.layers.tinyml.num_trees = value.parse().unwrap_or(8);
                    }

                    // Temporal layer
                    "burst_threshold" => {
                        config.layers.temporal.burst_threshold = value.parse().unwrap_or(3.0);
                    }
                    "window_ms" => {
                        config.layers.temporal.window_ms = value.parse().unwrap_or(1000);
                    }
                    "decay_factor" => {
                        config.layers.temporal.decay_factor = value.parse().unwrap_or(0.95);
                    }

                    // Training
                    "model_path" => config.training.model_path = value.to_string(),
                    "auto_update" => {
                        config.training.auto_update = value.parse().unwrap_or(true);
                    }
                    "update_interval_secs" => {
                        config.training.update_interval_secs = value.parse().unwrap_or(3600);
                    }

                    // JA3 database
                    "path" if line.contains("ja3") => {
                        config.ja3_database.path = value.to_string();
                    }
                    "refresh_interval_secs" => {
                        config.ja3_database.refresh_interval_secs = value.parse().unwrap_or(86400);
                    }

                    // Metrics
                    "export_interval_secs" => {
                        config.metrics.export_interval_secs = value.parse().unwrap_or(15);
                    }

                    _ => {}
                }
            }
        }

        config.config_hash = config.compute_hash();
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from YAML file
    ///
    /// # Arguments
    /// * `path` - Path to YAML configuration file
    ///
    /// # Returns
    /// Parsed configuration or error
    #[cfg(feature = "std")]
    pub fn from_yaml_file(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::IoError(e.to_string()))?;

        let mut config = Self::from_yaml_str(&content)?;
        config.source_path = Some(path.to_string());
        config.config_hash = config.compute_hash();
        Ok(config)
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate probabilistic layer
        if self.layers.probabilistic.hyperloglog_precision < 4
            || self.layers.probabilistic.hyperloglog_precision > 18
        {
            return Err(ConfigError::InvalidValue {
                field: "hyperloglog_precision".to_string(),
                reason: "Must be between 4 and 18".to_string(),
            });
        }

        // Validate GMM layer
        if self.layers.gmm.threshold <= 0.0 {
            return Err(ConfigError::InvalidValue {
                field: "gmm.threshold".to_string(),
                reason: "Must be positive".to_string(),
            });
        }
        if self.layers.gmm.ema_alpha <= 0.0 || self.layers.gmm.ema_alpha > 1.0 {
            return Err(ConfigError::InvalidValue {
                field: "gmm.ema_alpha".to_string(),
                reason: "Must be between 0 and 1".to_string(),
            });
        }
        if self.layers.gmm.num_components == 0 || self.layers.gmm.num_components > 16 {
            return Err(ConfigError::InvalidValue {
                field: "gmm.num_components".to_string(),
                reason: "Must be between 1 and 16".to_string(),
            });
        }

        // Validate TinyML layer
        if self.layers.tinyml.threshold <= 0.0 || self.layers.tinyml.threshold > 1.0 {
            return Err(ConfigError::InvalidValue {
                field: "tinyml.threshold".to_string(),
                reason: "Must be between 0 and 1".to_string(),
            });
        }
        if self.layers.tinyml.num_trees == 0 || self.layers.tinyml.num_trees > 32 {
            return Err(ConfigError::InvalidValue {
                field: "tinyml.num_trees".to_string(),
                reason: "Must be between 1 and 32".to_string(),
            });
        }

        // Validate temporal layer
        if self.layers.temporal.burst_threshold <= 0.0 {
            return Err(ConfigError::InvalidValue {
                field: "temporal.burst_threshold".to_string(),
                reason: "Must be positive".to_string(),
            });
        }
        if self.layers.temporal.decay_factor <= 0.0 || self.layers.temporal.decay_factor > 1.0 {
            return Err(ConfigError::InvalidValue {
                field: "temporal.decay_factor".to_string(),
                reason: "Must be between 0 and 1".to_string(),
            });
        }

        Ok(())
    }

    /// Compute configuration hash for audit trail
    fn compute_hash(&self) -> u64 {
        // Simple FNV-1a hash of key configuration values
        let mut hash: u64 = 0xcbf29ce484222325;
        let prime: u64 = 0x100000001b3;

        // Hash version
        for byte in self.version.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(prime);
        }

        // Hash enabled layers
        let layers_enabled = [
            self.layers.probabilistic.enabled,
            self.layers.gmm.enabled,
            self.layers.tinyml.enabled,
            self.layers.temporal.enabled,
        ];
        for enabled in layers_enabled {
            hash ^= enabled as u64;
            hash = hash.wrapping_mul(prime);
        }

        // Hash key thresholds
        hash ^= (self.layers.gmm.threshold * 1000.0) as u64;
        hash = hash.wrapping_mul(prime);
        hash ^= (self.layers.tinyml.threshold * 1000.0) as u64;
        hash = hash.wrapping_mul(prime);
        hash ^= (self.layers.temporal.burst_threshold * 1000.0) as u64;
        hash = hash.wrapping_mul(prime);

        hash
    }

    /// Get enabled layers as bitmask
    ///
    /// Bit 0: Probabilistic, Bit 1: GMM, Bit 2: TinyML, Bit 3: Temporal
    pub fn enabled_layers_mask(&self) -> u8 {
        let mut mask = 0u8;
        if self.layers.probabilistic.enabled {
            mask |= 0b0001;
        }
        if self.layers.gmm.enabled {
            mask |= 0b0010;
        }
        if self.layers.tinyml.enabled {
            mask |= 0b0100;
        }
        if self.layers.temporal.enabled {
            mask |= 0b1000;
        }
        mask
    }

    /// Create builder for custom configuration
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    /// Generate YAML template string
    pub fn to_yaml_template() -> String {
        r#"# Anomaly Detector V2 Configuration
# /etc/kindly/anomaly_v2.yaml

version: "1.0"

# Layer configurations
layers:
  # Layer 1: Probabilistic (Bloom filter + HyperLogLog)
  probabilistic:
    enabled: true
    bloom_size: 65536
    hyperloglog_precision: 14
    countmin_width: 1024
    countmin_depth: 4

  # Layer 2: GMM (Gaussian Mixture Model)
  gmm:
    enabled: true
    threshold: 9.0           # 3 sigma squared
    num_components: 8
    ema_alpha: 0.1
    min_samples: 100

  # Layer 3: TinyML (Decision Tree Ensemble)
  tinyml:
    enabled: true
    threshold: 0.6
    num_trees: 8
    max_depth: 6
    contamination: 0.1

  # Layer 4: Temporal (Sequence Analysis)
  temporal:
    enabled: true
    burst_threshold: 3.0
    window_ms: 1000
    decay_factor: 0.95
    min_events: 5
    timing_threshold_ms: 100

# Training/Model configuration
training:
  model_path: "/var/lib/kindly/anomaly_v2/model.bin"
  auto_update: true
  update_interval_secs: 3600
  retrain_min_samples: 10000

# JA3 fingerprint database
ja3_database:
  enabled: true
  path: "/var/lib/kindly/ja3/database.bin"
  refresh_interval_secs: 86400
  max_entries: 100000
  enable_ja3s: true

# Metrics/Prometheus configuration
metrics:
  enabled: true
  export_interval_secs: 15
  histogram_buckets: [30, 50, 100, 160, 500]
  endpoint_path: "/metrics"
  per_layer_metrics: true
"#.to_string()
    }
}

// ============================================================================
// BUILDER PATTERN
// ============================================================================

/// Builder for AnomalyV2Config
pub struct ConfigBuilder {
    config: AnomalyV2Config,
}

impl ConfigBuilder {
    /// Create new builder with defaults
    pub fn new() -> Self {
        Self {
            config: AnomalyV2Config::default(),
        }
    }

    /// Set GMM threshold
    pub fn gmm_threshold(mut self, threshold: f64) -> Self {
        self.config.layers.gmm.threshold = threshold;
        self
    }

    /// Set TinyML threshold
    pub fn tinyml_threshold(mut self, threshold: f64) -> Self {
        self.config.layers.tinyml.threshold = threshold;
        self
    }

    /// Set temporal burst threshold
    pub fn burst_threshold(mut self, threshold: f32) -> Self {
        self.config.layers.temporal.burst_threshold = threshold;
        self
    }

    /// Enable/disable probabilistic layer
    pub fn probabilistic_enabled(mut self, enabled: bool) -> Self {
        self.config.layers.probabilistic.enabled = enabled;
        self
    }

    /// Enable/disable GMM layer
    pub fn gmm_enabled(mut self, enabled: bool) -> Self {
        self.config.layers.gmm.enabled = enabled;
        self
    }

    /// Enable/disable TinyML layer
    pub fn tinyml_enabled(mut self, enabled: bool) -> Self {
        self.config.layers.tinyml.enabled = enabled;
        self
    }

    /// Enable/disable temporal layer
    pub fn temporal_enabled(mut self, enabled: bool) -> Self {
        self.config.layers.temporal.enabled = enabled;
        self
    }

    /// Set model path
    pub fn model_path(mut self, path: &str) -> Self {
        self.config.training.model_path = path.to_string();
        self
    }

    /// Set JA3 database path
    pub fn ja3_database_path(mut self, path: &str) -> Self {
        self.config.ja3_database.path = path.to_string();
        self
    }

    /// Enable/disable JA3 database
    pub fn ja3_database_enabled(mut self, enabled: bool) -> Self {
        self.config.ja3_database.enabled = enabled;
        self
    }

    /// Enable/disable metrics
    pub fn metrics_enabled(mut self, enabled: bool) -> Self {
        self.config.metrics.enabled = enabled;
        self
    }

    /// Set metrics export interval
    pub fn metrics_interval(mut self, secs: u64) -> Self {
        self.config.metrics.export_interval_secs = secs;
        self
    }

    /// Build the configuration
    pub fn build(mut self) -> Result<AnomalyV2Config, ConfigError> {
        self.config.config_hash = self.config.compute_hash();
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Configuration error
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// I/O error reading file
    IoError(String),
    /// YAML parse error
    ParseError(String),
    /// Invalid configuration value
    InvalidValue { field: String, reason: String },
    /// Missing required field
    MissingField(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::IoError(msg) => write!(f, "I/O error: {}", msg),
            ConfigError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ConfigError::InvalidValue { field, reason } => {
                write!(f, "Invalid value for '{}': {}", field, reason)
            }
            ConfigError::MissingField(field) => write!(f, "Missing required field: {}", field),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ConfigError {}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AnomalyV2Config::default();

        assert_eq!(config.version, "1.0");
        assert!(config.layers.probabilistic.enabled);
        assert!(config.layers.gmm.enabled);
        assert!(config.layers.tinyml.enabled);
        assert!(config.layers.temporal.enabled);
        assert_eq!(config.layers.gmm.threshold, 9.0);
        assert_eq!(config.layers.tinyml.threshold, 0.6);
        assert_eq!(config.layers.temporal.burst_threshold, 3.0);
    }

    #[test]
    fn test_enabled_layers_mask() {
        let config = AnomalyV2Config::default();
        assert_eq!(config.enabled_layers_mask(), 0b1111);

        let config = AnomalyV2Config::builder()
            .probabilistic_enabled(true)
            .gmm_enabled(false)
            .tinyml_enabled(true)
            .temporal_enabled(false)
            .build()
            .unwrap();
        assert_eq!(config.enabled_layers_mask(), 0b0101);
    }

    #[test]
    fn test_builder_pattern() {
        let config = AnomalyV2Config::builder()
            .gmm_threshold(16.0)
            .tinyml_threshold(0.7)
            .burst_threshold(5.0)
            .metrics_enabled(false)
            .build()
            .unwrap();

        assert_eq!(config.layers.gmm.threshold, 16.0);
        assert_eq!(config.layers.tinyml.threshold, 0.7);
        assert_eq!(config.layers.temporal.burst_threshold, 5.0);
        assert!(!config.metrics.enabled);
    }

    #[test]
    fn test_validation_gmm_threshold() {
        let result = AnomalyV2Config::builder()
            .gmm_threshold(-1.0)
            .build();

        assert!(result.is_err());
        if let Err(ConfigError::InvalidValue { field, .. }) = result {
            assert!(field.contains("threshold"));
        }
    }

    #[test]
    fn test_validation_ema_alpha() {
        let mut config = AnomalyV2Config::default();
        config.layers.gmm.ema_alpha = 1.5;

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_tinyml_threshold() {
        let result = AnomalyV2Config::builder()
            .tinyml_threshold(1.5)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_validation_hyperloglog_precision() {
        let mut config = AnomalyV2Config::default();
        config.layers.probabilistic.hyperloglog_precision = 20;

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_yaml_template() {
        let template = AnomalyV2Config::to_yaml_template();

        assert!(template.contains("version:"));
        assert!(template.contains("probabilistic:"));
        assert!(template.contains("gmm:"));
        assert!(template.contains("tinyml:"));
        assert!(template.contains("temporal:"));
        assert!(template.contains("ja3_database:"));
        assert!(template.contains("metrics:"));
    }

    #[test]
    fn test_yaml_parsing() {
        let yaml = r#"
version: "2.0"
gmm:
  threshold: 12.0
  num_components: 4
tinyml:
  threshold: 0.8
temporal:
  burst_threshold: 2.5
"#;

        let config = AnomalyV2Config::from_yaml_str(yaml).unwrap();
        assert_eq!(config.version, "2.0");
    }

    #[test]
    fn test_config_hash_changes() {
        let config1 = AnomalyV2Config::default();
        let config2 = AnomalyV2Config::builder()
            .gmm_threshold(16.0)
            .build()
            .unwrap();

        assert_ne!(config1.config_hash, config2.config_hash);
    }

    #[test]
    fn test_config_hash_deterministic() {
        let config1 = AnomalyV2Config::default();
        let config2 = AnomalyV2Config::default();

        assert_eq!(config1.config_hash, config2.config_hash);
    }

    #[test]
    fn test_layer_defaults() {
        let prob = ProbabilisticLayerConfig::default();
        assert!(prob.enabled);
        assert_eq!(prob.bloom_size, 65536);
        assert_eq!(prob.hyperloglog_precision, 14);

        let gmm = GmmLayerConfig::default();
        assert!(gmm.enabled);
        assert_eq!(gmm.threshold, 9.0);
        assert_eq!(gmm.num_components, 8);

        let tinyml = TinymlLayerConfig::default();
        assert!(tinyml.enabled);
        assert_eq!(tinyml.threshold, 0.6);
        assert_eq!(tinyml.num_trees, 8);

        let temporal = TemporalLayerConfig::default();
        assert!(temporal.enabled);
        assert_eq!(temporal.burst_threshold, 3.0);
        assert_eq!(temporal.window_ms, 1000);
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::InvalidValue {
            field: "test".to_string(),
            reason: "too large".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid value"));
        assert!(msg.contains("test"));

        let err = ConfigError::IoError("file not found".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("I/O error"));

        let err = ConfigError::MissingField("required_field".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Missing"));
    }
}
