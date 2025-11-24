//! # QUIC (RFC 9000/9002/9204) Support Capsules - Flow Control + Loss Detection + Header Compression + Audit Trail
//!
//! High-performance, lockfree QUIC support using computational capsules.
//!
//! ## Implemented Capsules
//!
//! - **QuicAuditTrailCapsule** (T0 Auditable, 256B):
//!   Hash-chain audit trail for QUIC connection events (tamper-evident, Q34 SOX/SOC2/GDPR/HIPAA compliance)
//! - **AckTrackerCapsule** (T4 Batch, 4KB):
//!   Ring buffer packet tracking with batch ACK range processing (RFC 9000 §19.3)
//! - **ConnectionTableCapsule** (T4 Batch, 8KB):
//!   Lockfree hash table for connection ID → state mapping (4,096 slots, <100ns lookup, 5× batch speedup)
//! - **StreamStateTableCapsule** (T4 Batch, 32KB):
//!   Lockfree hash table for stream ID → state mapping (2,048 streams, <100ns lookup, 5-10× batch speedup)
//! - **FlowControlCapsule** (T1 Atomic + T3 Fixed-Point, 64B):
//!   Dual-level flow control (connection + stream) with Q16.16 precision
//! - **RetransmissionQueueCapsule** (T5 Streaming, 2KB):
//!   Circular FIFO queue for lost packet retransmission (RFC 9002 §6.2, <100ns enqueue/dequeue, 128 entries)
//! - **QpackEncoderCapsule** (T2 SIMD + T4 Batch, 1024B):
//!   HTTP/3 header compression with SIMD static table lookup (5-20× speedup)
//! - **QpackDecoderCapsule** (T2 SIMD + T4 Batch, 1024B):
//!   HTTP/3 header decompression (RFC 9204 §4, 5-20× speedup, batch optimized)
//!
//! ## Overview
//!
//! QUIC is the foundation for HTTP/3. This module provides lockfree, cache-aligned capsules:
//! - Zero-copy atomic coordination (no Mutex/RwLock)
//! - Q16.16 fixed-point window tracking (deterministic, 0.0000153 precision)
//! - <20ns per operation (RFC 9000 §4.1 compliant)
//! - <100ns retransmission queue operations (RFC 9002 §6.2 loss detection)
//! - SIMD-accelerated header compression (RFC 9204, 5-20× speedup)
//! - Batch connection lookup (5× faster via sorted probing)
//!
//! ## RFC Compliance
//!
//! All capsules follow QUIC specifications:
//! - RFC 9000: QUIC Protocol (§4.1 Flow Control, connection + stream windows, Connection ID management)
//! - RFC 9002: QUIC Loss Detection and Congestion Control (§6.2 loss detection, retransmission management)
//! - RFC 9204: QPACK Header Compression (§3 Static Table, §4 Encoding)
//!
//! ## Feature Flags
//!
//! QUIC support is gated behind the `quic` feature flag:
//! ```toml
//! [features]
pub mod ack_tracker;
pub mod audit_trail;
pub mod connection_table;
pub mod endpoint_metacapsule;
pub mod flow_control;
pub mod guc_firmware_capsule;
pub mod http3_control_stream;
pub mod http3_request_stream;
pub mod pacing;
pub mod qpack_decoder;
pub mod qpack_encoder;
pub mod retransmission_queue;
pub mod stream_flow_control;
pub mod stream_state_table;

pub use ack_tracker::{AckRange, AckTrackerCapsule, SentPacket, MAX_ACK_RANGES, MAX_SENT_PACKETS};
pub use audit_trail::{AuditEventType, AuditTrailError, QuicAuditTrailCapsule};
#[cfg(feature = "std")]
pub use audit_trail::ExportedEvent;
pub use connection_table::{ConnectionTableCapsule, ConnectionId, ConnectionTableError};
pub use endpoint_metacapsule::{QuicEndpointError, QuicEndpointMetacapsule};
pub use flow_control::{FlowControlCapsule, FlowControlError};
pub use guc_firmware_capsule::{
    GuCFirmwareCapsule, DoorbellState, FirmwareResponse, GuCError, DoorbellHandle,
    WorkloadHandle, FirmwareStatus,
};
pub use http3_control_stream::{
    ControlStreamState, Http3ControlStreamCapsule, Http3ControlStreamError,
};
pub use http3_request_stream::{BodyChunk, ChunkFlags, Http3RequestStreamCapsule, Http3Result, Http3StreamError, HttpMethod, RequestStreamState};
pub use pacing::PacingCapsule;
pub use qpack_decoder::{QpackDecoderCapsule, QpackError, QpackEntry};
pub use qpack_encoder::{QpackEncoderCapsule, EncoderStats};
pub use retransmission_queue::{RetransmissionQueueCapsule, RetransmissionEntry, RetransmissionQueueError};
pub use stream_flow_control::{FlowControlSnapshot, StreamFlowControlCapsule};
pub use stream_state_table::{StreamStateTableCapsuleStandard, StreamStateTableError, StreamEntry, StreamBucket};
