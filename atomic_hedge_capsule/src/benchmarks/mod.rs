//! B32-Compliant Benchmark Framework
//!
//! Statistical validation and performance analysis tools for atomic hedge capsule benchmarks.
//! Implements UCE32 Q30 empirical validation requirements with Kontext27 reality checks.

pub mod statistical_validation;

pub use statistical_validation::{
    B32StatisticalValidator, Kontext27Classification, PerformanceReport, ValidationResult,
};
