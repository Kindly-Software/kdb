//! Error types for Kindly Inference Engine

use std::fmt;

/// Result type alias for Kindly Inference
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during inference
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Model loading errors
    #[error("Failed to load model: {0}")]
    ModelLoad(String),

    /// Quantization errors
    #[error("Quantization error: {0}")]
    Quantization(String),

    /// Inference errors
    #[error("Inference error: {0}")]
    Inference(String),

    /// Hardware detection errors
    #[error("Hardware detection error: {0}")]
    Hardware(String),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Out of memory errors
    #[error("Out of memory: requested {requested_gb}GB, available {available_gb}GB")]
    OutOfMemory {
        /// Requested memory in GB
        requested_gb: usize,
        /// Available memory in GB
        available_gb: usize,
    },

    /// Invalid model architecture
    #[error("Invalid model architecture: {0}")]
    InvalidArchitecture(String),

    /// Unsupported feature
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),
}

impl Error {
    /// Create a model loading error
    pub fn model_load(msg: impl fmt::Display) -> Self {
        Self::ModelLoad(msg.to_string())
    }

    /// Create a quantization error
    pub fn quantization(msg: impl fmt::Display) -> Self {
        Self::Quantization(msg.to_string())
    }

    /// Create an inference error
    pub fn inference(msg: impl fmt::Display) -> Self {
        Self::Inference(msg.to_string())
    }

    /// Create a hardware detection error
    pub fn hardware(msg: impl fmt::Display) -> Self {
        Self::Hardware(msg.to_string())
    }

    /// Create a configuration error
    pub fn config(msg: impl fmt::Display) -> Self {
        Self::Config(msg.to_string())
    }

    /// Create an invalid architecture error
    pub fn invalid_architecture(msg: impl fmt::Display) -> Self {
        Self::InvalidArchitecture(msg.to_string())
    }

    /// Create an unsupported feature error
    pub fn unsupported_feature(msg: impl fmt::Display) -> Self {
        Self::UnsupportedFeature(msg.to_string())
    }
}
