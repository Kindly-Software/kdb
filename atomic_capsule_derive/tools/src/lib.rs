//! Atomic Capsule Developer Tools
//!
//! This crate provides developer tools for maintaining and migrating atomic capsule code.

pub mod fix_padding_fields;

// Re-export main types for convenience
pub use fix_padding_fields::{
    type_size, TypeSize, evaluate_const_expr, transform_primitive_padding,
    fix_padding_file, fix_padding_recursive, TransformResult, PaddingError,
};
