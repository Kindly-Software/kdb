//! Model Loading and Architecture Support
//!
//! Supports:
//! - Llama (7B, 13B, 70B, 405B)
//! - Mistral (7B, Mixtral 8×7B)
//! - Qwen (7B, 14B, 72B)
//! - Gemma (2B, 7B)
//!
//! Format: Safetensors (recommended), GGUF (fallback)

use crate::error::Result;

/// Model architecture types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    /// Llama architecture (Meta)
    Llama,
    /// Mistral architecture (Mistral AI)
    Mistral,
    /// Qwen architecture (Alibaba)
    Qwen,
    /// Gemma architecture (Google)
    Gemma,
}

/// Model configuration
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Model architecture
    pub architecture: Architecture,
    /// Hidden size (embedding dimension)
    pub hidden_size: usize,
    /// Number of layers
    pub num_layers: usize,
    /// Number of attention heads
    pub num_heads: usize,
    /// Vocabulary size
    pub vocab_size: usize,
    /// Maximum sequence length
    pub max_seq_len: usize,
}

/// Model weights and configuration
#[derive(Debug)]
pub struct Model {
    config: ModelConfig,
    // Weights will be added in Phase 1
}

impl Model {
    /// Load model from Safetensors file
    pub fn from_safetensors(_path: &str) -> Result<Self> {
        // To be implemented in Phase 1 (Month 6)
        unimplemented!("Model loading will be implemented in Phase 1")
    }

    /// Get model configuration
    pub fn config(&self) -> &ModelConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architecture_types() {
        let arch = Architecture::Llama;
        assert_eq!(arch, Architecture::Llama);
    }
}
