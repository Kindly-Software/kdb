// Re-export load testing framework from shared location
#[path = "../load_test_common.rs"]
mod load_test_common;

pub use load_test_common::*;
