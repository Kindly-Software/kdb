//! WebSocket module - Real-time metrics streaming
//!
//! **Phase 3 - RingBufferBroadcast Integration**
//!
//! This module provides WebSocket-based real-time metrics streaming using:
//! - `atomic_capsule::collections::RingBufferBroadcast` (2-5× faster than tokio::broadcast)
//! - MessagePack binary serialization (60-70% smaller than JSON)
//! - 100ms polling interval (configurable)
//! - Lossless delivery guarantee (exponential backoff)

pub mod handler;
pub mod protocol;

// Re-export public API
pub use handler::{DashboardBroadcast, DashboardUpdate};
pub use protocol::{
    deserialize_batch, deserialize_snapshot, deserialize_update, serialize_batch,
    serialize_snapshot, serialize_update, ProtocolError, ProtocolResult,
};
