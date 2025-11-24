//! # T8 Network Capsules - Distributed Coordination
//!
//! **Tier 8**: Network-distributed atomic capsules for multi-node coordination.
//!
//! ## Architecture Overview
//!
//! T8 extends atomic capsule patterns to distributed systems:
//! - **NetworkShardCapsule**: 256B aligned shard state with health monitoring
//! - **RPC Protocol**: Type-safe async message passing (bincode serialization)
//! - **Consistent Hashing**: Deterministic shard routing with virtual nodes
//! - **RPC Client/Server**: Async I/O with circuit breaker integration
//!
//! ## Performance Targets (B32 Framework)
//!
//! - Shard health check: <10ns (atomic load)
//! - RPC latency: <5ms P99 (local network)
//! - Consistent hash lookup: <10ns (binary search)
//! - Circuit breaker overhead: <5ns (existing T1 integration)
//!
//! ## UCE34 Framework Compliance
//!
//! - **Q10**: T8 Network tier (distributed coordination)
//! - **Q11**: Rust async/await (tokio), bincode serialization
//! - **Q12**: Nightly not required (stable async)
//! - **Q33**: All capsules use #[derive(ComputationalCapsule)]
//! - **Q34**: Audit trail via generation counters
//!
//! ## ASSUM Safety Model
//!
//! - `#ASSUME_NETWORK_RELIABILITY`: RPC failures handled via circuit breaker
//! - `#ASSUME_SHARD_MONOTONIC`: Generation counters prevent rollback
//! - `#ASSUME_CONSISTENT_HASH_DETERMINISM`: Hash function is deterministic
//! - `#VERIFY_LOCKFREE`: All capsules use atomic operations (no mutex)
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use atomic_capsule::network::{NetworkShardCapsule, RpcClient, ConsistentHashRing};
//!
//! // Initialize shard
//! let shard = NetworkShardCapsule::new(42);
//! shard.update_heartbeat(current_ns);
//!
//! // Route requests
//! let ring = ConsistentHashRing::new(150); // 150 vnodes
//! ring.add_shard(42);
//! let shard_id = ring.get_shard(b"some_key");
//!
//! // RPC call
//! let client = RpcClient::connect("127.0.0.1:8080").await?;
//! let response = client.send(request).await?;
//! ```

pub mod consistent_hash;
pub mod json_rpc;
pub mod monitoring;
pub mod packet_buffer_const;
pub mod rpc_client;
pub mod rpc_protocol;
pub mod rpc_server;
pub mod shard_capsule;

// P2: Quorum Read Capsule (distributed consistency)
pub mod quorum_read;

// T8 Network Capsules - Pipeline implementations (Agents 4, 5, 6)
#[cfg(feature = "send-pipeline")]
pub mod send_pipeline;

#[cfg(feature = "receive-pipeline")]
pub mod receive_pipeline;

#[cfg(feature = "network-metacapsule")]
pub mod metacapsule;

// Re-exports
pub use consistent_hash::ConsistentHashRing;
pub use json_rpc::{
    format_error, format_response, parse_request, JsonRpcCapsule, JsonRpcErrorCode,
    JsonRpcRequest,
};
pub use monitoring::{
    ClusterMetrics, MetricsCapsule, MetricsDashboard, MetricsSnapshot, GLOBAL_METRICS,
};
pub use packet_buffer_const::{
    calculate_buffer_memory, is_power_of_2, validate_mtu, validate_queue_depth, PacketBufferConst,
    PacketError,
};
pub use rpc_client::{RpcClient, RpcClientConfig};
pub use rpc_protocol::{RpcMethod, RpcRequest, RpcResponse};
pub use rpc_server::RpcServer;
pub use shard_capsule::{NetworkShardCapsule, ShardHealth};

// P2: Quorum Read exports
pub use quorum_read::{QuorumReadCapsule, QuorumResult, MAX_REPLICAS, QUORUM_THRESHOLD};

// T2+T5 SIMD+Streaming Receive Pipeline (Agents 4, 5, 6)
#[cfg(feature = "receive-pipeline")]
pub use receive_pipeline::{
    ReceivePipelineCapsule, RecvState, RecvError, ReceivedPacket, DataFrame,
};

// T4+T5 Batch+Streaming Send Pipeline (Agents 4, 5, 6)
#[cfg(feature = "send-pipeline")]
pub use send_pipeline::{
    SendPipelineCapsule, SendState, SendError,
};

// T6 Mixed Network Metacapsule (Agents 4, 5, 6)
#[cfg(feature = "network-metacapsule")]
pub use metacapsule::NetworkPacketMetacapsule;
