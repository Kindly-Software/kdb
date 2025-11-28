//! Encoder Wiring Capsule - T6 Metacapsule Orchestration
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! This module provides the T6 Mixed tier metacapsule that orchestrates the complete AV1 encoding
//! pipeline via atomic_capsule encoder primitives.

use core::sync::atomic::{AtomicU64, Ordering};

use super::sub_capsules::EncoderSubCapsules;
use super::{EncoderError, FrameType, ObuType};

/// Wiring state enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WiringState {
    Uninitialized = 0,
    Ready = 1,
    Encoding = 2,
    Finalized = 3,
}

/// Encoder wiring statistics
#[derive(Debug, Clone)]
pub struct EncoderWiringStats {
    pub frames_encoded: u64,
    pub bytes_output: u64,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
    pub crf: u8,
    pub speed: u8,
    pub state: WiringState,
}

/// Encoder wiring capsule for T6 metacapsule orchestration (128B cache-aligned)
#[repr(C, align(128))]
pub struct EncoderWiringCapsule {
    frame_count: AtomicU64,
    bytes_output: AtomicU64,
    generation: AtomicU64,
    state: AtomicU64, // WiringState as u64
    width: u32,
    height: u32,
    crf: u8,
    speed: u8,
    _padding: [u8; 128 - 40], // 128 - (8*4 + 4*2 + 1*2) = 88
}

impl EncoderWiringCapsule {
    pub const fn new() -> Self {
        Self {
            frame_count: AtomicU64::new(0),
            bytes_output: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            state: AtomicU64::new(WiringState::Uninitialized as u64),
            width: 0,
            height: 0,
            crf: 0,
            speed: 0,
            _padding: [0u8; 88],
        }
    }

    pub fn initialize(
        &mut self,
        width: u32,
        height: u32,
        crf: u8,
        speed: u8,
    ) -> Result<EncoderSubCapsules, String> {
        // Store configuration
        unsafe {
            let width_ptr = &self.width as *const u32 as *mut u32;
            let height_ptr = &self.height as *const u32 as *mut u32;
            let crf_ptr = &self.crf as *const u8 as *mut u8;
            let speed_ptr = &self.speed as *const u8 as *mut u8;

            *width_ptr = width;
            *height_ptr = height;
            *crf_ptr = crf;
            *speed_ptr = speed;
        }

        // Transition to Ready
        self.state.store(WiringState::Ready as u64, Ordering::Release);

        Ok(EncoderSubCapsules::new())
    }

    pub fn encode_frame(
        &self,
        yuv_data: &[u8],
        sub_capsules: &EncoderSubCapsules,
    ) -> Result<Vec<u8>, String> {
        // Get current frame
        let frame_num = self.frame_count.load(Ordering::Acquire);
        let is_key_frame = frame_num == 0;

        // Update state to Encoding
        if frame_num == 0 {
            self.state.store(WiringState::Encoding as u64, Ordering::Release);
        }

        let mut output = Vec::with_capacity(yuv_data.len() / 2);

        // Write sequence header (first frame)
        if is_key_frame {
            let seq_header = sub_capsules.bitstream().write_sequence_header(0, 0);
            output.extend_from_slice(&seq_header);
        }

        // Write frame header
        let frame_type = if is_key_frame {
            FrameType::KeyFrame
        } else {
            FrameType::InterFrame
        };
        let frame_header = sub_capsules.bitstream().write_frame_header(
            frame_type,
            self.width as u16,
            self.height as u16,
        );
        output.extend_from_slice(&frame_header);

        // Create placeholder tile data
        let tile_data = vec![0u8; 64];
        let tile_group = sub_capsules.bitstream().write_tile_group(&tile_data, 0);
        output.extend_from_slice(&tile_group);

        // Update counters
        self.frame_count.fetch_add(1, Ordering::AcqRel);
        self.bytes_output.fetch_add(output.len() as u64, Ordering::AcqRel);
        sub_capsules.increment_generation();

        Ok(output)
    }

    pub fn flush(&self, _sub_capsules: &EncoderSubCapsules) -> Result<Vec<Vec<u8>>, String> {
        self.state.store(WiringState::Finalized as u64, Ordering::Release);
        Ok(Vec::new())
    }

    pub fn state(&self) -> WiringState {
        match self.state.load(Ordering::Acquire) {
            0 => WiringState::Uninitialized,
            1 => WiringState::Ready,
            2 => WiringState::Encoding,
            3 => WiringState::Finalized,
            _ => WiringState::Uninitialized,
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub fn stats(&self) -> EncoderWiringStats {
        EncoderWiringStats {
            frames_encoded: self.frame_count.load(Ordering::Acquire),
            bytes_output: self.bytes_output.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
            width: self.width,
            height: self.height,
            crf: self.crf,
            speed: self.speed,
            state: self.state(),
        }
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count.load(Ordering::Acquire)
    }

    pub fn increment_frame(&self) -> u64 {
        self.frame_count.fetch_add(1, Ordering::AcqRel)
    }
}

impl Default for EncoderWiringCapsule {
    fn default() -> Self {
        Self::new()
    }
}

const _: () = {
    assert!(
        core::mem::size_of::<EncoderWiringCapsule>() == 128,
        "EncoderWiringCapsule must be exactly 128 bytes"
    );
    assert!(
        core::mem::align_of::<EncoderWiringCapsule>() == 128,
        "EncoderWiringCapsule must be 128-byte aligned"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wiring_capsule_size() {
        assert_eq!(core::mem::size_of::<EncoderWiringCapsule>(), 128);
        assert_eq!(core::mem::align_of::<EncoderWiringCapsule>(), 128);
    }

    #[test]
    fn test_frame_counter() {
        let wiring = EncoderWiringCapsule::new();
        assert_eq!(wiring.frame_count(), 0);
        assert_eq!(wiring.increment_frame(), 0);
        assert_eq!(wiring.frame_count(), 1);
    }
}
