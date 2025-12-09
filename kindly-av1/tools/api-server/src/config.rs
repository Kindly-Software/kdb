//! Server Configuration - Environment-based config with sensible defaults
//!
//! SOTA 2024-2025 Patterns:
//! - 12-factor app methodology (environment-based config)
//! - Graceful degradation (sensible defaults)
//! - Zero-trust security (validate all inputs)

use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;

/// Server configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Port to listen on (default: 8443)
    pub port: u16,

    /// Storage path for uploaded files and outputs
    pub storage_path: PathBuf,

    /// Database path for job tracking (SQLite)
    pub database_path: PathBuf,

    /// Max concurrent encoding jobs (default: CPU count)
    pub max_concurrent_jobs: usize,

    /// Rate limit: requests per IP per minute (default: 60)
    pub rate_limit_per_minute: u32,

    /// Max file upload size in bytes (default: 2GB)
    pub max_upload_size: u64,

    /// Job retention: days to keep completed jobs (default: 7)
    pub job_retention_days: u32,
}

impl ServerConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8443);

        let storage_path = env::var("STORAGE_PATH")
            .unwrap_or_else(|_| "/var/kindly-av1/storage".to_string())
            .into();

        let database_path = env::var("DATABASE_PATH")
            .unwrap_or_else(|_| "/var/kindly-av1/jobs.db".to_string())
            .into();

        let max_concurrent_jobs = env::var("MAX_CONCURRENT_JOBS")
            .ok()
            .and_then(|j| j.parse().ok())
            .unwrap_or_else(|| num_cpus::get());

        let rate_limit_per_minute = env::var("RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|r| r.parse().ok())
            .unwrap_or(60);

        let max_upload_size = env::var("MAX_UPLOAD_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2 * 1024 * 1024 * 1024); // 2GB default

        let job_retention_days = env::var("JOB_RETENTION_DAYS")
            .ok()
            .and_then(|d| d.parse().ok())
            .unwrap_or(7);

        Ok(Self {
            port,
            storage_path,
            database_path,
            max_concurrent_jobs,
            rate_limit_per_minute,
            max_upload_size,
            job_retention_days,
        })
    }

    /// Create default configuration for testing
    pub fn default() -> Self {
        Self {
            port: 8443,
            storage_path: PathBuf::from("/tmp/kindly-av1-test"),
            database_path: PathBuf::from(":memory:"),
            max_concurrent_jobs: 4,
            rate_limit_per_minute: 60,
            max_upload_size: 2 * 1024 * 1024 * 1024,
            job_retention_days: 7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.port, 8443);
        assert_eq!(config.max_concurrent_jobs, 4);
        assert_eq!(config.rate_limit_per_minute, 60);
    }

    #[test]
    fn test_env_override() {
        env::set_var("PORT", "9000");
        env::set_var("MAX_CONCURRENT_JOBS", "8");

        let config = ServerConfig::from_env().unwrap();
        assert_eq!(config.port, 9000);
        assert_eq!(config.max_concurrent_jobs, 8);

        env::remove_var("PORT");
        env::remove_var("MAX_CONCURRENT_JOBS");
    }
}
