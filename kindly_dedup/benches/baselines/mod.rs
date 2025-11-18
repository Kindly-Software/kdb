// Baseline implementations module
// B32 Framework: Fair comparison baselines for honest benchmarking

pub mod python_datasketch;
pub mod rust_scalar;

pub use python_datasketch::{BaselineResult, PythonDatasketch};
pub use rust_scalar::{scalar_tokenize, ScalarDedupPipeline, ScalarLSH, ScalarMinHash};
