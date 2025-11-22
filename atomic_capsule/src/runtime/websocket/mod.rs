//! WebSocket Protocol Capsules (T1 Atomic)
//!
//! Lockfree WebSocket frame handling for RFC 6455 compliance.
//!
//! # Modules
//!
//! - **frame_writer**: WebSocket frame serialization (RFC 6455)

pub mod frame_writer;

pub use frame_writer::{
    WebSocketFrameWriterCapsule, OpCode, FrameWriteError, FrameWriterStats,
};
