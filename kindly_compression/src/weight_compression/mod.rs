//! # Weight Compression Module
//!
//! Deterministic fixed-point quantization for neural network weights.
//!
//! ## UCE34 Q10: T3 Fixed-Point Tier
//!
//! This module implements T3 Fixed-Point computational capsules for weight compression:
//! - **Q4.4**: 4-bit integer, 4-bit fractional (±8.0 range, 0.0625 precision)
//! - **Q6.6**: 6-bit integer, 6-bit fractional (±32.0 range, 0.015625 precision)
//! - **Q8.8**: 8-bit integer, 8-bit fractional (±128.0 range, 0.00390625 precision)
//!
//! ## Framework Compliance
//!
//! - **UCE34 Q10**: T3 Fixed-Point (2-5× speedup, 100% deterministic)
//! - **ASSUM**: Zero FP arithmetic (integer ALU only)
//! - **T28**: Property tests for determinism (1000 iterations)
//! - **B32**: Performance validation vs FP arithmetic
//!
//! ## Key Properties
//!
//! - **Zero FP Arithmetic**: Integer ALU only (deterministic rounding)
//! - **100% Deterministic**: Same input → same output always
//! - **Round-Trip**: dequantize(quantize(x)) ≈ x (within precision)
//! - **2-5× Speedup**: Integer ops vs FP ops

pub mod quantization;

pub use quantization::{
    QuantFormat,
    quantize_q4_4, dequantize_q4_4,
    quantize_q6_6, dequantize_q6_6,
    quantize_q8_8, dequantize_q8_8,
    quantize_block, dequantize_block,
};
