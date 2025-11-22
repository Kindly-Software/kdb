//! # Tier 8: Network Capsule - Distributed Coordination
//!
//! **UCE33 Q10**: Tier 8 Network capsules provide distributed coordination across networked systems.
//!
//! ## Performance Expectations (B32 Guidelines)
//!
//! - **Throughput**: 10-50× via horizontal scaling (multiple nodes)
//! - **Latency**: 100μs-10ms (network RTT + processing)
//! - **Packet Rate**: 10-100 Mpps with DPDK/io_uring
//! - **Bandwidth**: 10-100 Gbps with zero-copy techniques
//!
//! ## Use Cases
//!
//! - Multi-venue trading (cross-exchange arbitrage, <100μs latency)
//! - Distributed training (data parallelism, gradient synchronization)
//! - Consensus systems (Raft, Paxos, BFT protocols)
//! - HFT market data (multicast, zero-copy packet processing)
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_NETWORK_AVAILABLE`: Network connectivity established
//! - `#VERIFY_NETWORK_AVAILABLE`: Connection health checks
//! - `#ASSUME_EVENTUAL_CONSISTENCY`: CRDTs converge eventually
//! - `#VERIFY_EVENTUAL_CONSISTENCY`: Property tests with network partitions
//! - `#ASSUME_MESSAGE_DELIVERY`: Messages are eventually delivered
//! - `#VERIFY_MESSAGE_DELIVERY`: Timeout handling + retries
//!
//! ## B32 Reality Checks
//!
//! - **Localhost**: 10μs RTT (kernel stack)
//! - **LAN**: 200μs RTT (1GbE switch)
//! - **WAN**: 50ms RTT (cross-region)
//! - **Kernel Bypass**: 10-100× faster (DPDK, io_uring)
//!
//! ## Implementation Notes
//!
//! This is a **foundation trait** - actual network implementations will require:
//! - External crates (tokio, io_uring, dpdk bindings)
//! - Async/await integration
//! - Connection management
//! - Retry logic and timeouts
//!
//! ## Example Use Cases
//!
//! ```rust,ignore
//! // Multi-venue arbitrage (3 exchanges)
//! // Latency budget: <100μs
//! // Network: 20μs + Processing: 50μs + Execution: 30μs
//!
//! // Distributed consensus (5 nodes, Raft)
//! // Commit latency: 1-2ms (2 RTTs)
//! // Throughput: 10K commits/sec
//!
//! // Market data multicast (1M msg/sec)
//! // Processing: 500ns/msg
//! // DPDK zero-copy: 100Mpps capable
//! ```

use crate::traits::ComputationalCapsule;
use core::fmt;

/// Error types for network capsule operations.
///
/// ## UCE33 Q20: Error Handling
///
/// Network operations can fail in several ways:
/// - Connection not established
/// - Send/receive failures (timeout, network error)
/// - Consensus timeout (distributed systems)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkError {
    /// Not connected to network
    NotConnected,
    /// Send operation failed (timeout, network error, buffer full)
    SendFailed(&'static str),
    /// Receive operation failed (timeout, network error, buffer empty)
    ReceiveFailed(&'static str),
    /// Consensus timeout (quorum not reached)
    ConsensusTimeout {
        /// Number of nodes that responded
        responses: usize,
        /// Required quorum size
        quorum: usize,
    },
    /// Invalid node ID (unknown peer)
    InvalidNode,
    /// Message too large for network MTU
    MessageTooLarge {
        /// Message size in bytes
        size: usize,
        /// Maximum allowed size
        max_size: usize,
    },
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::NotConnected => write!(f, "Not connected to network"),
            NetworkError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
            NetworkError::ReceiveFailed(msg) => write!(f, "Receive failed: {}", msg),
            NetworkError::ConsensusTimeout { responses, quorum } => {
                write!(
                    f,
                    "Consensus timeout: got {} responses, needed {}",
                    responses, quorum
                )
            }
            NetworkError::InvalidNode => write!(f, "Invalid node ID"),
            NetworkError::MessageTooLarge { size, max_size } => {
                write!(f, "Message too large: {} bytes (max {})", size, max_size)
            }
        }
    }
}

impl core::error::Error for NetworkError {}

/// Tier 8: Network Capsule trait for distributed coordination.
///
/// ## UCE33 Q10: Tier 8 Network
///
/// Network capsules provide:
/// - 10-50× throughput via horizontal scaling
/// - Distributed coordination (consensus, CRDTs)
/// - High-performance networking (DPDK, io_uring)
///
/// ## ASSUM Framework
///
/// - `#ASSUME_NETWORK_AVAILABLE`: Network connectivity established
/// - `#VERIFY_NETWORK_AVAILABLE`: Connection health checks
/// - `#ASSUME_EVENTUAL_CONSISTENCY`: CRDTs converge eventually
/// - `#VERIFY_EVENTUAL_CONSISTENCY`: Property tests with network partitions
/// - `#ASSUME_MESSAGE_ORDER`: Messages may be reordered (UDP) or ordered (TCP)
/// - `#VERIFY_MESSAGE_ORDER`: Sequence numbers in protocol
///
/// ## Safety
///
/// This trait is unsafe to implement because:
/// - Network I/O requires careful error handling
/// - Distributed coordination can deadlock
/// - Message ordering affects correctness
///
/// ## Example
///
/// ```rust,ignore
/// use atomic_capsule::traits::network::{NetworkCapsule, NetworkError};
/// use atomic_capsule::traits::ComputationalCapsule;
///
/// #[repr(C, align(64))]
/// struct ConsensusNodeCapsule {
///     node_id: u64,
///     peers: Vec<u64>,
///     state: AtomicU64,
/// }
///
/// unsafe impl ComputationalCapsule for ConsensusNodeCapsule {
///     const ALIGNMENT: usize = 64;
///     const SIZE: usize = 64; // Capsule header size
///     const TYPE_ID: &'static str = "ConsensusNodeCapsule";
/// }
///
/// unsafe impl NetworkCapsule for ConsensusNodeCapsule {
///     type NodeId = u64;
///
///     fn send(&self, node: Self::NodeId, message: &[u8]) -> Result<(), NetworkError> {
///         // Send message to remote node
///         tcp_send(node, message)?;
///         Ok(())
///     }
///
///     fn receive(&self) -> Result<Option<(Self::NodeId, Vec<u8>)>, NetworkError> {
///         // Non-blocking receive
///         if let Some((node, msg)) = tcp_recv_nonblocking()? {
///             Ok(Some((node, msg)))
///         } else {
///             Ok(None)
///         }
///     }
///
///     fn broadcast(&self, message: &[u8]) -> Result<(), NetworkError> {
///         // Send to all peers
///         for &peer in &self.peers {
///             self.send(peer, message)?;
///         }
///         Ok(())
///     }
///
///     fn sync(&mut self) -> Result<(), NetworkError> {
///         // CRDT merge: synchronize state across nodes
///         let local_state = self.state.load(Ordering::Acquire);
///         self.broadcast(&local_state.to_le_bytes())?;
///
///         // Receive and merge remote states
///         while let Some((_, msg)) = self.receive()? {
///             let remote_state = u64::from_le_bytes(msg[..8].try_into().unwrap());
///             // CRDT merge: max(local, remote)
///             self.state.fetch_max(remote_state, Ordering::Release);
///         }
///
///         Ok(())
///     }
/// }
/// ```
pub unsafe trait NetworkCapsule: ComputationalCapsule {
    /// Network node identifier.
    ///
    /// ## Implementation-Specific
    ///
    /// - TCP/UDP: IP address + port
    /// - RDMA: Queue pair number
    /// - Local: Process ID or thread ID
    ///
    /// Must implement:
    /// - `Clone`: For routing tables
    /// - `Eq`: For node comparison
    type NodeId: Clone + Eq;

    /// Send message to remote node.
    ///
    /// ## UCE33 Q20: Error Handling
    ///
    /// This operation can fail if:
    /// - Not connected to network
    /// - Send buffer full (backpressure)
    /// - Network error (timeout, reset)
    ///
    /// ## B32 Reality Check
    ///
    /// - TCP send: 1-10μs (kernel stack)
    /// - UDP send: 500ns-5μs (kernel stack)
    /// - DPDK send: 100-500ns (kernel bypass)
    ///
    /// # Arguments
    ///
    /// - `node`: Target node identifier
    /// - `message`: Message bytes to send
    ///
    /// # Returns
    ///
    /// - `Ok(())` if message was sent (buffered or transmitted)
    /// - `Err(NetworkError)` if send failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// capsule.send(peer_id, b"hello")?;
    /// ```
    fn send(&self, node: Self::NodeId, message: &[u8]) -> Result<(), NetworkError>;

    /// Receive message from remote node (non-blocking).
    ///
    /// ## UCE33 Q20: Error Handling
    ///
    /// This operation can fail if:
    /// - Network error (timeout, reset)
    /// - Message corrupted (checksum failure)
    /// - Receive buffer empty (no data available)
    ///
    /// ## B32 Reality Check
    ///
    /// - TCP recv: 1-10μs (kernel stack)
    /// - UDP recv: 500ns-5μs (kernel stack)
    /// - DPDK recv: 100-500ns (kernel bypass)
    ///
    /// # Returns
    ///
    /// - `Ok(Some((node, message)))` if message received
    /// - `Ok(None)` if no message available (non-blocking)
    /// - `Err(NetworkError)` if receive failed
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some((peer, msg)) = capsule.receive()? {
    ///     println!("Got message from {}", peer);
    /// }
    /// ```
    fn receive(&self) -> ReceiveResult<Self::NodeId>;

    /// Broadcast message to all nodes.
    ///
    /// ## UCE33 Q28: Simplicity
    ///
    /// Convenience method for sending to all peers.
    ///
    /// ## B32 Reality Check
    ///
    /// - Unicast N times: N × 5μs = 50μs for 10 nodes
    /// - Multicast: 5-10μs (single send to multicast group)
    ///
    /// # Arguments
    ///
    /// - `message`: Message bytes to broadcast
    ///
    /// # Returns
    ///
    /// - `Ok(())` if broadcast succeeded to all nodes
    /// - `Err(NetworkError)` at first failure
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// capsule.broadcast(b"state_update")?;
    /// ```
    fn broadcast(&self, message: &[u8]) -> Result<(), NetworkError>;

    /// Synchronize state across nodes (CRDT merge).
    ///
    /// ## UCE33 Q10: Tier 8 Network
    ///
    /// Network capsules provide eventual consistency via CRDT merges:
    /// - Send local state to all peers
    /// - Receive remote states
    /// - Merge states using CRDT rules (max, LWW, etc.)
    ///
    /// ## ASSUM Framework
    ///
    /// - `#ASSUME_EVENTUAL_CONSISTENCY`: All nodes eventually converge
    /// - `#VERIFY_EVENTUAL_CONSISTENCY`: Property tests with partitions
    ///
    /// ## B32 Reality Check
    ///
    /// - Sync latency: 1-10ms (2 RTTs for quorum)
    /// - Throughput: 100-1000 syncs/sec
    /// - Conflict resolution: <1μs (CRDT merge)
    ///
    /// # Returns
    ///
    /// - `Ok(())` if synchronization completed
    /// - `Err(NetworkError)` if sync failed (timeout, network error)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// capsule.sync()?;
    /// // State is now consistent across all nodes
    /// ```
    fn sync(&mut self) -> Result<(), NetworkError>;

    /// Check if connected to network.
    ///
    /// ## ASSUM Framework
    ///
    /// - `#ASSUME_NETWORK_AVAILABLE`: This returns true before operations
    /// - `#VERIFY_NETWORK_AVAILABLE`: Call this before attempting operations
    ///
    /// # Returns
    ///
    /// - `true` if network connection is established
    /// - `false` if disconnected (fallback or retry)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if capsule.is_connected() {
    ///     capsule.send(peer, message)?;
    /// } else {
    ///     reconnect()?;
    /// }
    /// ```
    fn is_connected(&self) -> bool {
        // Default implementation: assume connected
        // Implementations should override with actual health check
        true
    }

    /// Get network statistics.
    ///
    /// ## UCE33 Q19: Monitoring
    ///
    /// Returns information about network performance:
    /// - Messages sent/received
    /// - Bytes transferred
    /// - Latency percentiles
    ///
    /// # Returns
    ///
    /// - `Some(NetworkStats)` if statistics available
    /// - `None` if monitoring disabled
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(stats) = capsule.network_stats() {
    ///     println!("RTT: {}μs", stats.avg_rtt_us);
    /// }
    /// ```
    fn network_stats(&self) -> Option<NetworkStats> {
        None // Default: no statistics
    }

    /// Get list of connected peers.
    ///
    /// ## UCE33 Q17: Interfaces
    ///
    /// Returns current network topology.
    ///
    /// # Returns
    ///
    /// - List of connected node IDs
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let peers = capsule.peers();
    /// println!("Connected to {} peers", peers.len());
    /// ```
    fn peers(&self) -> Vec<Self::NodeId> {
        Vec::new() // Default: no peers
    }
}

/// Result type for receive operations (node ID + message bytes).
///
/// ## UCE33 Q31: Simplicity
///
/// Type alias reduces complexity of return types.
pub type ReceiveResult<NodeId> = Result<Option<(NodeId, Vec<u8>)>, NetworkError>;

/// Network statistics for monitoring.
///
/// ## UCE33 Q19: Monitoring
///
/// Performance metrics for network operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkStats {
    /// Messages sent since startup
    pub messages_sent: u64,
    /// Messages received since startup
    pub messages_received: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Average round-trip time in microseconds
    pub avg_rtt_us: u64,
    /// P99 latency in microseconds
    pub p99_latency_us: u64,
    /// Number of send failures
    pub send_failures: u64,
    /// Number of receive failures
    pub recv_failures: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_error_display() {
        let err = NetworkError::NotConnected;
        assert!(err.to_string().contains("Not connected"));

        let err = NetworkError::SendFailed("timeout");
        assert!(err.to_string().contains("timeout"));

        let err = NetworkError::ConsensusTimeout {
            responses: 2,
            quorum: 3,
        };
        assert!(err.to_string().contains("2"));
        assert!(err.to_string().contains("3"));

        let err = NetworkError::MessageTooLarge {
            size: 2000,
            max_size: 1500,
        };
        assert!(err.to_string().contains("2000"));
        assert!(err.to_string().contains("1500"));
    }

    #[test]
    fn test_network_stats_default() {
        let stats = NetworkStats::default();
        assert_eq!(stats.messages_sent, 0);
        assert_eq!(stats.messages_received, 0);
        assert_eq!(stats.avg_rtt_us, 0);
    }
}
