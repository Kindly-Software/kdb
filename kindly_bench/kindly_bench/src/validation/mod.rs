//! B32 validation for benchmark reproducibility
//!
//! Performs 27 hardware/software checks to ensure reproducible results

pub mod hardware;

pub use hardware::HardwareInfo;
