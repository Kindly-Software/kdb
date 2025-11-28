//! kindly-av1 CLI Metacapsule - T6 Mixed Tier
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Top-level metacapsule orchestrating all encoder sub-capsules for
//! the kindly-av1 CLI application.

use super::{EncoderConfig, EncoderWiringCapsule, EncoderSubCapsules};
use atomic_capsule::encoder::EncoderError;

/// kindly-av1 CLI metacapsule (T6 Mixed tier).
///
/// This is the top-level orchestrator connecting:
/// - Configuration (EncoderConfig)
/// - Wiring logic (EncoderWiringCapsule)
/// - Sub-capsules (EncoderSubCapsules)
#[repr(C, align(1024))]
pub struct KindlyAv1CliMetacapsule {
    /// Encoder configuration
    config: EncoderConfig,

    /// Wiring capsule for coordination
    wiring: EncoderWiringCapsule,

    /// All encoder sub-capsules
    sub_capsules: EncoderSubCapsules,

    /// Padding to 1024 bytes
    _padding: [u8; 1024 - 256], // Placeholder size
}

impl KindlyAv1CliMetacapsule {
    /// Create a new CLI metacapsule with given configuration.
    pub fn new(config: EncoderConfig) -> Result<Self, EncoderError> {
        config.validate()?;

        Ok(Self {
            config,
            wiring: EncoderWiringCapsule::new(),
            sub_capsules: EncoderSubCapsules::new(),
            _padding: [0u8; 1024 - 256],
        })
    }

    /// Get encoder configuration.
    pub fn config(&self) -> &EncoderConfig {
        &self.config
    }

    /// Get wiring capsule.
    pub fn wiring(&self) -> &EncoderWiringCapsule {
        &self.wiring
    }

    /// Get sub-capsules.
    pub fn sub_capsules(&self) -> &EncoderSubCapsules {
        &self.sub_capsules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metacapsule_creation() {
        let config = EncoderConfig::default();
        let metacapsule = KindlyAv1CliMetacapsule::new(config);
        assert!(metacapsule.is_ok());
    }

    #[test]
    fn test_metacapsule_alignment() {
        assert_eq!(core::mem::align_of::<KindlyAv1CliMetacapsule>(), 1024);
    }
}
