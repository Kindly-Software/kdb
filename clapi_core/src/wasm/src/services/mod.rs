//! Service layer for WASM frontend
//!
//! Provides services for:
//! - WebSocket real-time client (ws_client)
//! - WebSocket connection pooling (PollingServiceCapsule)
//! - HTTP polling for Free tier (5s interval)
//!
//! # T4 Batch Capsules
//! - **PollingServiceCapsule**: 10K connection management, <100ns operations
//!
//! # T1 Atomic Services
//! - **WebSocketClient**: Real-time dashboard updates with automatic reconnect

pub mod ws_client;
pub mod ws_pool;

pub use ws_client::{ConnectionState as WsConnectionState, WebSocketClient, WsMessageCapsule};
pub use ws_pool::{
    ConnectionId, ConnectionState, ConnectionStorage, PollingServiceCapsule, SubscriptionTier,
    UserId, WsPoolError,
};
