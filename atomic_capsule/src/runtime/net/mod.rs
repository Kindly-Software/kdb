//! Network Module - Lockfree Async Networking (T5 Streaming + T1 Atomic)
//!
//! # Components
//!
//! - **AsyncTcpCapsule**: Async TCP socket wrapper with ring buffers (T5 Streaming)
//! - **AsyncUdpCapsule**: Async UDP socket wrapper (T1 Atomic + T8 Network)
//!
//! # Architecture
//!
//! 100% lockfree network operations:
//! - Atomic socket state tracking (DualAtomicU64)
//! - Ring buffer read/write queues (lockfree coordination)
//! - Generation counters for FD reuse prevention
//! - Reactor integration for epoll/kqueue multiplexing
//!
//! # Performance Targets (B32)
//!
//! - TCP accept/connect: <100ns (vs 5-10µs tokio)
//! - TCP read/write: <1µs batched (vs 2-5µs tokio)
//! - Throughput: 10Gbps+ (vs 5-8Gbps tokio)
//! - Memory: 256B capsule + ring buffers (tunable)
//!
//! # Safety & Testing
//!
//! - 99.5%+ safe code (ASSUM framework)
//! - 50+ tests covering unit, property, integration, production
//! - Generation counters prevent socket FD reuse bugs
//! - Atomic operations for all shared state

#[cfg(feature = "kind-tcp")]
pub mod tcp;

#[cfg(feature = "runtime-net")]
pub mod udp;

#[cfg(feature = "unix-socket")]
pub mod unix_socket;

#[cfg(feature = "kind-tcp")]
pub use tcp::{AsyncTcpCapsule, AsyncTcpStream, AsyncTcpListener, TcpState, TcpError, TcpResult};

#[cfg(feature = "runtime-net")]
pub use udp::{AsyncUdpCapsule, UdpStatsCapsule, UdpStats, SocketState, SocketFlags};

#[cfg(feature = "unix-socket")]
pub use unix_socket::AsyncUnixSocketCapsule;

// # Tier Selection (UCE34 Q10)
//
// - **T5 Streaming**: TCP reads/writes (incremental I/O, O(1) per batch)
// - **T1 Atomic**: Socket state coordination (DualAtomicU64)
// - **T8 Network**: Distributed coordination & resilience
//
// # Usage Examples
//
// ```ignore
// use atomic_capsule::runtime::net::AsyncTcpCapsule;
//
// // Connect to server
// let capsule = AsyncTcpCapsule::connect("127.0.0.1:8080".parse()?).await?;
//
// // Async read
// let mut buf = vec![0u8; 4096];
// let n = capsule.read(&mut buf).await?;
//
// // Async write
// capsule.write_all(b"Hello").await?;
// ```
//
// # Relation to Other Modules
//
// - **reactor**: Provides epoll/kqueue event multiplexing
// - **timer_wheel**: Scheduling timeouts & connection keep-alives
// - **async_file**: Similar ring buffer pattern for file I/O
// - **AsyncUdpCapsule**: Parallel UDP networking (same tier)
