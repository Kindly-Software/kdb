//! Configuration structures for Clapi proxy
//!
//! # UCE33 Q13: Interfaces
//! - TOML-based configuration (standard format)
//! - Environment variable overrides
//! - Serde deserialization

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{ClapiError, ClapiResult};

/// Proxy configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProxyConfig {
    /// Server listen address (e.g., "127.0.0.1:8080")
    pub listen_addr: String,

    /// Provider configurations
    pub providers: Vec<ProviderConfig>,

    /// Default budget for new users (cents)
    #[serde(default = "default_budget")]
    pub default_budget: i64,

    /// Audit log path
    pub audit_log_path: PathBuf,

    /// Request timeout (seconds)
    #[serde(default = "default_timeout")]
    pub request_timeout_secs: u64,

    /// Test mode: Use MockProvider instead of real providers
    ///
    /// When enabled:
    /// - All requests routed through MockProvider
    /// - No API calls to external services
    /// - Realistic latency simulation (~100ms)
    /// - Realistic token counting and cost calculations
    /// - No API keys required
    ///
    /// # CLI
    /// Set via `clapi start --test` flag
    #[serde(default)]
    pub test_mode: bool,

    /// PagerDuty routing key (optional - for critical alerts)
    #[serde(default)]
    pub pagerduty_token: Option<String>,

    /// Slack incoming webhook URL (optional - for team notifications)
    #[serde(default)]
    pub slack_webhook: Option<String>,

    /// Show configuration wizard on startup
    ///
    /// When enabled (default):
    /// - `clapi` launches with interactive wizard
    /// - Wizard transitions to main TUI dashboard after completion
    ///
    /// When disabled:
    /// - `clapi` launches main TUI dashboard directly
    /// - Skip wizard for power users with existing config
    ///
    /// # Toggle Options
    /// - Config file: `show_wizard_on_start = false`
    /// - TUI command palette: Press `/` → type `wizard off`
    /// - CLI override: `clapi --no-wizard` or `clapi --wizard`
    #[serde(default = "default_show_wizard")]
    pub show_wizard_on_start: bool,
}

fn default_budget() -> i64 {
    10_000 // $100.00 (10,000 cents)
}

fn default_show_wizard() -> bool {
    true // Always show wizard by default
}

fn default_timeout() -> u64 {
    30 // 30 seconds
}

/// Provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    /// Provider name (e.g., "openai", "anthropic")
    pub name: String,

    /// Base URL for API (e.g., "https://api.openai.com")
    pub base_url: String,

    /// API key
    pub api_key: String,

    /// Priority (0 = highest, 255 = lowest)
    #[serde(default)]
    pub priority: u8,

    /// Models supported by this provider
    #[serde(default)]
    pub models: Vec<String>,
}

impl ProxyConfig {
    /// Load configuration from TOML file
    ///
    /// # Examples
    /// ```no_run
    /// use clapi_core::proxy::ProxyConfig;
    ///
    /// let config = ProxyConfig::load("clapi.toml").unwrap();
    /// ```
    pub fn load<P: AsRef<Path>>(path: P) -> ClapiResult<Self> {
        let contents = fs::read_to_string(path.as_ref())
            .map_err(|e| ClapiError::ConfigError(format!("Failed to read config: {}", e)))?;

        let config: ProxyConfig = toml::from_str(&contents)
            .map_err(|e| ClapiError::ConfigError(format!("Failed to parse config: {}", e)))?;

        config.validate()?;

        Ok(config)
    }

    /// Validate configuration
    fn validate(&self) -> ClapiResult<()> {
        // In test mode, providers are optional
        if !self.test_mode {
            if self.providers.is_empty() {
                return Err(ClapiError::ConfigError("No providers configured".to_string()));
            }

            if self.providers.len() > 255 {
                return Err(ClapiError::ConfigError("Too many providers (max 255)".to_string()));
            }

            for provider in &self.providers {
                if provider.base_url.is_empty() {
                    return Err(ClapiError::ConfigError(format!(
                        "Provider {} has empty base_url",
                        provider.name
                    )));
                }

                if provider.api_key.is_empty() {
                    return Err(ClapiError::ConfigError(format!(
                        "Provider {} has empty api_key",
                        provider.name
                    )));
                }
            }
        }

        Ok(())
    }

    /// Get provider config by index
    pub fn get_provider(&self, index: usize) -> ClapiResult<&ProviderConfig> {
        self.providers
            .get(index)
            .ok_or(ClapiError::InvalidProviderId(index as u16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        assert_eq!(default_budget(), 10_000);
        assert_eq!(default_timeout(), 30);
    }

    #[test]
    fn test_validate_empty_providers() {
        let config = ProxyConfig {
            listen_addr: "127.0.0.1:8080".to_string(),
            providers: vec![],
            default_budget: 10_000,
            audit_log_path: PathBuf::from("/tmp/audit.log"),
            request_timeout_secs: 30,
            test_mode: false,
            pagerduty_token: None,
            slack_webhook: None,
            show_wizard_on_start: true,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_valid_config() {
        let config = ProxyConfig {
            listen_addr: "127.0.0.1:8080".to_string(),
            providers: vec![ProviderConfig {
                name: "test".to_string(),
                base_url: "https://api.test.com".to_string(),
                api_key: "test_key".to_string(),
                priority: 0,
                models: vec![],
            }],
            default_budget: 10_000,
            audit_log_path: PathBuf::from("/tmp/audit.log"),
            request_timeout_secs: 30,
            test_mode: false,
            pagerduty_token: None,
            slack_webhook: None,
            show_wizard_on_start: true,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_test_mode_empty_providers() {
        // In test mode, empty providers is OK
        let config = ProxyConfig {
            listen_addr: "127.0.0.1:8080".to_string(),
            providers: vec![],
            default_budget: 10_000,
            audit_log_path: PathBuf::from("/tmp/audit.log"),
            request_timeout_secs: 30,
            test_mode: true,
            pagerduty_token: None,
            slack_webhook: None,
            show_wizard_on_start: true,
        };

        assert!(config.validate().is_ok());
    }
}
