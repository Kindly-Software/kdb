//! Audio Decoder Capsules - Phase 10
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Production-ready audio decoders for common codecs found in video containers:
//!
//! ## Supported Codecs
//!
//! - **AAC** (Advanced Audio Coding) - Most common in MP4/M4A
//!   - AAC-LC (Low Complexity)
//!   - HE-AAC v1/v2 (SBR/PS)
//!   - ADTS/raw frame parsing
//!
//! - **Opus** - Modern codec for WebM/MKV
//!   - SILK (speech)
//!   - CELT (audio)
//!   - Hybrid mode
//!
//! - **FLAC** - Lossless codec for MKV
//!   - LPC prediction (orders 1-32)
//!   - Fixed predictors
//!   - Rice coding
//!
//! - **Vorbis** - Legacy codec for WebM/OGG
//!   - MDCT transform
//!   - Floor/residue decoding
//!   - Codebook lookup
//!
//! ## Architecture
//!
//! Each codec has two capsules:
//! - **Bitstream Capsule** (T1 Atomic): Frame/packet parsing, sync detection
//! - **Decoder Capsule** (T2 SIMD): DSP operations, sample generation
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T1/T2 tiers, Q33 lockfree atomics
//! - **Chaos**: 100% lockfree, 256B/512B cache-aligned capsules
//! - **ASSUM**: All unsafe documented with #ASSUME/#VERIFY
//! - **T28**: 5-tier testing (unit/property/integration/production/determinism)
//! - **B32**: Criterion benchmarks with 95% CI

#![allow(dead_code)]

// AAC (Advanced Audio Coding)
#[cfg(feature = "audio-aac")]
pub mod aac_bitstream;
#[cfg(feature = "audio-aac")]
pub mod aac_decoder;

// Opus
#[cfg(feature = "audio-opus")]
pub mod opus_bitstream;
#[cfg(feature = "audio-opus")]
pub mod opus_decoder;

// FLAC (Free Lossless Audio Codec)
#[cfg(feature = "audio-flac")]
pub mod flac_bitstream;
#[cfg(feature = "audio-flac")]
pub mod flac_decoder;

// Vorbis
#[cfg(feature = "audio-vorbis")]
pub mod vorbis_bitstream;
#[cfg(feature = "audio-vorbis")]
pub mod vorbis_decoder;

// Re-exports
#[cfg(feature = "audio-aac")]
pub use aac_bitstream::*;
#[cfg(feature = "audio-aac")]
pub use aac_decoder::*;

#[cfg(feature = "audio-opus")]
pub use opus_bitstream::*;
#[cfg(feature = "audio-opus")]
pub use opus_decoder::*;

#[cfg(feature = "audio-flac")]
pub use flac_bitstream::*;
#[cfg(feature = "audio-flac")]
pub use flac_decoder::*;

#[cfg(feature = "audio-vorbis")]
pub use vorbis_bitstream::*;
#[cfg(feature = "audio-vorbis")]
pub use vorbis_decoder::*;

/// Common audio sample formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SampleFormat {
    /// 16-bit signed PCM (most common)
    S16 = 0,
    /// 32-bit signed PCM
    S32 = 1,
    /// 32-bit floating point
    F32 = 2,
    /// Planar 16-bit (separate channel buffers)
    S16Planar = 3,
    /// Planar 32-bit float
    F32Planar = 4,
}

/// Common sample rates in Hz
pub const SAMPLE_RATE_8000: u32 = 8000;
pub const SAMPLE_RATE_16000: u32 = 16000;
pub const SAMPLE_RATE_22050: u32 = 22050;
pub const SAMPLE_RATE_44100: u32 = 44100;
pub const SAMPLE_RATE_48000: u32 = 48000;
pub const SAMPLE_RATE_96000: u32 = 96000;

/// Channel configurations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChannelLayout {
    /// Mono (1 channel)
    Mono = 1,
    /// Stereo (2 channels: L, R)
    Stereo = 2,
    /// 2.1 (3 channels: L, R, LFE)
    Surround21 = 3,
    /// Quadraphonic (4 channels: FL, FR, RL, RR)
    Quad = 4,
    /// 5.0 (5 channels: FL, FR, FC, RL, RR)
    Surround50 = 5,
    /// 5.1 (6 channels: FL, FR, FC, LFE, RL, RR)
    Surround51 = 6,
    /// 7.1 (8 channels: FL, FR, FC, LFE, RL, RR, SL, SR)
    Surround71 = 8,
}

impl ChannelLayout {
    /// Get number of channels
    pub const fn channels(&self) -> u8 {
        *self as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_format() {
        assert_eq!(SampleFormat::S16 as u8, 0);
        assert_eq!(SampleFormat::F32 as u8, 2);
    }

    #[test]
    fn test_channel_layout() {
        assert_eq!(ChannelLayout::Mono.channels(), 1);
        assert_eq!(ChannelLayout::Stereo.channels(), 2);
        assert_eq!(ChannelLayout::Surround51.channels(), 6);
        assert_eq!(ChannelLayout::Surround71.channels(), 8);
    }

    #[test]
    fn test_sample_rates() {
        assert_eq!(SAMPLE_RATE_44100, 44100);
        assert_eq!(SAMPLE_RATE_48000, 48000);
    }
}
