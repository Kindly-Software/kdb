//! # Digital Signal Processing (DSP) Primitives
//!
//! Compile-time DSP filter generation for signal processing applications.
//!
//! ## Included Primitives
//!
//! - **FIRFilterConst**: Compile-time FIR filter coefficient generation with SIMD convolution
//!
//! ## Nightly Features
//!
//! All DSP primitives require:
//! - `const_fn_floating_point`: Compile-time floating-point math (sinc, Hamming window)
//! - `generic_const_exprs`: Compile-time validation (tap counts, sample rates)
//! - `portable_simd`: SIMD vectorization for fast convolution
//! - `std`: Standard library (Vec allocations, mutable iteration)

#[cfg(feature = "std")]
pub mod fir_filter_const;

#[cfg(feature = "std")]
pub use fir_filter_const::FIRFilterConst;
