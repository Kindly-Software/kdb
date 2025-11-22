# T8 Network Capsule - Complete UCE34 Analysis
**Version**: 1.0
**Date**: 2025-10-27
**Framework**: UCE34 Q1-Q34 (Systematic Discovery)
**Tier**: T8 Network (Distributed Coordination)
**Status**: Design Complete - Ready for Implementation

---

## Executive Summary

**Tier 8 (Network)** enables lockfree coordination across distributed systems via RPC with atomic semantics.

**Core Innovation**: Network RPC behaves like local atomic operations → distributed algorithms use same patterns as single-machine.

**Key Pattern**: Shard coordination with health monitoring → automatic failover, load balancing, zero coordinator bottleneck.

**Use Case**: Distributed LLM deduplication (100B+ documents across 100+ servers).

**Performance**: <10ms RPC latency (local datacenter), <1ms local shard access, automatic failover <100ms.

**Dependencies**: tokio (async I/O), bincode (zero-copy serialization).

---

## PHASE 1: META-COGNITIVE FOUNDATION (Q1-Q9)

### Q1: Problem Statement - What does T8 solve?

**The Problem**: Scaling beyond single-machine limits

**Single-Machine Limits**:
```
16-core server: 20K docs/sec dedup
Maximum corpus: ~100M documents (takes 83 minutes)
Memory limit: 32GB RAM = 125M MinHash signatures max

OpenAI needs: 100B documents (1000× larger)
Would take: 83 minutes × 1000 = 57 days on single machine ❌
```

**T8 Network Solution** (Distributed):
```
100 servers × 20K docs/sec = 2M docs/sec
100B documents: 50K seconds = 13.9 hours ✅

Sharding strategy:
- LSH bucket ID % 100 = shard server
- Each server: 1B docs (within 32GB RAM)
- Coordinator: Routes requests to correct shard
- Lockfree: No central coordinator bottleneck
```

---

**Specific Problems T8 Solves**:

**Problem 1**: Horizontal Scaling (Beyond 32GB RAM)
- **Need**: Store 1B MinHash signatures (256B × 1B = 256GB)
- **Single machine**: Can't fit in 32GB RAM
- **T8 Solution**: Shard across 10 servers (25.6GB each)

**Problem 2**: Geographic Distribution (Low Latency)
- **Need**: Serve global customers (<100ms latency)
- **Single datacenter**: 200ms from Asia → US
- **T8 Solution**: Regional shards (US, EU, APAC)

**Problem 3**: Fault Tolerance (High Availability)
- **Need**: Survive server failures (99.99% uptime)
- **Single machine**: Failure = total outage
- **T8 Solution**: Replicas + automatic failover

---

### Q2: Core Invariant - What MUST always be true?

**INVARIANT I1**: Shard assignment is deterministic
```rust
fn shard_id(lsh_bucket: u16, shard_count: u16) -> u16 {
    lsh_bucket % shard_count
}

// INVARIANT: Same bucket always routes to same shard
// Critical for: Finding duplicates (must search same shard)

#ASSUME_DETERMINISTIC_SHARDING: Modulo is deterministic
#VERIFY_DETERMINISTIC_SHARDING: Property test (1000 buckets → same shards)
```

**INVARIANT I2**: Shard health is eventually consistent
```rust
// Each shard reports health via heartbeat
heartbeat_capsule.last_seen_ns.store(now(), Ordering::Release);

// Coordinator reads health (may be stale, acceptable)
let last_seen = heartbeat_capsule.last_seen_ns.load(Ordering::Acquire);
let is_healthy = (now() - last_seen) < TIMEOUT_NS;

// INVARIANT: Health status is eventually consistent (not real-time)
// Acceptable: Stale health (100ms) is fine, not safety-critical

#ASSUME_EVENTUAL_CONSISTENCY: Heartbeats propagate within 100ms
#VERIFY_EVENTUAL_CONSISTENCY: Network partition test (measure convergence time)
```

**INVARIANT I3**: RPC failures are isolated
```rust
// RPC to shard server
match rpc_call(shard_id, request).await {
    Ok(response) => process(response),
    Err(NetworkError) => {
        // INVARIANT: Network failure doesn't corrupt state
        // Shard state unchanged (idempotent operations)
        retry_with_backoff()?;
    }
}

#ASSUME_RPC_IDEMPOTENT: Retry is safe (same request → same result)
#VERIFY_RPC_IDEMPOTENT: Duplicate request test (ensure no double-processing)
```

---

### Q3: Success Criteria - What defines victory?

**FUNCTIONAL CRITERIA**:
- ✅ Shard 1B documents across 100 servers (10M per server)
- ✅ Query latency: <10ms p99 (local datacenter)
- ✅ Failover: <100ms (automatic shard replica promotion)
- ✅ Load balancing: ±10% variance across shards (even distribution)

**PERFORMANCE CRITERIA**:
- ✅ Throughput: 2M docs/sec (100 servers × 20K docs/sec)
- ✅ RPC latency: <5ms p50, <10ms p99 (local network)
- ✅ Network bandwidth: <100Mbps per server (efficient protocol)
- ✅ CPU overhead: <10% for RPC marshalling (rest is dedup)

**RELIABILITY CRITERIA**:
- ✅ Uptime: 99.99% (4× nines, <53 minutes downtime/year)
- ✅ Data durability: 99.999% (5× nines, replicas + backups)
- ✅ Partition tolerance: Survives network split (continue operating)
- ✅ Consistency: Eventual (100ms convergence typical)

**BUSINESS CRITERIA** (Enables Enterprise Deals):
- ✅ Scale: 100B+ documents (OpenAI/Google scale)
- ✅ SLA: 99.99% uptime (enterprise requirement)
- ✅ Compliance: Data residency (regional shards for GDPR)
- ✅ Revenue: Unlocks $500K+ deals (large-scale customers)

---

### Q4: Failure Modes - What breaks?

**FAILURE MODE F1**: Shard server crashes (lose 1/100 of capacity)
- **Probability**: 10% per year per server (hardware failure)
- **Impact**: MEDIUM (1% capacity loss, automatic failover)
- **Detection**: Heartbeat timeout (no heartbeat for 10 seconds)
- **Recovery**: Route to replica shard (automatic, <100ms)
- **Mitigation**: 2× replication (every shard has replica)

**FAILURE MODE F2**: Network partition (split brain)
- **Probability**: 5% per year (datacenter network issue)
- **Impact**: HIGH (half of shards unreachable)
- **Detection**: Heartbeat timeouts for >50% of shards
- **Recovery**: Degrade gracefully (serve from available shards only)
- **Mitigation**: Regional redundancy (if US-East fails, route to US-West)

**FAILURE MODE F3**: Coordinator crashes (single point of failure)
- **Probability**: 10% per year
- **Impact**: CRITICAL (total outage if single coordinator)
- **Detection**: Health check fails
- **Recovery**: Standby coordinator promotes (Raft consensus)
- **Mitigation**: 3× coordinator replicas (Raft quorum)

**FAILURE MODE F4**: Thundering herd (all clients to one shard)
- **Probability**: 20% (hot partition, poor sharding)
- **Impact**: MEDIUM (one shard overloaded, others idle)
- **Detection**: Shard latency >10× average
- **Recovery**: Sub-sharding (split hot shard into 16 sub-shards)
- **Mitigation**: Consistent hashing (even distribution)

**FAILURE MODE F5**: RPC timeout (slow network)
- **Probability**: 30% (occasional latency spikes)
- **Impact**: LOW (retry succeeds)
- **Detection**: RPC timeout (>1 second)
- **Recovery**: Retry with exponential backoff
- **Mitigation**: Circuit breaker (skip unhealthy shards)

---

### Q5: Simplest Solution - Alternatives?

**ALTERNATIVE A**: Single-machine only (no distribution)
- **Pros**: Simple, zero network overhead
- **Cons**: Limited to 100M docs (~32GB RAM)
- **Verdict**: INSUFFICIENT (can't serve OpenAI-scale)

**ALTERNATIVE B**: Database sharding (PostgreSQL, MongoDB)
- **Pros**: Proven, mature, SQL interface
- **Cons**: 10-100× slower than capsules, complex setup
- **Verdict**: REJECT (performance unacceptable)

**ALTERNATIVE C**: Message queue (Kafka, RabbitMQ)
- **Pros**: Decoupled, async processing
- **Cons**: Complex, latency overhead, not atomic-friendly
- **Verdict**: REJECT (latency + complexity)

**ALTERNATIVE D**: Distributed hash table (Chord, Kademlia)
- **Pros**: Decentralized, fault-tolerant
- **Cons**: O(log N) lookup, complex, eventual consistency
- **Verdict**: REJECT (latency + complexity)

**CHOSEN APPROACH**: T8 Network Capsule (RPC + consistent hashing)
- **Pros**: <10ms latency, deterministic sharding, simple
- **Cons**: Requires N servers, networking code
- **Verdict**: ACCEPT (necessary for scale, benefits >> costs)

---

### Q6: Constraints - What limits exist?

**NETWORK CONSTRAINTS**:
- **Latency**: <10ms within datacenter (cross-datacenter: 50-200ms)
- **Bandwidth**: 1-10 Gbps per server (limits throughput)
- **Packet loss**: 0.01-0.1% typical (retry needed)

**COORDINATION CONSTRAINTS**:
- **CAP theorem**: Can't have all 3 (Consistency, Availability, Partition tolerance)
- **T8 choice**: AP system (Available + Partition-tolerant, eventual consistency)
- **Trade-off**: Stale shard health OK (100ms lag acceptable)

**SCALE CONSTRAINTS**:
- **Shard count**: 1-1024 shards realistic (more = coordination overhead)
- **RPC fanout**: 1-10 shards per query (more = latency)
- **Coordinator limit**: 10K req/sec per coordinator (add more if needed)

**PLATFORM CONSTRAINTS**:
- **Requires**: tokio (async I/O)
- **Requires**: Stable network (local datacenter, not consumer internet)
- **Optional**: Kubernetes (for orchestration, not required)

---

### Q7-Q9: Dependencies, Performance, Trade-offs

**Q7 (Dependencies)**:
- tokio: Async I/O runtime
- bincode: Zero-copy serialization
- Optional: tonic (gRPC framework)

**Q8 (Performance Targets)**:
- RPC latency: <5ms p50, <10ms p99
- Throughput: 100K RPC/sec per coordinator
- Failover: <100ms (detect + promote replica)

**Q9 (Trade-offs)**:
- Maximize: Throughput (2M docs/sec), availability (99.99%)
- Constrain: Latency (<10ms), complexity (simple sharding)
- Accept: Eventual consistency (100ms lag OK)
- Reject: Strong consistency (Paxos/Raft too slow for hot path)

---

## PHASE 2: FOUNDATION (Q10-Q12)

### Q10: Computational Capsule Tier - Why T8?

**TIER: T8 Network** (Distributed Atomic Operations)

**TIER COMPOSITION**: T8 = T1 (Atomic) + Network RPC
- T1 provides: Lockfree atomic operations (local)
- Network provides: RPC to remote atomics
- Combined: Distributed atomic operations

**CAPSULE STRUCTURE**:
```rust
/// Network Shard Capsule - Distributed dedup shard (256B)
///
/// # UCE34 Q10
/// - Tier: T8 Network (distributed coordination)
/// - Why: Scale beyond single-machine (32GB → 3.2TB with 100 shards)
/// - Compound: T8 (distribution) + T9 (persistence) = distributed persistent dedup
///
/// # Performance
/// - Local shard access: <1ms (in-process)
/// - Remote RPC: <10ms (local datacenter)
/// - Failover: <100ms (replica promotion)
///
/// # Reliability
/// - Replication: 2× (primary + replica)
/// - Heartbeat: Every 1 second
/// - Timeout: 10 seconds (mark unhealthy)
#[repr(C, align(256))]
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
pub struct NetworkShardCapsule {
    // Shard identity
    shard_id: u16,               // Which shard (0-1023)
    replica_id: u8,              // Primary (0) or replica (1-3)

    // Network location
    server_ipv4: u32,            // IPv4 address (big-endian)
    server_port: u16,            // TCP port

    // Health monitoring (T1 atomic)
    health_status: AtomicU8,     // 0=healthy, 1=degraded, 2=failed
    last_heartbeat_ns: AtomicU64,  // Last seen timestamp
    documents_count: AtomicU64,  // How many docs in this shard

    // Performance metrics (T1 atomic, EMA)
    rpc_latency_ns: AtomicU64,   // P50 latency
    rpc_errors_total: AtomicU64, // Cumulative errors
    load_percentage: AtomicU8,   // 0-100 (CPU utilization)

    // Coordination (T1 atomic, TOCTOU prevention)
    generation: AtomicU64,       // Generation counter

    _padding: [u8; 168],
}

impl NetworkShardCapsule {
    /// Check if shard is healthy (atomic read)
    #[inline(always)]
    pub fn is_healthy(&self) -> bool {
        let status = self.health_status.load(Ordering::Acquire);
        status == 0  // 0 = healthy
    }

    /// Check if heartbeat is recent (atomic read)
    pub fn heartbeat_fresh(&self, timeout_ns: u64) -> bool {
        let last_seen = self.last_heartbeat_ns.load(Ordering::Acquire);
        let now = current_timestamp_ns();
        (now - last_seen) < timeout_ns
    }

    /// Update heartbeat (called by shard server every 1 second)
    pub fn update_heartbeat(&self) {
        self.last_heartbeat_ns.store(current_timestamp_ns(), Ordering::Release);
        self.health_status.store(0, Ordering::Release);  // Mark healthy
    }

    /// Mark as failed (called by coordinator on timeout)
    pub fn mark_failed(&self) {
        self.health_status.store(2, Ordering::Release);
    }

    /// Record RPC latency (EMA with atomic CAS)
    pub fn record_rpc_latency(&self, latency_ns: u64) {
        const ALPHA_Q16: u64 = 6554;  // 0.1 in Q16 fixed-point

        let mut retries = 0;
        while retries < 8 {
            let old_ema = self.rpc_latency_ns.load(Ordering::Relaxed);
            let new_ema = ((ALPHA_Q16 * latency_ns) + ((65536 - ALPHA_Q16) * old_ema)) / 65536;

            if self.rpc_latency_ns.compare_exchange_weak(
                old_ema, new_ema,
                Ordering::Relaxed, Ordering::Relaxed
            ).is_ok() {
                return;
            }

            retries += 1;
        }
        // Give up after 8 retries (acceptable for approximate EMA)
    }
}
```

---

### Q11: Rust Transform - How does Rust enable this?

**RUST ADVANTAGE 1**: Async/await for RPC
```rust
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Send RPC request to shard server (async, non-blocking)
pub async fn rpc_deduplicate(
    shard: &NetworkShardCapsule,
    documents: Vec<String>,
) -> Result<DeduplicationResponse> {
    // Connect (or reuse connection pool)
    let addr = format!("{}:{}", shard.ip(), shard.port());
    let mut stream = TcpStream::connect(addr).await?;

    // Serialize request (zero-copy with bincode)
    let request = RpcRequest::Deduplicate { documents };
    let bytes = bincode::serialize(&request)?;

    // Send (async write)
    stream.write_u32(bytes.len() as u32).await?;
    stream.write_all(&bytes).await?;

    // Receive response (async read)
    let len = stream.read_u32().await?;
    let mut buf = vec![0u8; len as usize];
    stream.read_exact(&mut buf).await?;

    // Deserialize (zero-copy)
    let response: RpcResponse = bincode::deserialize(&buf)?;

    Ok(response.into())
}
// Zero unsafe code, perfect error handling, async-friendly
```

**RUST ADVANTAGE 2**: Type-safe RPC protocol
```rust
#[derive(serde::Serialize, serde::Deserialize)]
pub enum RpcRequest {
    Deduplicate { documents: Vec<String> },
    Query { signature: MinHashSignatureCapsule },
    Health,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum RpcResponse {
    DeduplicateResult { duplicates: Vec<usize> },
    QueryResult { is_duplicate: bool },
    HealthOk { load: u8 },
    Error(String),
}

// Compiler enforces: All variants handled
// Benefit: Can't forget to handle response type
```

**RUST ADVANTAGE 3**: Connection pooling (Arc + Mutex)
```rust
pub struct ConnectionPool {
    pools: HashMap<SocketAddr, Arc<Mutex<Vec<TcpStream>>>>,
}

impl ConnectionPool {
    pub async fn get_connection(&self, addr: SocketAddr) -> Result<TcpStream> {
        // Reuse existing connection (amortize TCP handshake)
        if let Some(pool) = self.pools.get(&addr) {
            if let Some(conn) = pool.lock().await.pop() {
                return Ok(conn);
            }
        }

        // Create new connection
        TcpStream::connect(addr).await
    }

    pub async fn return_connection(&self, addr: SocketAddr, conn: TcpStream) {
        self.pools.entry(addr)
            .or_insert_with(|| Arc::new(Mutex::new(Vec::new())))
            .lock().await
            .push(conn);
    }
}
// Safe connection reuse, prevents connection leak
```

---

### Q12: Nightly Enhancement - Cutting-edge features?

**OPTIONAL NIGHTLY FEATURES** (T8 doesn't require nightly):

**Feature 1: async_fn_in_trait** (Stabilized in 1.75!)
```rust
trait ShardClient {
    async fn deduplicate(&self, docs: Vec<String>) -> Result<DeduplicationResponse>;
}

// Now stable! Can use in production
```

**Feature 2: type_alias_impl_trait** (Nice-to-have)
```rust
type RpcFuture = impl Future<Output = Result<RpcResponse>>;

pub fn rpc_call_typed(shard: &NetworkShardCapsule) -> RpcFuture {
    async move {
        // Return opaque future type (less type noise)
    }
}
```

**Feature 3: generic_const_exprs** (Parameterized shards)
```rust
pub struct ShardArray<const N: usize>
where
    [(); N * 256]: ,  // N shards × 256B each
{
    shards: [NetworkShardCapsule; N],
}
```

**NIGHTLY STRATEGY**: T8 works on STABLE Rust (no nightly required)
- Benefit: Enterprise customers can use (no nightly requirement)
- Trade-off: Can't use cutting-edge features (but don't need them)

---

## PHASE 3: DOMAIN ANALYSIS (Q13-Q21)

### Q13: Resources - What's needed?

**NETWORK RESOURCES**:
- **Bandwidth**: 100Mbps per server (10MB/sec sustained)
- **Latency**: <10ms within datacenter (same region)
- **Connections**: 100 shards × 10 connections = 1000 TCP connections
- **Cost**: Included in server cost (cloud networking free within region)

**SERVER RESOURCES** (per shard):
- **CPU**: 16 cores (dedup processing)
- **RAM**: 32GB (10M signatures × 256B = 2.5GB + overhead)
- **Disk**: 100GB SSD (persistent index if using T9)
- **Network**: 1Gbps NIC
- **Cost**: $200/month per server (Hetzner, AWS, etc.)

**COORDINATOR RESOURCES**:
- **CPU**: 4 cores (routing only, not compute)
- **RAM**: 8GB (shard metadata only)
- **Network**: 10Gbps (fanout to 100 shards)
- **Cost**: $100/month (cheaper than shard servers)

**TOTAL INFRASTRUCTURE** (100-shard deployment):
- 100 shard servers: $20K/month
- 3 coordinators (Raft): $300/month
- Load balancers: $200/month
- Monitoring: $500/month
- **Total**: $21K/month for 2M docs/sec capacity

---

### Q14: Scalability - Growth scenarios?

**SCALING SCENARIO 1**: Startup (10M docs)
- **Shards**: 1 server (no distribution needed)
- **Cost**: $200/month
- **Latency**: <1ms (local access)
- **Simple**: No network overhead

**SCALING SCENARIO 2**: Mid-Market (100M docs)
- **Shards**: 10 servers (10M docs each)
- **Cost**: $2K/month
- **Latency**: <10ms (RPC to shard)
- **Complexity**: Coordinator + consistent hashing

**SCALING SCENARIO 3**: Enterprise (1B docs)
- **Shards**: 100 servers (10M docs each)
- **Cost**: $20K/month
- **Latency**: <10ms p99 (99% local, 1% cross-shard)
- **Complexity**: Raft coordinators, replicas, monitoring

**SCALING SCENARIO 4**: OpenAI-Scale (100B docs)
- **Shards**: 10,000 servers (10M docs each)
- **Cost**: $2M/month infrastructure
- **Latency**: <20ms p99 (multi-hop routing)
- **Complexity**: Regional sharding (US, EU, APAC), federation

**ELASTICITY**:
```
Traffic Pattern    | Response
──────────────────────────────────────────────────────────────
Spike (10× load)   | Add 10× servers (Kubernetes autoscale)
Drop (0.1× load)   | Remove 90% servers (scale down)
Regional (US only) | Add US shards, remove EU/APAC
```

---

### Q15-Q21: Security, Interface, Testing, Monitoring, Errors, Lifecycle

**Q15 (Security)**:
- TLS for RPC (encrypt in transit)
- mTLS for auth (shard ↔ coordinator)
- API keys for clients (rate limiting)

**Q16 (Interface)**:
```rust
pub trait ShardClient {
    async fn deduplicate(&self, docs: Vec<String>) -> Result<Vec<usize>>;
    async fn query(&self, sig: &MinHashCapsule) -> Result<bool>;
    async fn health(&self) -> Result<HealthStatus>;
}
```

**Q17 (Testing)**:
- Unit: Shard routing, heartbeat, health checks
- Property: Failover, load balancing, partition tolerance
- Integration: Multi-shard dedup, RPC correctness
- Production: 100-shard stress, network partition, cascading failures

**Q18 (Monitoring)**:
- Shard health (healthy/degraded/failed count)
- RPC latency (p50/p99 per shard)
- Network bandwidth (MB/sec per shard)
- Error rate (RPC failures, timeouts)

**Q19 (Error Handling)**:
- RPC timeout: Retry with exponential backoff
- Shard unavailable: Route to replica
- Network partition: Degrade gracefully (serve from available shards)
- Coordinator failure: Raft failover (standby promotes)

**Q20 (Lifecycle)**:
- Bootstrap: Start shards, register with coordinator
- Operate: Handle RPC, update heartbeats
- Failover: Detect failure, promote replica
- Scale: Add/remove shards dynamically
- Shutdown: Drain requests, flush state

---

## PHASE 4: IMPLEMENTATION (Q22-Q30)

### Q22-Q24: State, Concurrency, Memory Layout

**Q22 (State Management)**:
- Coordinator state: Shard registry (100-1024 shards)
- Shard state: Local dedup index (T9 persistent)
- Coordination: Heartbeats (atomic timestamps)

**Q23 (Concurrency)**:
- Coordinator: Single-threaded (tokio async, 10K req/sec sufficient)
- Shards: Multi-threaded (16 cores per shard)
- RPC: Connection pool (reuse TCP connections)

**Q24 (Memory Layout)**:
- NetworkShardCapsule: 256B (shard metadata)
- Array of 1024 shards: 256KB total (fits in L3 cache)
- Hot path: Shard selection (<10ns), RPC dispatch (~10ms)

---

### Q25-Q30: Verification, Optimization, Composition, Migration, Docs, Production

**Q25 (Verification)**:
- #[derive(ComputationalCapsule)] on NetworkShardCapsule
- Multi-shard integration tests
- Chaos engineering (random failures)

**Q26 (Optimization)**:
- Connection pooling (amortize TCP handshake)
- Batch RPC (send 100 docs in one RPC, not 100 RPCs)
- Pipelining (send next request before previous response)

**Q27 (Composition)**:
- T8 + T9: Distributed persistent dedup (mmap per shard)
- T8 + T1: Distributed atomic counters (global statistics)
- T8 + T10: Distributed probabilistic structures (sharded LSH)

**Q28 (Migration)**:
- Add shards: Consistent hashing (minimal rebalancing)
- Remove shards: Drain + redistribute (zero downtime)
- Version: Protocol version field (forward/backward compat)

**Q29 (Documentation)**:
- This UCE34 doc
- RPC protocol spec
- Deployment guide (Kubernetes, Docker Swarm)

**Q30 (Production Readiness)**:
- T28 tests (40+ tests for multi-shard scenarios)
- B32 benchmarks (RPC latency, throughput)
- Chaos tests (Jepsen-style partition testing)

---

## PHASE 5: REFINEMENT (Q31-Q34)

### Q31-Q34: Simplicity, Constraints, Validation, Auditability

**Q31 (Simplicity)**:
- 3 components: Coordinator, Shard, Client
- 5 RPC methods: Deduplicate, Query, Health, Register, Unregister
- Zero external coordination (no Zookeeper, no etcd)

**Q32 (Constraints)**:
- Datacenter-only (not consumer internet, <10ms latency required)
- Stable Rust (no nightly features needed)
- TCP-based (not UDP, reliability over absolute lowest latency)

**Q33 (Validation)**:
- Property tests (Quickcheck for shard assignment)
- Chaos tests (Jepsen for partition tolerance)
- Load tests (100 shards × 1K RPC/sec)

**Q34 (Auditability)**:
- RPC logs (who called what, when)
- Shard assignment audit (which doc went to which shard)
- Failover audit (when did shard fail, which replica promoted)

---

## Part 6: Distributed LLM Dedup Architecture

### Complete System Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                     Load Balancer                            │
│                     (Cloudflare, NGINX)                      │
└────────────┬─────────────────────────────────────────────────┘
             │
    ┌────────┴────────┬────────────────┐
    │                 │                │
┌───▼────┐      ┌─────▼───┐      ┌────▼────┐
│Coord 1 │      │Coord 2  │      │Coord 3  │  (Raft quorum)
│(Leader)│      │(Follower│      │(Follower│
└───┬────┘      └─────┬───┘      └────┬────┘
    │                 │                │
    └─────────────────┴────────────────┘
                      │
         ┌────────────┼────────────┬─────────────┐
         │            │            │             │
    ┌────▼──┐    ┌───▼──┐    ┌────▼──┐    ┌────▼──┐
    │Shard 0│    │Shard1│    │Shard2 │... │Shard99│
    │(Prim) │    │(Prim)│    │(Prim) │    │(Prim) │
    └───┬───┘    └──┬───┘    └───┬───┘    └───┬───┘
        │           │            │            │
    ┌───▼───┐    ┌─▼────┐    ┌──▼────┐    ┌─▼─────┐
    │Shard 0│    │Shard│    │Shard2 │... │Shard99│
    │(Repl) │    │1(Rpl)│    │(Repl) │    │(Repl) │
    └───────┘    └──────┘    └───────┘    └───────┘
```

**DATA FLOW**:
```
Client request: Deduplicate 1000 documents
  ↓
Load balancer: Route to Coord 1 (leader)
  ↓
Coord 1: For each document:
  ├─ Compute LSH bucket (fast, <100ns)
  ├─ Shard ID = bucket % 100
  └─ Batch docs by shard (100 docs → 10 shards avg)
  ↓
Coord 1: Send 10 RPC requests (parallel)
  ├─ RPC to Shard 0: 100 docs
  ├─ RPC to Shard 5: 95 docs
  ├─ RPC to Shard 12: 102 docs
  └─ ... (10 shards total)
  ↓
Each Shard: Process locally
  ├─ Query LSH index (local)
  ├─ Find duplicates (local MinHash comparison)
  └─ Return results
  ↓
Coord 1: Merge results
  ├─ Aggregate duplicate lists
  └─ Return to client
  ↓
Client receives: [5, 17, 23, ...] (duplicate indices)

Total latency: ~15ms (10ms RPC + 5ms processing)
```

---

### Consistent Hashing (Shard Assignment)

**ALGORITHM**:
```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub struct ConsistentHashRing {
    /// Virtual nodes (100 shards × 150 vnodes = 15K points)
    vnodes: Vec<(u64, u16)>,  // (hash, shard_id)
}

impl ConsistentHashRing {
    pub fn new(shard_count: u16) -> Self {
        const VNODES_PER_SHARD: u16 = 150;  // Even distribution

        let mut vnodes = Vec::new();

        for shard_id in 0..shard_count {
            for vnode in 0..VNODES_PER_SHARD {
                let mut hasher = DefaultHasher::new();
                (shard_id, vnode).hash(&mut hasher);
                let hash = hasher.finish();

                vnodes.push((hash, shard_id));
            }
        }

        // Sort by hash (for binary search)
        vnodes.sort_by_key(|(hash, _)| *hash);

        Self { vnodes }
    }

    /// Find shard for LSH bucket (consistent hashing)
    pub fn get_shard(&self, lsh_bucket: u16) -> u16 {
        let mut hasher = DefaultHasher::new();
        lsh_bucket.hash(&mut hasher);
        let bucket_hash = hasher.finish();

        // Binary search for closest vnode
        let idx = self.vnodes.binary_search_by_key(&bucket_hash, |(h, _)| *h)
            .unwrap_or_else(|i| i % self.vnodes.len());

        self.vnodes[idx].1  // Return shard_id
    }

    /// Add new shard (rebalances only K/N keys, not all)
    pub fn add_shard(&mut self, shard_id: u16) {
        // Add 150 vnodes for new shard
        // Only affects keys near new vnodes (minimal rebalancing)
    }

    /// Remove shard (keys redistribute to next vnode)
    pub fn remove_shard(&mut self, shard_id: u16) {
        // Remove all vnodes for this shard
        // Keys redistribute evenly (no single shard hotspot)
    }
}
```

**BENEFIT**: Add/remove shards with <5% key migration (vs 50% for naive modulo)

---

## Part 7: RPC Protocol Specification

### Wire Format (Binary Protocol)

**REQUEST FORMAT**:
```
[4 bytes] Message length (u32, little-endian)
[1 byte]  RPC method ID (0=Deduplicate, 1=Query, 2=Health)
[N bytes] Payload (bincode-serialized)

Example (Deduplicate request):
00 00 04 00  (length: 1024 bytes)
00           (method: Deduplicate)
...          (bincode payload: Vec<String>)
```

**RESPONSE FORMAT**:
```
[4 bytes] Message length
[1 byte]  Status code (0=OK, 1=Error)
[N bytes] Payload (bincode-serialized result)

Example (Success):
00 00 00 64  (length: 100 bytes)
00           (status: OK)
...          (bincode payload: Vec<usize> duplicate indices)

Example (Error):
00 00 00 32  (length: 50 bytes)
01           (status: Error)
...          (bincode payload: String error message)
```

**PERFORMANCE**:
- Serialization: <100μs for 1K docs (bincode is fast)
- Network: ~10ms (1Gbps network, <1MB payload)
- Deserialization: <100μs
- **Total RPC latency**: ~10ms (dominated by network)

---

### Heartbeat Protocol

**SHARD → COORDINATOR** (every 1 second):
```
[1 byte] Method ID (3=Heartbeat)
[2 bytes] Shard ID
[8 bytes] Documents count
[1 byte] Load percentage (0-100)
[8 bytes] Timestamp
```

**COORDINATOR → SHARD** (health check request):
```
[1 byte] Method ID (2=Health)

Response:
[1 byte] Status (0=OK)
[1 byte] Load (0-100)
```

**FAILURE DETECTION**:
```
Coordinator checks (every 5 seconds):
  For each shard:
    If last_heartbeat > 10 seconds ago:
      Mark shard as DEGRADED
    If last_heartbeat > 30 seconds ago:
      Mark shard as FAILED
      Promote replica to primary
```

---

## Part 8: Fault Tolerance & High Availability

### Replication Strategy

**PRIMARY-REPLICA PATTERN**:
```
Each logical shard → 2 physical servers

Shard 0: Primary (server-01) + Replica (server-51)
Shard 1: Primary (server-02) + Replica (server-52)
...
Shard 99: Primary (server-100) + Replica (server-150)

Total: 200 servers (100 primary + 100 replica)
```

**WRITE PATH** (synchronous replication):
```
Client → Coordinator → Primary shard
  ↓
Primary processes (deduplicate locally)
  ↓
Primary replicates to Replica (async, best-effort)
  ↓
Primary returns result to coordinator
  ↓
Coordinator returns to client

Latency: ~10ms (primary only, replica async)
Durability: 99.9% (primary persisted, replica eventual)
```

**FAILOVER PATH** (automatic):
```
Coordinator detects: Primary shard 0 failed (no heartbeat 30 seconds)
  ↓
Coordinator promotes: Replica shard 0 → new Primary
  ↓
Coordinator updates: Shard registry (route to new primary)
  ↓
Coordinator notifies: Load balancer (update routing table)

Failover time: <100ms (detect 30s + promote <1s, but health checked every 5s)
Actual: ~35 seconds (next health check cycle)
```

**SPLIT-BRAIN PREVENTION**:
```
If network partition:
  Coordinator A sees: Shards 0-49 (healthy), 50-99 (failed)
  Coordinator B sees: Shards 50-99 (healthy), 0-49 (failed)

Raft consensus:
  ├─ Only LEADER can promote replicas
  ├─ Followers reject promotion requests
  └─ Quorum (2/3) required for leader election

Result: Only one coordinator promotes (no split-brain)
```

---

## Part 9: Implementation Checklist

### Files to Create

**Core Network Implementation**:
1. **`src/network/mod.rs`** (100 LOC)
2. **`src/network/shard_capsule.rs`** (300 LOC) - NetworkShardCapsule
3. **`src/network/rpc_client.rs`** (400 LOC) - Async RPC client
4. **`src/network/rpc_server.rs`** (400 LOC) - Async RPC server
5. **`src/network/coordinator.rs`** (500 LOC) - Shard coordinator
6. **`src/network/consistent_hash.rs`** (300 LOC) - Shard routing

**Testing**:
7. **`tests/network_tests.rs`** (600 LOC) - T28 4-tier tests
8. **`tests/chaos_tests.rs`** (400 LOC) - Partition, failures

**Benchmarks**:
9. **`benches/network_bench.rs`** (300 LOC) - RPC latency, throughput

**Total**: ~3,300 LOC

---

### Dependencies

```toml
[dependencies]
tokio = { version = "1.0", features = ["full"] }
bincode = "1.3"  # Zero-copy serialization
serde = { version = "1.0", features = ["derive"] }

# Optional (for gRPC instead of custom protocol)
tonic = { version = "0.11", optional = true }
prost = { version = "0.12", optional = true }

[features]
network = ["std", "dep:tokio", "dep:bincode"]
network-grpc = ["network", "dep:tonic", "dep:prost"]  # Alternative RPC
```

---

## Part 10: Performance Modeling

### Throughput Analysis

**SINGLE-SHARD THROUGHPUT**: 20K docs/sec (from local benchmarks)

**DISTRIBUTED THROUGHPUT** (100 shards):
```
Ideal: 100 shards × 20K docs/sec = 2M docs/sec
Realistic (with overhead):
  ├─ RPC overhead: 10% (networking, serialization)
  ├─ Load imbalance: 20% (hot shards get 2× traffic)
  ├─ Coordinator limit: 5% (routing bottleneck)
  └─ Network saturation: 5% (1Gbps limits)

Actual: 2M × 0.9 × 0.8 × 0.95 × 0.95 = 1.3M docs/sec

100B documents: 76,923 seconds = 21.4 hours ✅
```

**vs SINGLE-MACHINE**:
- Single: 20K docs/sec → 100B docs = 5M seconds = 57 days
- Distributed: 1.3M docs/sec → 100B docs = 21.4 hours
- **Speedup**: 65× faster with 100 shards (sub-linear but acceptable)

---

### Cost-Benefit Analysis

**INFRASTRUCTURE COST** (100-shard deployment):
- 100 primary shards: $20K/month
- 100 replica shards: $20K/month
- 3 coordinators: $300/month
- Load balancers: $500/month
- Monitoring: $500/month
- **Total**: $41.3K/month

**REVENUE MODEL** (to justify cost):
- OpenAI contract: $500K/year = $42K/month ✅ (break-even on 1 customer!)
- Meta contract: $300K/year = $25K/month
- Anthropic contract: $200K/year = $17K/month
- **Total**: $84K/month revenue - $41K infrastructure = $43K profit (51% margin)

**MARGIN ANALYSIS**:
- Gross margin: 51% (infrastructure-heavy)
- Net margin (after partner split): 25.5% ($10.75K each)
- **Verdict**: ACCEPTABLE for enterprise scale (51% is good for infrastructure business)

---

## Conclusion

**T8 Network Capsule**: ✅ **CRITICAL for Enterprise Scale**

**Why**:
- Enables 100B+ document dedup (OpenAI/Google scale)
- Unlocks $500K+ enterprise deals (vs $100K mid-market)
- 65× speedup with 100 shards (21 hours vs 57 days)

**Complexity**: HIGH (distributed systems are hard)

**Timeline**: 2-3 weeks to implement (async RPC, sharding, failover)

**Priority**: MEDIUM (implement Month 6-9 when enterprise deals need it)

**Status**: ✅ **APPROVED** - Design complete, defer until enterprise customer requires

**Recommendation**: Launch without T8 (single-server sufficient for $1M ARR), add T8 when enterprise customer needs >1B docs

---

**Next Primitive**: T10.1 HyperLogLog (cardinality estimation)
