//! # Weight Compression Module
//!
//! **6-10× compression with <2% accuracy loss for neural network weights.**

pub mod simd;
pub mod batch;

// Re-export key types from simd module
pub use simd::{
    BlockData, CompressedLayer,
    unpack_block_8x8_simd, find_nearest_centroid_simd,
    dequantize_blocks_simd, block_to_vector,
};

#[cfg(all(feature = "portable_simd", target_feature = "avx512f"))]
pub use simd::{
    unpack_block_8x8_simd_avx512,
    find_nearest_centroid_simd_avx512,
};

pub use batch::{
    decompress_blocks_batch,
    compress_blocks_batch,
    BatchConfig,
    CompressedBlock,
    DecompressedBlock,
};

/// Quantization format for mixed-precision compression
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum QuantFormat {
    Q4_4 = 0,
    Q6_6 = 1,
    Q8_8 = 2,
}

impl QuantFormat {
    #[inline(always)]
    pub const fn scale(self) -> f32 {
        match self {
            QuantFormat::Q4_4 => 16.0,
            QuantFormat::Q6_6 => 64.0,
            QuantFormat::Q8_8 => 256.0,
        }
    }
}

/// Quantized block data
#[derive(Clone, Debug)]
pub struct QuantizedBlock {
    pub data: Vec<u8>,
    pub format: QuantFormat,
    pub block_index: u32,
    pub scale: f32,
    pub zero_point: i16,
}
