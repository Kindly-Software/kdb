//! Cause flag definitions for the standard layout.

/// Thermal pressure indicator.
pub const THERM: u8 = 1 << 0;
/// Network congestion indicator.
pub const NET: u8 = 1 << 1;
/// Input/output saturation indicator.
pub const IO: u8 = 1 << 2;
/// Memory pressure indicator.
pub const MEM: u8 = 1 << 3;
/// CPU saturation indicator.
pub const CPU: u8 = 1 << 4;
/// Latency breach indicator.
pub const LAT: u8 = 1 << 5;
/// Jitter breach indicator.
pub const JIT: u8 = 1 << 6;
/// Timeout or staleness indicator.
pub const TIMEOUT: u8 = 1 << 7;
