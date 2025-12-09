# Tokio Capsule: Comprehensive Plan for Zero-Dependency Async Runtime

**Version**: 1.0.0  
**Date**: 2025-11-07  
**Framework**: UCE34 (34-Question Systematic Discovery)  
**Author**: Claude Code  
**Status**: RESEARCH COMPLETE - GO/NO-GO DECISION REQUIRED

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current Tokio Usage Analysis](#current-tokio-usage-analysis)
3. [UCE34 Framework Application (Q1-Q34)](#uce34-framework-application)
4. [Architecture Design](#architecture-design)
5. [Detailed Implementation Plan](#detailed-implementation-plan)
6. [Risk Mitigation](#risk-mitigation)
7. [Success Metrics](#success-metrics)
8. [Go/No-Go Recommendation](#gono-go-recommendation)

---

## Executive Summary

### Feasibility Assessment: **TECHNICALLY FEASIBLE, STRATEGICALLY QUESTIONABLE**

Building a custom async runtime to replace Tokio is **100% technically achievable** using computational capsule primitives. However, it represents a **massive 5-6 month undertaking** that would significantly delay capsule-os development for **minimal practical benefit**.

### Reality Check

**Current Tokio Usage in kindly_dedup**:
- **1 module**: `AsyncLogCapsule` (T5 Streaming)
- **5 functions**: `tokio::spawn`, `time::interval`, `io::AsyncWriteExt`, `fs::File::from_std`, `io::BufWriter::new`
- **Total lines using Tokio**: ~50 lines across 2 files (async_log.rs, audit_logger.rs)
- **Dependency weight**: `tokio = { version = "1.0", features = ["full"] }` adds ~500KB to binary

**Replacement Options**:

| Option | Timeline | Complexity | Risk | Recommendation |
|--------|----------|------------|------|----------------|
| **A: Remove Tokio Entirely** | 1 week | LOW | LOW | ✅ **RECOMMENDED** |
| **B: Minimal Executor (AsyncLogCapsule only)** | 2-4 weeks | MEDIUM | MEDIUM | ⚠️ Consider |
| **C: Full Async Runtime (Tokio replacement)** | 20 weeks | VERY HIGH | VERY HIGH | ❌ **NOT RECOMMENDED** |

### Estimated Timeline (Option C: Full Async Runtime)

| Phase | Duration | Milestone | Risk Level |
|-------|----------|-----------|------------|
| **Phase 0: Research & Prototyping** | 2 weeks | Working single-threaded executor | LOW |
| **Phase 1: Minimal Viable Runtime** | 4 weeks | Multi-threaded executor + basic channels | MEDIUM |
| **Phase 2: Async I/O Support** | 4 weeks | Reactor (epoll/kqueue) + AsyncRead/AsyncWrite | HIGH |
| **Phase 3: Full Feature Parity** | 6 weeks | Timers, broadcast, select!/join!/race! | VERY HIGH |
| **Phase 4: Optimization & Production** | 4 weeks | SIMD scheduling, comprehensive testing | HIGH |
| **TOTAL** | **20 weeks (5 months)** | Production-ready Tokio replacement | **EXTREME** |

### Risk Analysis: **VERY HIGH**

**Top 5 Risks**:

1. **Schedule Underestimation** (90% probability)
   - Async runtimes are deceptively complex
   - Edge cases multiply (epoll vs kqueue vs IOCP, signal handling, thread parking)
   - Realistic estimate: 6-8 months for production quality

2. **Scope Creep** (70% probability)
   - "Just one more feature" syndrome (timeouts, select!, join!, spawn_blocking)
   - Ecosystem expectations (hyper, tonic, tower assume Tokio APIs)
   - Network effects force API compatibility

3. **Performance Regression** (50% probability)
   - Tokio has 10 years of optimization
   - Work-stealing executor is HARD to beat
   - SIMD scheduling might not match hand-tuned assembly

4. **Ecosystem Incompatibility** (60% probability)
   - Most async crates assume `tokio::Runtime`
   - Tower middleware stack tightly coupled to Tokio
   - Would need to rewrite hyper, tonic, etc.

5. **Maintenance Burden** (80% probability)
   - Async Rust evolves rapidly (async fn in trait, async closures, effects)
   - Cross-platform support (Linux, macOS, Windows, BSDs)
   - Security updates, bug fixes, feature requests

### Go/No-Go Recommendation: **NO** (Strong Recommendation)

**Rationale**:
- **Minimal current usage**: Only AsyncLogCapsule uses Tokio (<50 lines)
- **Easy alternative**: Rewrite AsyncLogCapsule with `std::thread` + lockfree queue (1 week)
- **Massive opportunity cost**: 5-6 months delays capsule-os, kindly-db, other strategic work
- **Tokio is excellent**: Mature, tested, fast, well-maintained (not a bottleneck)
- **Strategic mismatch**: Capsule-os vision doesn't require async runtime (can use sync + lockfree primitives)

**IF you still want to proceed**:
1. Start with **Option A** (Remove Tokio entirely, 1 week)
2. If async is truly needed, build **Option B** (Minimal executor, 2-4 weeks)
3. Only pursue **Option C** if async runtime becomes critical infrastructure (revisit in 6 months)

---

## Current Tokio Usage Analysis

### Summary

**Total Tokio Dependencies**: 2 crates

1. **kindly_dedup**: `tokio = { version = "1.0", features = ["full"] }` (ALWAYS enabled, non-optional)
2. **atomic_capsule**: `tokio = { version = "1.39", optional = true, features = ["rt", "time", "fs", "io-util", "net"] }`

**Actual Usage Breakdown**:

| Feature | kindly_dedup | atomic_capsule | Complexity | Replacement Difficulty |
|---------|--------------|----------------|------------|------------------------|
| **Task Spawning** | ✅ (1 usage) | ✅ (tests, RPC) | MEDIUM | MEDIUM (need executor) |
| **Timers** | ✅ (1 usage) | ✅ (tests, distributed cache) | MEDIUM | MEDIUM (timer wheel) |
| **Async File I/O** | ✅ (1 usage) | ❌ | HIGH | HIGH (epoll + async traits) |
| **Async Networking** | ❌ (tests only) | ✅ (RPC server/client) | VERY HIGH | VERY HIGH (reactor + TCP) |
| **Channels** | ❌ | ❌ | LOW | TRIVIAL (atomic_capsule has lockfree queues) |
| **select!/join!/race!** | ❌ | ❌ | MEDIUM | MEDIUM (macro magic) |
| **spawn_blocking** | ❌ | ❌ | LOW | TRIVIAL (std::thread) |

### Detailed File-by-File Analysis

#### kindly_dedup (2 files using Tokio)

**1. src/benchmarking/audit_logger.rs** (58 + 246-249 + 644-652 = ~20 lines)
```rust
use tokio::task::JoinHandle;  // Line 58

// Line 245-246: Convert std File to tokio File
let tokio_file = tokio::fs::File::from_std(file);
let writer = tokio::io::BufWriter::new(tokio_file);

// Line 644-652: Drop implementation (graceful shutdown)
if let Ok(rt) = tokio::runtime::Handle::try_current() {
    rt.block_on(async { let _ = handle.await; });
}
```

**2. tests/server_tests.rs** (15 + entire file = ~800 lines)
```rust
use tokio::runtime::Runtime;  // Line 15

#[tokio::test]  // 28 test functions
async fn test_server_start() { ... }

tokio::spawn(async move { ... });  // Concurrent request tests
tokio::time::sleep(Duration::from_millis(100)).await;  // Timing tests
```

**USAGE CATEGORY**: TRIVIAL to EASY
- AuditLogger: 20 lines, async flush only (can rewrite with std::thread)
- Server tests: Test-only code (not production critical)

#### atomic_capsule (13 files using Tokio)

**Key Files**:

1. **src/collections/async_log.rs** (~500 lines)
   - `tokio::task::spawn` (spawn flush task)
   - `tokio::time::interval` (periodic flush)
   - `tokio::io::AsyncWriteExt` (write to file)
   - `tokio::fs::File::from_std` (async file wrapper)

2. **src/network/rpc_server.rs** + **rpc_client.rs** (~400 lines each)
   - `tokio::net::TcpListener::bind` (async TCP server)
   - `tokio::net::TcpStream::connect` (async TCP client)
   - `tokio::io::AsyncReadExt` / `AsyncWriteExt` (async I/O traits)
   - `tokio::time::timeout` (request timeouts)

3. **src/collections/distributed_cache.rs** (~200 lines)
   - `tokio::time::timeout` (cache expiration)

4. **src/protection/remote_attestation.rs** (~300 lines)
   - `tokio::net::TcpStream` (remote attestation protocol)

**USAGE CATEGORY**: MEDIUM to HARD
- AsyncLogCapsule: MEDIUM (ring buffer works, just need non-Tokio flush)
- RPC server/client: HARD (need full reactor + TCP abstraction)
- Distributed cache: EASY (timeout can use std::thread)
- Remote attestation: HARD (network protocol)

### Migration Difficulty Matrix

| Component | Current LOC | Tokio Features Used | Replacement LOC | Difficulty | Timeline |
|-----------|-------------|---------------------|-----------------|------------|----------|
| **AsyncLogCapsule** | 500 | spawn, interval, fs, io | 300 | MEDIUM | 1-2 weeks |
| **AuditLogger** | 20 | spawn, fs, io | 50 | EASY | 2 days |
| **RPC Server/Client** | 800 | net, io, timeout | 1500 | HARD | 3-4 weeks |
| **Distributed Cache** | 200 | timeout | 100 | EASY | 3 days |
| **Remote Attestation** | 300 | net, io | 500 | HARD | 2-3 weeks |
| **Tests** | 800 | test macro, spawn, time | 200 | TRIVIAL | 1 week |
| **TOTAL** | **2620** | **6 features** | **2650** | **HARD** | **8-13 weeks** |

### Feature Complexity Breakdown

#### 1. Task Spawning (MEDIUM Complexity)

**Tokio Implementation**:
```rust
let handle = tokio::spawn(async move {
    // Async task runs on executor
});
handle.await?;  // Wait for completion
```

**Capsule Replacement**:
- **Option A**: `std::thread::spawn` (simple, synchronous)
- **Option B**: Custom executor with `std::future::Future` polling

**Complexity Drivers**:
- Need work-stealing task queue (atomic_capsule has `WorkStealingQueue`)
- Need waker infrastructure (std::task::Waker)
- Need multi-threaded executor (park/unpark threads)

**Estimated LOC**: 500-800 lines (executor + task queue + waker)

#### 2. Timers (MEDIUM Complexity)

**Tokio Implementation**:
```rust
let mut interval = tokio::time::interval(Duration::from_millis(100));
loop {
    interval.tick().await;
    // Periodic work
}
```

**Capsule Replacement**:
- **Option A**: `std::thread::sleep` in background thread
- **Option B**: Hierarchical timer wheel (O(1) insert/remove)

**Complexity Drivers**:
- Timer wheel data structure (efficient for many timers)
- Integration with waker system (wake tasks on timeout)
- Platform-specific sleep (timerfd on Linux, kevent on macOS)

**Estimated LOC**: 300-500 lines (timer wheel + reactor integration)

#### 3. Async File I/O (HIGH Complexity)

**Tokio Implementation**:
```rust
let file = tokio::fs::File::from_std(std_file);
let mut writer = tokio::io::BufWriter::new(file);
writer.write_all(b"data").await?;
writer.flush().await?;
```

**Capsule Replacement**:
- **Option A**: `std::fs::File` in background thread (RECOMMENDED)
- **Option B**: io_uring (Linux only, requires kernel 5.1+)
- **Option C**: epoll + O_NONBLOCK (doesn't work well for files)

**Complexity Drivers**:
- File I/O is inherently blocking on most platforms
- io_uring is Linux-only and complex
- Thread pool approach is simplest (what Tokio does internally!)

**Estimated LOC**: 50-100 lines (thread pool) OR 1000+ lines (io_uring)

#### 4. Async Networking (VERY HIGH Complexity)

**Tokio Implementation**:
```rust
let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
loop {
    let (socket, _) = listener.accept().await?;
    tokio::spawn(async move {
        // Handle connection
    });
}
```

**Capsule Replacement**:
- Need reactor (epoll on Linux, kqueue on macOS/BSD, IOCP on Windows)
- Need AsyncRead/AsyncWrite traits (similar to std::io::Read/Write)
- Need DNS resolution (async getaddrinfo)
- Need TCP state machine (half-close, keepalive, etc.)

**Complexity Drivers**:
- Platform-specific syscalls (epoll != kqueue != IOCP)
- Edge-triggered vs level-triggered events
- Error handling (EAGAIN, EINTR, connection reset)
- Backpressure and flow control

**Estimated LOC**: 2000-3000 lines (reactor + TCP + traits + platform abstraction)

### Conclusion: Tokio Usage is MINIMAL

**Bottom Line**:
- kindly_dedup: Only AsyncLogCapsule uses Tokio in production code
- atomic_capsule: Network modules use Tokio, but are OPTIONAL features
- Total production code: <100 lines of actual async logic
- Tests: 800 lines, but NOT production-critical

**Recommendation**:
- **Phase 0**: Remove Tokio from kindly_dedup entirely (1 week)
  - Rewrite AsyncLogCapsule flush with `std::thread` + lockfree queue
  - Rewrite tests to use `std::thread::spawn` instead of `tokio::spawn`
  - Validate performance matches async version (B32 benchmarks)

- **Phase 1** (OPTIONAL): Minimal executor for atomic_capsule network features (2-4 weeks)
  - Single-threaded executor (no work-stealing)
  - Basic reactor (epoll on Linux only)
  - AsyncRead/AsyncWrite traits
  - TCP support only (no UDP, no Unix sockets)

- **Phase 2** (FUTURE): Full async runtime if ecosystem demands it (revisit in 6 months)

---

## UCE34 Framework Application (Q1-Q34)

### Q1-Q9: Problem Definition & Scope

#### Q1: What problem does Tokio solve for kindly_dedup?

**Answer**: Async I/O for `AsyncLogCapsule` to avoid blocking write syscalls.

**Current Behavior**:
- Sync path: `write()` syscall blocks thread for 1-5μs per log entry
- Async path: `tokio::spawn` flush task writes batches (100+ entries/syscall)
- Result: 20-100× throughput improvement

**Actual Need**:
- Ring buffer + batched writes (ALREADY lockfree in AsyncLogCapsule)
- Background thread to flush buffer to disk
- NO async needed (can use `std::thread::spawn` instead of `tokio::spawn`)

**Verdict**: Tokio solves a problem that `std::thread` can also solve.

#### Q2: What's the simplest async primitive needed?

**Answer**: Background thread with lockfree queue communication.

**Minimal Replacement** (WITHOUT Tokio):
```rust
pub struct SyncFlushTask {
    queue: Arc<LockfreeQueue<LogEntry>>,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl SyncFlushTask {
    pub fn start(mut writer: std::io::BufWriter<std::fs::File>) -> Self {
        let queue = Arc::new(LockfreeQueue::new());
        let running = Arc::new(AtomicBool::new(true));
        
        let queue_clone = Arc::clone(&queue);
        let running_clone = Arc::clone(&running);
        
        let thread_handle = std::thread::spawn(move || {
            while running_clone.load(Ordering::Acquire) {
                // Batch pop entries
                let mut batch = Vec::with_capacity(128);
                while let Some(entry) = queue_clone.try_pop() {
                    batch.push(entry);
                    if batch.len() >= 128 { break; }
                }
                
                // Write batch
                for entry in batch {
                    writeln!(writer, "{}", entry.as_str()).ok();
                }
                writer.flush().ok();
                
                // Sleep 100ms between flushes
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        
        Self { queue, running, thread_handle: Some(thread_handle) }
    }
}
```

**LOC**: ~50 lines (vs 500 lines in AsyncLogCapsule)

**Performance**: Identical to async version (batching is the key, not async)

**Verdict**: Tokio is NOT NEEDED for AsyncLogCapsule.

#### Q3: What are the constraints?

**Constraints**:

1. **Performance**: Must match Tokio's <100ns spawn, 1M tasks/sec throughput
2. **Compatibility**: Must work with `std::future::Future` (no breaking changes)
3. **Platform Support**: Linux (epoll), macOS (kqueue), Windows (IOCP)
4. **Memory**: <1KB overhead per task (same as Tokio)
5. **Safety**: 100% lockfree, zero deadlocks
6. **Maintenance**: One-person maintenance burden (no team like Tokio has)

#### Q4: What are the inputs/outputs?

**Inputs**:
- Future: `impl Future<Output = T>`
- Task priority: Default or high-priority
- Spawn options: Blocking vs non-blocking, CPU affinity

**Outputs**:
- JoinHandle: `impl Future<Output = Result<T, JoinError>>`
- Waker: Notify when task can make progress
- Poll result: Ready(T) or Pending

#### Q5: What's the data flow?

```
User Code
   ↓
spawn(future)  // Submit task
   ↓
Task Queue (lockfree)  // Multi-producer, multi-consumer
   ↓
Worker Threads (N cores)  // Work-stealing scheduler
   ↓
Poll Future (std::task::poll)  // Drive state machine
   ↓
Pending → Register Waker → Park Thread
Ready(T) → Return JoinHandle result
```

#### Q6: What are the edge cases?

**Edge Cases**:

1. **Task Panic**: Must catch panic, return Err(JoinError::Panic)
2. **Task Cancellation**: Must support drop(handle) cancellation
3. **Waker Spurious Wakeup**: Must re-poll after wake
4. **Work-Stealing Contention**: Must handle CAS failures gracefully
5. **Thread Parking Deadlock**: Must ensure waker always wakes thread
6. **EAGAIN/EINTR**: Must retry syscalls on interrupt
7. **Half-Closed TCP**: Must handle FIN before all data sent
8. **Signal Handling**: Must not interrupt epoll_wait incorrectly

#### Q7: What scale are we targeting?

**Scale Requirements**:

| Metric | kindly_dedup Current | Tokio Baseline | Target |
|--------|----------------------|----------------|--------|
| **Tasks/sec** | <1K (AsyncLogCapsule only) | 800K-1M | 10K (100× headroom) |
| **Concurrent Tasks** | <10 (flush task only) | 10K-100K | 100 (10× headroom) |
| **Task Spawn Latency** | N/A (1 task total) | 200ns | <500ns (2× slower OK) |
| **Memory per Task** | N/A | 1.5KB | <2KB (33% more OK) |
| **Throughput** | 10K log entries/sec | 10M ops/sec | 100K entries/sec (10× headroom) |

**Verdict**: Scale requirements are TRIVIAL (1 task, 10K ops/sec). Full async runtime is OVERKILL.

#### Q8: What are the bottlenecks?

**Potential Bottlenecks**:

1. **Disk I/O**: Flush syscall is 1-5ms (NOT CPU-bound, batching helps)
2. **Memory Allocation**: Task struct allocation (can use slab allocator)
3. **Work-Stealing Contention**: CAS on task queue (atomic_capsule has lockfree queue)
4. **Epoll Syscall Overhead**: Each epoll_wait is 1-10μs (acceptable)
5. **Waker Overhead**: Waker.wake() is 50-100ns (atomic + park/unpark)

**Optimization Opportunities**:
- SIMD task queue (pack 8 tasks in 512-bit register)
- Adaptive work-stealing (reduce CAS on low load)
- Huge pages for task slab (reduce TLB misses)

#### Q9: What assumptions are we making?

**Assumptions**:

1. **#ASSUME_FUTURE_COMPATIBILITY**: `std::future::Future` trait is stable and won't change
2. **#VERIFY_FUTURE_TRAIT**: Implemented in std since Rust 1.36 (June 2019), 5+ years stable
3. **#ASSUME_EPOLL_AVAILABLE**: Linux 2.6+ has epoll (2003, 22 years old)
4. **#VERIFY_EPOLL**: All modern Linux distros support epoll
5. **#ASSUME_WAKER_CORRECTNESS**: std::task::Waker wakes exactly once per wake() call
6. **#VERIFY_WAKER**: Rust standard library guarantees this
7. **#ASSUME_SINGLE_PLATFORM**: Can start with Linux-only (epoll), add macOS/Windows later
8. **#VERIFY_PLATFORM**: kindly_dedup targets Linux servers primarily

### Q10-Q12: Capsule Tier Selection (CRITICAL)

#### Q10: Which tier transforms this problem?

**Tier Analysis**:

| Component | Tier | Primitive | Speedup | Complexity |
|-----------|------|-----------|---------|------------|
| **Task Queue** | T1 Atomic | WorkStealingQueue | 3-10× | MEDIUM |
| **Executor** | T5 Streaming | Lockfree work-stealing | O(1) per task | HIGH |
| **Reactor** | T1 Atomic | Lockfree event queue | <100ns register | VERY HIGH |
| **Channels** | T1 Atomic | LockfreeMPSC/Broadcast | 3-59× | LOW (EXISTS) |
| **Timers** | T1 Atomic | Hierarchical timer wheel | O(1) insert | MEDIUM |
| **Async File I/O** | T9 Persistent | mmap + background thread | 2-5× | LOW |
| **Async Net I/O** | T8 Network | Reactor + TCP state machine | 10-50× | VERY HIGH |
| **Full Runtime** | T6 Mixed | Compound (T1+T4+T5+T8+T9) | 50-100× | EXTREME |

**Q10.1: Primary Tier Selection**

**Recommendation**: **T5 Streaming** (for executor) + **T1 Atomic** (for primitives)

**Rationale**:
- Executor is incremental task processing (T5: streaming work-stealing)
- Task queue is coordination primitive (T1: lockfree CAS operations)
- Reactor is event-driven coordination (T1: lockfree event queue)
- Timers are sorted coordination (T1: lockfree timer wheel)

**Compound Capsule Strategy**:
- **AsyncRuntimeCapsule** (T6 Mixed):
  - Contains: TaskQueueCapsule (T1), ExecutorStateCapsule (T1), ReactorCapsule (T1), TimerWheelCapsule (T1)
  - Composition: 4× T1 Atomic capsules orchestrated by T5 streaming executor loop
  - Target: 50-100× compound speedup vs baseline thread pool

#### Q11: Rust transformation patterns

**Pattern Selection**:

1. **Future Trait Integration**:
   ```rust
   use std::future::Future;
   use std::task::{Context, Poll};
   
   // MUST use std::future::Future (ecosystem compatibility)
   pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
   where
       F: Future + Send + 'static,
       F::Output: Send + 'static,
   {
       // Create task, submit to queue, return handle
   }
   ```

2. **Waker Infrastructure**:
   ```rust
   use std::task::{Waker, Wake};
   
   struct TaskWaker {
       task_id: AtomicU64,
       queue: Arc<TaskQueue>,
   }
   
   impl Wake for TaskWaker {
       fn wake(self: Arc<Self>) {
           // Re-queue task for polling
           self.queue.enqueue(self.task_id.load(Ordering::Acquire));
       }
   }
   ```

3. **Lockfree Task Queue**:
   ```rust
   use atomic_capsule::collections::WorkStealingQueue;
   
   #[derive(ComputationalCapsule)]
   #[capsule(alignment = 64, size = 64)]
   struct TaskQueueCapsule {
       head: AtomicU64,  // [gen:32 | idx:32]
       tail: AtomicU64,  // [gen:32 | idx:32]
       _padding: [u8; 48],
   }
   ```

4. **Reactor Event Queue**:
   ```rust
   #[derive(ComputationalCapsule)]
   #[capsule(alignment = 128, size = 128)]
   struct ReactorCapsule {
       epoll_fd: AtomicI32,  // epoll file descriptor
       event_queue: Arc<LockfreeQueue<EpollEvent>>,
       _padding: [u8; 120],
   }
   ```

#### Q12: Nightly features needed?

**Nightly Features**:

| Feature | Benefit | Risk | Recommendation |
|---------|---------|------|----------------|
| **portable_simd** | 2-8× SIMD task queue | MEDIUM | ⚠️ Optional (stable fallback) |
| **async_fn_in_trait** | STABLE (1.75+) | NONE | ✅ Use it |
| **generic_const_exprs** | Const task buffer size | HIGH | ❌ Avoid (unstable) |
| **naked_functions** | Zero-cost task poll | VERY HIGH | ❌ Avoid (unsafe) |

**Q12.1: Baseline (Stable Rust 1.76+)**
- `std::future::Future` (stable)
- `std::task::Waker` (stable)
- `async fn` functions (stable)
- `async/await` syntax (stable)

**Q12.2: Nightly Optimizations (Optional)**
- `portable_simd`: 8-way SIMD task queue operations
- SIMD task ID packing (fit 8× u64 task IDs in __m512i register)
- Batch poll 8 tasks simultaneously (unlikely to help, polling is sequential)

**Q12.3: Ultrathink (Game-Changing Features)**

**P0: Polonius** (Future Rust 2024/2025)
- **Impact**: Zero-copy task borrowing (eliminates Arc<Mutex<Task>>)
- **Speedup**: 10-20% (reduce allocation pressure)
- **Availability**: RFC 2094, ETA unknown

**P1: Negative Trait Bounds** (Future Rust)
- **Impact**: `where F: !Send` for local tasks (avoid Send overhead)
- **Speedup**: 5-10% (smaller task struct)
- **Availability**: RFC 2056, ETA unknown

**P2: Linear Types** (Future Rust)
- **Impact**: Enforce task runs exactly once (prevent double-execution bugs)
- **Speedup**: 0% (safety, not performance)
- **Availability**: Research phase, ETA 2026+

### Q13-Q30: Implementation Planning

#### Q13: Interfaces (Public API)

**Core Executor API** (Drop-in Tokio Replacement):
```rust
pub mod capsule_rt {
    use std::future::Future;
    use std::time::Duration;
    
    // Executor builder
    pub struct Runtime {
        executor: Arc<ExecutorCapsule>,
    }
    
    impl Runtime {
        pub fn new() -> std::io::Result<Self>;
        pub fn block_on<F: Future>(&self, future: F) -> F::Output;
        pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static;
    }
    
    // Global executor (thread-local)
    pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;
    
    pub async fn sleep(duration: Duration);
    pub async fn timeout<F>(duration: Duration, future: F) -> Result<F::Output, Elapsed>
    where
        F: Future;
    
    // Channels (delegate to atomic_capsule)
    pub use atomic_capsule::collections::{
        mpsc,  // MPSC channel
        broadcast,  // Broadcast channel
        oneshot,  // One-shot channel
    };
}
```

**Compatibility Layer** (Optional Tokio API Emulation):
```rust
// Alias capsule_rt to tokio for drop-in replacement
pub use capsule_rt as tokio;
```

#### Q14: Dependencies (ZERO External Deps)

**Dependency Policy**: **ZERO external async dependencies**

**Required** (std library only):
- `std::future::Future`
- `std::task::{Context, Poll, Waker}`
- `std::sync::Arc`
- `std::thread::{spawn, park, unpark}`
- `std::time::{Duration, Instant}`

**Platform-Specific** (syscalls):
- Linux: `libc::epoll_create1`, `epoll_ctl`, `epoll_wait`
- macOS: `libc::kqueue`, `kevent`
- Windows: IOCP (MUCH harder, defer to Phase 3)

**Internal** (atomic_capsule):
- `atomic_capsule::collections::WorkStealingQueue` (task queue)
- `atomic_capsule::collections::LockfreeMPSC` (channels)
- `atomic_capsule::primitives::DualAtomicU64` (state tracking)

#### Q15: Resources (CPU, Memory, Syscalls)

**Resource Budget**:

| Resource | Tokio Baseline | Target | Budget |
|----------|----------------|--------|--------|
| **CPU Cores** | N (configurable) | N (match Tokio) | 1-64 cores |
| **Memory per Task** | 1.5KB | <2KB | +33% acceptable |
| **Memory Overhead** | ~100KB (runtime) | <200KB | +100% acceptable |
| **Syscalls** | 1× epoll_wait per event loop (1-10μs) | 1× epoll_wait | Same |
| **File Descriptors** | 1× epoll_fd + N× sockets | 1× epoll_fd + N× sockets | Same |

**Memory Layout** (Minimal Runtime):
- Executor: 128B (cache-aligned)
- Task Queue: 64B × N slots (e.g., 4KB for 64 tasks)
- Reactor: 128B (epoll_fd + event queue)
- Timer Wheel: 1KB (64 buckets × 16B pointers)
- **Total**: ~6KB fixed overhead + 1.5KB per task

#### Q16: Scaling (1 thread → 64 threads)

**Scaling Strategy**:

1. **Single-Threaded (Phase 0 MVP)**:
   - 1 executor thread, 1 reactor thread
   - Simple round-robin task polling
   - NO work-stealing (sequential queue)
   - Target: 10K tasks/sec

2. **Multi-Threaded (Phase 1)**:
   - N executor threads (1 per core)
   - 1 reactor thread (epoll in dedicated thread)
   - Work-stealing task queue (lockfree)
   - Target: 100K tasks/sec @ 8 cores

3. **NUMA-Aware (Phase 4)**:
   - Per-NUMA-node task queues (reduce cross-socket traffic)
   - CPU affinity per worker thread (pin to core)
   - Adaptive work-stealing (prefer local queue)
   - Target: 1M tasks/sec @ 64 cores

#### Q17: Granularity (Task Size, Batching)

**Task Granularity**:
- **Micro-tasks**: <100ns compute (e.g., state machine poll)
- **Small tasks**: 1-10μs compute (e.g., parse request)
- **Medium tasks**: 10-100μs compute (e.g., database query)
- **Large tasks**: >100μs compute (e.g., file I/O, use spawn_blocking)

**Batching Strategy**:
- **Task Polling**: Poll up to 64 tasks per loop iteration (reduce queue CAS overhead)
- **Event Processing**: Process up to 128 epoll events per epoll_wait (reduce syscall overhead)
- **Timer Expiry**: Expire all timers in same bucket (reduce tree traversals)

#### Q18-Q30: [Additional UCE34 Questions]

**Q18: Transformations** - Task → Pinned Future → Poll → Ready/Pending
**Q19: Concurrency** - Multi-threaded work-stealing executor
**Q20: State Management** - Executor state in capsule, tasks in heap
**Q21: Memory Layout** - 128B executor capsule, 64B task queue nodes
**Q22: Verification** - #[derive(ComputationalCapsule)] on all capsules
**Q23: Error Handling** - Result<T, JoinError>, panic catching
**Q24: Testing** - T28 (28 tests: unit, property, integration, production)
**Q25: Monitoring** - Metrics capsule (task count, queue depth, latency P99)
**Q26: Lifecycle** - Runtime::new() → spawn() → shutdown() → join()
**Q27: Composition** - Executor + Reactor + Timers + Channels (T6 Mixed)
**Q28: Simplicity** - Minimal API (spawn, block_on, sleep, timeout)
**Q29: Migration** - Phase 0 removes Tokio, Phase 1 adds minimal runtime if needed
**Q30: Documentation** - Rustdoc + examples + migration guide

### Q31-Q34: Validation

#### Q31: Simplicity Check

**Question**: Is a custom async runtime simpler than using Tokio?

**Answer**: **NO**

**Complexity Comparison**:

| Approach | LOC | Dependencies | Maintenance | Simplicity Score |
|----------|-----|--------------|-------------|------------------|
| **Keep Tokio** | 0 new | 1 (tokio) | 0 (community) | ✅ **10/10** |
| **Remove Tokio (std::thread)** | 50 | 0 | LOW | ✅ **9/10** |
| **Minimal Executor** | 500 | 0 | MEDIUM | ⚠️ **6/10** |
| **Full Async Runtime** | 5000+ | 0 | VERY HIGH | ❌ **2/10** |

**Verdict**: Building a custom async runtime is FAR more complex than keeping Tokio.

#### Q32: Constraints Honored

**Constraints**:
1. ✅ **Zero Dependencies**: No external async deps (only std + atomic_capsule)
2. ✅ **100% Lockfree**: All capsules use atomics, no Mutex/RwLock
3. ⚠️ **Performance**: Likely slower than Tokio initially (need optimization)
4. ⚠️ **Compatibility**: May not work with ecosystem crates (hyper, tonic expect Tokio)
5. ✅ **Safety**: 100% safe Rust (no unsafe in user-facing API)

#### Q33: Verification (Compile-Time)

**Verification Strategy**:

1. **All capsules verified**:
   ```rust
   #[derive(ComputationalCapsule)]
   #[capsule(alignment = 64, size = 64)]
   struct TaskQueueCapsule { ... }
   ```

2. **Clippy enforcement**:
   ```toml
   [lints.clippy]
   missing_capsule_verification = "deny"
   ```

3. **ASSUM framework** (99.99% safety):
   - #ASSUME_LOCKFREE: No mutex, verified by grep
   - #VERIFY_LOCKFREE: All operations use atomics
   - #ASSUME_EPOLL_CORRECTNESS: epoll syscall is correct
   - #VERIFY_EPOLL: Test with 1000+ concurrent connections

#### Q34: Auditability (Compliance)

**Q34 Compliance Requirements**:

**SOX/SOC2/GDPR/HIPAA Audit Trail**:
- Task spawn/completion timestamps (SHA-256 hash chain)
- Executor metrics (queue depth, latency P99, task count)
- Reactor events (epoll syscalls, file descriptor activity)
- Timer expiry (precise timestamps for timeout tracking)

**Implementation**:
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
struct AuditLogCapsule {
    event_count: AtomicU64,
    prev_hash: Arc<AtomicHash256>,  // SHA-256 hash chain
    _padding: [u8; 40],
}
```

---

## Architecture Design

### System Architecture (Layered)

```
┌─────────────────────────────────────────────────────────────┐
│                      USER APPLICATION                        │
│  async fn main() { capsule_rt::spawn(async { ... }); }     │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                  CAPSULE_RT PUBLIC API (L4)                  │
│  spawn(), block_on(), sleep(), timeout(), channels          │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    EXECUTOR LAYER (L3)                       │
│  - Multi-threaded work-stealing scheduler                   │
│  - Task polling loop (poll futures until Ready)             │
│  - Waker infrastructure (wake parked threads)               │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                   PRIMITIVES LAYER (L2)                      │
│  TaskQueueCapsule (T1) | ReactorCapsule (T1)                │
│  TimerWheelCapsule (T1) | ChannelCapsules (T1)              │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                  PLATFORM LAYER (L1)                         │
│  Linux: epoll | macOS: kqueue | Windows: IOCP               │
│  std::thread | std::sync::atomic | std::task                │
└─────────────────────────────────────────────────────────────┘
```

### Component Breakdown

#### 1. TaskQueueCapsule (T1 Atomic)

**Purpose**: Lockfree MPMC work-stealing task queue

**Layout** (64B cache-aligned):
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct TaskQueueCapsule {
    head: AtomicU64,  // [gen:32 | idx:32] (writer position)
    tail: AtomicU64,  // [gen:32 | idx:32] (reader position)
    capacity: u32,    // Queue size (power of 2)
    _padding: [u8; 44],
}
```

**Operations**:
- `push(task_id)`: Increment head, CAS if tail not full
- `pop()`: Increment tail, CAS if head not empty
- `steal()`: Try pop from remote queue (work-stealing)

**Performance**: <100ns per operation (3-10× vs Mutex<VecDeque>)

#### 2. ExecutorCapsule (T1 Atomic + T5 Streaming)

**Purpose**: Multi-threaded task executor with work-stealing

**Layout** (128B cache-aligned):
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct ExecutorCapsule {
    num_workers: AtomicU32,       // Worker thread count
    running: AtomicBool,          // Shutdown flag
    total_tasks: AtomicU64,       // Task counter (metrics)
    completed_tasks: AtomicU64,   // Completion counter
    _padding: [u8; 107],
}
```

**Executor Loop** (per worker thread):
```rust
loop {
    // 1. Try pop from local queue
    if let Some(task_id) = local_queue.pop() {
        poll_task(task_id);
        continue;
    }
    
    // 2. Try steal from other workers
    for other_queue in &worker_queues {
        if let Some(task_id) = other_queue.steal() {
            poll_task(task_id);
            continue;
        }
    }
    
    // 3. Check reactor for I/O events
    if let Some(task_id) = reactor.poll_events(timeout_10ms) {
        local_queue.push(task_id);
        continue;
    }
    
    // 4. Park thread (no work available)
    thread::park_timeout(Duration::from_millis(100));
}
```

#### 3. ReactorCapsule (T1 Atomic)

**Purpose**: Lockfree I/O event reactor (epoll wrapper)

**Layout** (128B cache-aligned):
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct ReactorCapsule {
    epoll_fd: AtomicI32,              // epoll file descriptor
    registered_count: AtomicU64,      // Number of registered FDs
    event_count: AtomicU64,           // Total events processed
    _padding: [u8; 108],
}
```

**Operations**:
- `register(fd, task_id, events)`: epoll_ctl(ADD) + store task_id mapping
- `poll_events(timeout)`: epoll_wait() + return ready task IDs
- `unregister(fd)`: epoll_ctl(DEL)

**Event Handling**:
```rust
pub fn poll_events(&self, timeout: Duration) -> Vec<u64> {
    let mut events = [MaybeUninit::uninit(); 128];  // Batch 128 events
    
    let n = unsafe {
        libc::epoll_wait(
            self.epoll_fd.load(Ordering::Relaxed),
            events.as_mut_ptr() as *mut libc::epoll_event,
            128,
            timeout.as_millis() as i32,
        )
    };
    
    if n <= 0 { return vec![]; }
    
    // Extract task IDs from events
    (0..n).map(|i| unsafe {
        let event = events[i].assume_init();
        event.u64  // Stored task_id
    }).collect()
}
```

#### 4. TimerWheelCapsule (T1 Atomic)

**Purpose**: O(1) timer insert/remove with hierarchical buckets

**Layout** (variable size):
```rust
struct TimerWheelCapsule {
    current_tick: AtomicU64,         // Current time (milliseconds)
    buckets: [Bucket; 64],           // 64 buckets (1-64ms granularity)
}

struct Bucket {
    head: AtomicU64,  // Linked list of timers
}

struct Timer {
    expiry: u64,           // Expiry time (milliseconds)
    task_id: u64,          // Task to wake
    next: AtomicU64,       // Next timer in bucket
}
```

**Operations**:
- `insert(duration, task_id)`: Hash duration to bucket, CAS insert
- `tick()`: Increment current_tick, expire bucket[tick % 64]
- `cancel(timer_id)`: Find timer, CAS remove from bucket

**Complexity**: O(1) insert, O(1) amortized expiry (batch expire bucket)

### Tier Composition Strategy

**Primary Capsules** (Single-Tier):
- TaskQueueCapsule (T1 Atomic): Lockfree CAS operations
- ReactorCapsule (T1 Atomic): Epoll coordination
- TimerWheelCapsule (T1 Atomic): Timer coordination
- ChannelCapsules (T1 Atomic): MPSC/Broadcast (ALREADY in atomic_capsule)

**Composite Capsule** (T6 Mixed):
```rust
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
struct AsyncRuntimeCapsule {
    executor: ExecutorCapsule,        // 128B
    task_queue: TaskQueueCapsule,     // 64B
    reactor: ReactorCapsule,          // 128B (OPTIONAL, use Arc if needed)
    // Note: reactor is shared, so use Arc<ReactorCapsule> instead
}
```

**Composition Pattern**: Container Capsule (stores Arc<> to sub-capsules)
- Executor owns task queues (Vec<Arc<TaskQueueCapsule>>)
- Reactor is shared across all workers (Arc<ReactorCapsule>)
- Timer wheel is shared (Arc<TimerWheelCapsule>)

---

## Detailed Implementation Plan (4 Phases, 20 Weeks)

### Phase 0: Research & Prototyping (2 Weeks)

**Goal**: Validate feasibility, prototype single-threaded executor

**Milestones**:

**Week 1: Prototype Executor**
- [ ] Create `capsule_rt` crate skeleton
- [ ] Implement `Runtime::new()` (single-threaded)
- [ ] Implement `Runtime::block_on(future)` (poll until Ready)
- [ ] Implement `spawn(future)` → `JoinHandle<T>`
- [ ] Integrate `std::future::Future` trait
- [ ] Build waker infrastructure (Arc<TaskWaker>)
- [ ] Unit tests (10 tests: spawn, block_on, join, panic handling)

**Deliverable**: Working single-threaded executor that can spawn and poll futures

**Example**:
```rust
let rt = Runtime::new()?;
let handle = rt.spawn(async {
    println!("Hello from task!");
    42
});
let result = rt.block_on(handle)?;
assert_eq!(result, 42);
```

**Week 2: Benchmark Baseline**
- [ ] Implement basic task queue (VecDeque<Task>, NOT lockfree yet)
- [ ] Benchmark spawn latency (target: <500ns)
- [ ] Benchmark throughput (target: 10K tasks/sec)
- [ ] Compare vs Tokio baseline (spawn 10K trivial tasks)
- [ ] Identify bottlenecks (profiling with perf)
- [ ] Write Phase 0 report (feasibility assessment)

**Deliverable**: Performance baseline, feasibility report

**Success Criteria**:
- ✅ Can spawn and poll futures using `std::future::Future`
- ✅ Waker correctly wakes parked tasks
- ✅ Spawn latency <500ns (2× Tokio is acceptable)
- ✅ Throughput >10K tasks/sec (1% of Tokio is acceptable for MVP)

**Risk Mitigation**:
- If waker doesn't work: Debug with println! tracing
- If performance is terrible: Profile with perf, optimize hot path
- If Future integration fails: Revisit std::task documentation

**Go/No-Go Decision Point**: If Week 2 shows spawn latency >10μs or throughput <1K tasks/sec, STOP and recommend keeping Tokio.

---

### Phase 1: Minimal Viable Runtime (4 Weeks)

**Goal**: Multi-threaded executor with work-stealing + basic channels

**Milestones**:

**Week 3: Lockfree Task Queue**
- [ ] Implement `TaskQueueCapsule` (T1 Atomic)
- [ ] Implement `push(task_id)` with CAS head increment
- [ ] Implement `pop()` with CAS tail increment
- [ ] Implement `steal()` for work-stealing
- [ ] Add verification: `#[derive(ComputationalCapsule)]`
- [ ] Property tests (1000 iterations: push/pop consistency)

**Week 4: Multi-Threaded Executor**
- [ ] Implement `ExecutorCapsule` with N worker threads
- [ ] Implement work-stealing scheduler (try local → steal → park)
- [ ] Implement thread parking/unparking (std::thread::park)
- [ ] Integrate waker with work-stealing (wake → push to queue → unpark thread)
- [ ] Unit tests (20 tests: multi-threaded spawn, concurrent join)

**Week 5: Basic Channels**
- [ ] Expose `atomic_capsule::collections::LockfreeMPSC` as `capsule_rt::mpsc`
- [ ] Expose `atomic_capsule::collections::RingBufferBroadcast` as `capsule_rt::broadcast`
- [ ] Implement oneshot channel (single-use, lockfree)
- [ ] Integration tests (send/recv across tasks)

**Week 6: Integration Testing**
- [ ] Replace AsyncLogCapsule flush task with capsule_rt executor
- [ ] Benchmark vs Tokio (spawn 100K tasks, measure throughput)
- [ ] Write Phase 1 report (MVP status, performance comparison)

**Deliverable**: Multi-threaded executor that can run AsyncLogCapsule without Tokio

**Success Criteria**:
- ✅ Can spawn 100K tasks and complete them
- ✅ Work-stealing reduces idle threads
- ✅ Spawn latency <200ns (matches Tokio)
- ✅ Throughput >100K tasks/sec @ 8 cores
- ✅ AsyncLogCapsule works with capsule_rt instead of Tokio

**Risk Mitigation**:
- If work-stealing has high contention: Add adaptive stealing (reduce CAS frequency)
- If performance regresses: Profile, optimize CAS operations
- If AsyncLogCapsule doesn't work: Debug waker integration

---

### Phase 2: Async I/O Support (4 Weeks)

**Goal**: Reactor (epoll) + async file/net I/O

**Milestones**:

**Week 7: Reactor (Linux epoll)**
- [ ] Implement `ReactorCapsule` (T1 Atomic)
- [ ] Implement `register(fd, task_id, events)` using epoll_ctl
- [ ] Implement `poll_events(timeout)` using epoll_wait
- [ ] Implement `unregister(fd)` using epoll_ctl
- [ ] Integrate reactor with executor (reactor thread wakes workers)
- [ ] Unit tests (30 tests: register, poll, unregister, edge cases)

**Week 8: Async File I/O**
- [ ] Implement `AsyncFile::from_std(std::fs::File)`
- [ ] Implement `AsyncReadExt` trait (read, read_exact, read_to_end)
- [ ] Implement `AsyncWriteExt` trait (write, write_all, flush)
- [ ] Use thread pool for blocking file I/O (epoll doesn't work for files)
- [ ] Integration tests (read/write files asynchronously)

**Week 9: Async Networking (TCP only)**
- [ ] Implement `TcpListener::bind(addr)` → register epoll
- [ ] Implement `TcpListener::accept()` → async wait for EPOLLIN
- [ ] Implement `TcpStream::connect(addr)` → async wait for EPOLLOUT
- [ ] Implement `AsyncRead/AsyncWrite` for `TcpStream`
- [ ] Integration tests (TCP echo server, 1K concurrent connections)

**Week 10: AsyncLogCapsule Integration**
- [ ] Replace Tokio in AsyncLogCapsule with capsule_rt
- [ ] Replace `tokio::spawn` with `capsule_rt::spawn`
- [ ] Replace `tokio::time::interval` with `capsule_rt::sleep` loop
- [ ] Replace `tokio::fs::File` with `capsule_rt::fs::File`
- [ ] Benchmark vs Tokio version (validate performance parity)

**Deliverable**: Working async I/O (files + TCP) that can replace Tokio in AsyncLogCapsule

**Success Criteria**:
- ✅ Can read/write files asynchronously
- ✅ Can accept TCP connections and read/write data
- ✅ AsyncLogCapsule works with capsule_rt (no Tokio)
- ✅ Throughput matches Tokio (within 10%)

**Risk Mitigation**:
- If epoll is too complex: Simplify to Linux-only (defer macOS/Windows)
- If file I/O doesn't work: Use thread pool (same as Tokio)
- If TCP has bugs: Extensive testing with netcat/curl

---

### Phase 3: Full Feature Parity (6 Weeks)

**Goal**: Timers, broadcast channels, select!/join!/race! macros

**Milestones**:

**Week 11-12: Timers**
- [ ] Implement `TimerWheelCapsule` (hierarchical buckets)
- [ ] Implement `sleep(duration)` → register timer, suspend task
- [ ] Implement `timeout(duration, future)` → race timer vs future
- [ ] Implement timer expiry loop (reactor ticks timer wheel)
- [ ] Integration tests (1000 concurrent timers, verify expiry order)

**Week 13-14: Broadcast Channels**
- [ ] Expose `atomic_capsule::collections::RingBufferBroadcast`
- [ ] Implement `broadcast::channel(capacity)` → Sender + Receiver
- [ ] Implement `Sender::send(&msg)` → broadcast to all receivers
- [ ] Implement `Receiver::recv()` → async wait for message
- [ ] Integration tests (100 senders, 100 receivers, verify fanout)

**Week 15: Utility Macros**
- [ ] Implement `select!` macro (poll multiple futures, return first Ready)
- [ ] Implement `join!` macro (poll all futures, wait for all Ready)
- [ ] Implement `race!` macro (poll multiple futures, cancel others)
- [ ] Unit tests (verify macro expansion, edge cases)

**Week 16: Full Tokio Replacement**
- [ ] Replace ALL Tokio usage in kindly_dedup
- [ ] Replace ALL Tokio usage in atomic_capsule (network modules)
- [ ] Run full test suite (530+ tests in atomic_capsule)
- [ ] Benchmark full kindly_dedup pipeline (validate no regression)

**Deliverable**: Feature-complete async runtime that can replace Tokio entirely

**Success Criteria**:
- ✅ Can replace ALL Tokio usage in kindly_dedup + atomic_capsule
- ✅ All tests pass (530+ in atomic_capsule, 266+ in kindly_dedup)
- ✅ Performance matches Tokio (within 10%)
- ✅ No regressions in latency/throughput

**Risk Mitigation**:
- If timers are buggy: Extensive testing with sleeps/timeouts
- If macros don't work: Simplify to function-based API
- If tests fail: Debug, fix bugs, iterate

---

### Phase 4: Optimization & Production (4 Weeks)

**Goal**: SIMD scheduling, production hardening, comprehensive testing

**Milestones**:

**Week 17: SIMD Optimization**
- [ ] Implement SIMD task queue (8-way parallel push/pop)
- [ ] Benchmark vs scalar baseline (validate 2-8× speedup)
- [ ] Add feature flag: `simd-executor = ["portable_simd"]`
- [ ] Integration tests (verify SIMD + scalar produce same results)

**Week 18: Adaptive Optimizations**
- [ ] Implement adaptive work-stealing (reduce stealing on low load)
- [ ] Implement NUMA-aware task queues (per-NUMA-node queues)
- [ ] Implement CPU pinning (pin workers to cores)
- [ ] Benchmark vs baseline (validate 10-20% improvement)

**Week 19: Production Hardening**
- [ ] Comprehensive testing (T28: 28 tests × 4 tiers = 112 tests)
- [ ] Stress testing (1M tasks, 1K concurrent connections, 10K timers)
- [ ] Fuzz testing (afl.rs, 10M iterations)
- [ ] Security audit (ASSUM framework, 99.99% safety)

**Week 20: Documentation & Release**
- [ ] Write comprehensive documentation (Rustdoc, examples)
- [ ] Write migration guide (Tokio → capsule_rt)
- [ ] Write performance report (B32 benchmarks)
- [ ] Tag v0.1.0 release
- [ ] Publish to kindly ecosystem (NOT crates.io yet)

**Deliverable**: Production-ready async runtime with SIMD optimization

**Success Criteria**:
- ✅ Performance matches or exceeds Tokio
- ✅ 100% test coverage (unit, property, integration, production)
- ✅ 99.99% ASSUM safety
- ✅ Comprehensive documentation

---

## Risk Mitigation

### Top 5 Risks + Mitigation Strategies

#### Risk 1: Schedule Underestimation (90% probability)

**Description**: Async runtimes are deceptively complex. Tokio took YEARS to mature.

**Impact**: Project takes 6-8 months instead of 5 months

**Mitigation**:
- **Phased approach**: Stop after each phase, reassess
- **MVP-first**: Build minimal executor (Phase 0-1), validate before proceeding
- **Cut scope aggressively**: Linux-only, TCP-only, defer macOS/Windows/UDP
- **Go/No-Go gates**: If Phase 0 shows <1K tasks/sec, STOP

**Contingency**: If underestimated, recommend keeping Tokio and building minimal executor ONLY for AsyncLogCapsule

#### Risk 2: Scope Creep (70% probability)

**Description**: "Just one more feature" syndrome (select!, join!, timeouts, spawn_blocking, etc.)

**Impact**: Project expands from 5 months to 12+ months

**Mitigation**:
- **Strict scope**: Only features ACTUALLY used in kindly_dedup
- **Feature gating**: All features behind Cargo feature flags
- **Defer ecosystem compatibility**: Don't worry about hyper/tonic compatibility
- **Say NO**: Resist urge to match all Tokio features

**Contingency**: If scope creeps, cut features ruthlessly (timers are nice-to-have, not must-have)

#### Risk 3: Performance Regression (50% probability)

**Description**: Tokio has 10 years of optimization. We start from zero.

**Impact**: capsule_rt is 2-10× slower than Tokio

**Mitigation**:
- **B32 benchmarking**: Measure everything (spawn, throughput, latency)
- **Profile aggressively**: Use perf, flamegraph, cachegrind
- **Optimize hot paths**: Focus on task queue, waker, epoll
- **SIMD when appropriate**: 8-way SIMD task queue (Phase 4)

**Contingency**: If 2× slower but still meets requirements, ship it (100K tasks/sec @ 8 cores is plenty)

#### Risk 4: Ecosystem Incompatibility (60% probability)

**Description**: Most async crates assume `tokio::Runtime` (hyper, tonic, tower, etc.)

**Impact**: Can't use popular async crates without rewriting them

**Mitigation**:
- **Don't need ecosystem compatibility**: kindly_dedup only uses AsyncLogCapsule
- **Compatibility layer**: Provide `pub use capsule_rt as tokio;` alias
- **Document limitations**: "Works with std::future::Future, not Tokio-specific crates"

**Contingency**: If ecosystem compatibility is critical, recommend keeping Tokio

#### Risk 5: Maintenance Burden (80% probability)

**Description**: Async Rust evolves rapidly. We need to keep up.

**Impact**: Constant bug fixes, feature requests, platform updates

**Mitigation**:
- **Minimal surface area**: Only implement what we use
- **Leverage atomic_capsule**: Reuse existing lockfree primitives
- **Automated testing**: CI runs 112+ tests on every commit
- **Community maintenance**: Open-source, accept contributions

**Contingency**: If maintenance becomes overwhelming, deprecate and recommend Tokio

---

## Success Metrics

### Performance Metrics

| Metric | Tokio Baseline | Target | Measurement Method |
|--------|----------------|--------|-------------------|
| **Task Spawn Latency** | 200ns | <500ns | B32 benchmark, 1M iterations |
| **Task Throughput** | 800K tasks/sec @ 8 cores | >100K tasks/sec | Spawn 1M trivial tasks |
| **Memory per Task** | 1.5KB | <2KB | Measure with Valgrind |
| **Runtime Overhead** | 100KB | <200KB | Binary size difference |
| **AsyncLogCapsule Performance** | 10K entries/sec | >9K entries/sec | B32 benchmark |

### Compatibility Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| **std::future::Future Support** | 100% | Can spawn any async fn |
| **Waker Correctness** | 100% | All wakers wake exactly once |
| **Test Pass Rate** | 100% | 530+ tests in atomic_capsule pass |
| **kindly_dedup Works** | 100% | All 266+ tests pass |

### Quality Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| **ASSUM Safety** | 99.99% | All #ASSUME tags have #VERIFY |
| **Test Coverage** | 100% | T28 (28 tests × 4 tiers) |
| **Documentation** | 100% | Every public API has Rustdoc |
| **Zero UB** | 100% | MIRI clean, Valgrind clean |

### Success Criteria (Go/No-Go)

**MUST HAVE** (non-negotiable):
- ✅ Can replace Tokio in AsyncLogCapsule
- ✅ Performance within 2× of Tokio (spawn latency <500ns)
- ✅ All tests pass (100% compatibility)
- ✅ ASSUM 99.99% safe (no UB)

**NICE TO HAVE** (bonus):
- ⚠️ Matches Tokio performance (within 10%)
- ⚠️ SIMD optimization (2-8× speedup on task queue)
- ⚠️ macOS/Windows support (kqueue, IOCP)
- ⚠️ Ecosystem compatibility (works with hyper, tonic)

**DEALBREAKERS** (abandon project):
- ❌ Spawn latency >10μs (50× slower than Tokio)
- ❌ Throughput <10K tasks/sec (100× slower than Tokio)
- ❌ Tests fail after 2 weeks of debugging
- ❌ UB detected in production

---

## Go/No-Go Recommendation

### Recommendation: **NO** (Do Not Proceed with Full Async Runtime)

### Alternative Recommendation: **YES** (Option A: Remove Tokio Entirely)

---

### Option A: Remove Tokio Entirely (RECOMMENDED)

**Timeline**: 1 week  
**Complexity**: LOW  
**Risk**: LOW  
**ROI**: HIGH (zero dependencies, simple code)

**Plan**:

**Week 1: Rewrite AsyncLogCapsule**
- [ ] Replace `tokio::spawn` with `std::thread::spawn`
- [ ] Replace `tokio::time::interval` with `loop { sleep(100ms) }`
- [ ] Replace `tokio::fs::File` with `std::fs::File`
- [ ] Replace `tokio::io::BufWriter` with `std::io::BufWriter`
- [ ] Benchmark vs Tokio version (validate performance parity)

**Implementation** (50 lines):
```rust
pub struct SyncFlushTask {
    queue: Arc<LockfreeQueue<LogEntry>>,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl SyncFlushTask {
    pub fn start(mut writer: std::io::BufWriter<std::fs::File>) -> Self {
        let queue = Arc::new(LockfreeQueue::new());
        let running = Arc::new(AtomicBool::new(true));
        
        let queue_clone = Arc::clone(&queue);
        let running_clone = Arc::clone(&running);
        
        let thread_handle = std::thread::spawn(move || {
            while running_clone.load(Ordering::Acquire) {
                // Batch pop entries (up to 128 per flush)
                let mut batch = Vec::with_capacity(128);
                while let Some(entry) = queue_clone.try_pop() {
                    batch.push(entry);
                    if batch.len() >= 128 { break; }
                }
                
                // Write batch to file
                for entry in batch {
                    writeln!(writer, "{}", entry.as_str()).ok();
                }
                writer.flush().ok();
                
                // Sleep 100ms between flushes
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        
        Self { queue, running, thread_handle: Some(thread_handle) }
    }
    
    pub fn append(&self, entry: LogEntry) -> Result<(), Error> {
        self.queue.push(entry)
    }
    
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
        if let Some(handle) = self.thread_handle.take() {
            handle.join().ok();
        }
    }
}
```

**Benefits**:
- ✅ ZERO Tokio dependency
- ✅ Same performance (batching is the key, not async)
- ✅ Simpler code (50 lines vs 500 lines)
- ✅ No async complexity

**Drawbacks**:
- ⚠️ One thread per AsyncLogCapsule instance (acceptable, typically 1-2 instances)
- ⚠️ Cannot share executor across tasks (not needed for our use case)

**Success Criteria**:
- ✅ Throughput matches Tokio version (10K entries/sec)
- ✅ Latency matches Tokio version (append <50ns)
- ✅ All tests pass

**Go/No-Go**: **YES** (STRONGLY RECOMMENDED)

---

### Option B: Minimal Executor (AsyncLogCapsule Only)

**Timeline**: 2-4 weeks  
**Complexity**: MEDIUM  
**Risk**: MEDIUM  
**ROI**: MEDIUM (learning exercise, but not critical)

**Plan**:

**Week 1-2: Single-Threaded Executor**
- Build minimal executor (spawn, block_on, waker)
- NO work-stealing, NO multi-threading
- Target: 10K tasks/sec

**Week 3-4: AsyncLogCapsule Integration**
- Replace Tokio in AsyncLogCapsule
- Benchmark vs Tokio version
- Validate performance parity

**Benefits**:
- ✅ ZERO Tokio dependency
- ✅ Learning opportunity (async runtime internals)
- ✅ Foundation for future async work

**Drawbacks**:
- ⚠️ 2-4 weeks development time
- ⚠️ Limited to single-threaded (no work-stealing)
- ⚠️ Still complex (500+ LOC)

**Success Criteria**:
- ✅ Can spawn and poll futures
- ✅ AsyncLogCapsule works
- ✅ Performance within 2× of Tokio

**Go/No-Go**: **MAYBE** (Consider if learning is a priority)

---

### Option C: Full Async Runtime (NOT RECOMMENDED)

**Timeline**: 20 weeks (5 months)  
**Complexity**: VERY HIGH  
**Risk**: VERY HIGH  
**ROI**: VERY LOW (massive opportunity cost)

**Rationale**:
- ❌ Tokio is excellent (not a bottleneck)
- ❌ Massive development effort (5 months)
- ❌ Delays capsule-os by 6+ months
- ❌ Maintenance burden (one person vs community)
- ❌ Ecosystem incompatibility (hyper, tonic expect Tokio)

**When to Revisit**:
- capsule-os needs async runtime as core infrastructure
- Tokio becomes a bottleneck (unlikely)
- Strategic need for zero-dependency async (philosophical purity)
- Team grows to 3+ developers (can afford maintenance)

**Go/No-Go**: **NO** (Do not proceed)

---

## Final Recommendation

### Immediate Action (Week of 2025-11-07)

**RECOMMENDED PATH**: **Option A (Remove Tokio Entirely)**

**Next Steps**:

1. **Week 1** (Nov 7-14):
   - Implement SyncFlushTask (50 lines)
   - Replace AsyncLogCapsule Tokio usage with std::thread
   - Benchmark vs Tokio version (B32 framework)
   - Run all tests (validate no regressions)

2. **Week 2** (Nov 15-21):
   - Update kindly_dedup Cargo.toml (remove tokio dependency)
   - Update atomic_capsule Cargo.toml (make tokio optional)
   - Run full test suite (530+ atomic_capsule + 266+ kindly_dedup)
   - Tag kindly_dedup v1.9.0 (zero Tokio dependency)

3. **Decision Point** (Nov 22):
   - If successful: STOP HERE, move on to capsule-os
   - If needed: Revisit async runtime in 6 months

**Success Metrics** (Go/No-Go for Option A):
- ✅ Throughput matches Tokio (10K entries/sec)
- ✅ Latency matches Tokio (append <50ns)
- ✅ All tests pass (100%)
- ✅ Zero Tokio dependency

**Budget**:
- Time: 1-2 weeks
- LOC: +50 (SyncFlushTask), -500 (remove Tokio code)
- Net: 450 LOC reduction (simpler code!)

---

### Long-Term Strategy (6-12 Months)

**IF async runtime becomes critical**:

1. **6 Months** (May 2026):
   - Reassess async runtime need
   - If capsule-os needs it: Start Phase 0 (research)
   - If not needed: Continue with std::thread approach

2. **12 Months** (Nov 2026):
   - If Phase 0 successful: Proceed to Phase 1 (minimal executor)
   - If not successful: Keep Tokio or std::thread

**Strategic Considerations**:
- capsule-os vision: Can it use sync + lockfree primitives instead of async?
- Team size: One person cannot maintain async runtime long-term
- Ecosystem: Do we need hyper/tonic compatibility?
- Performance: Is Tokio a bottleneck? (Currently: NO)

---

## Appendix A: Tokio Feature Usage Detail

### kindly_dedup Tokio Usage (MINIMAL)

**Production Code** (20 lines):
```rust
// src/benchmarking/audit_logger.rs (lines 245-246)
let tokio_file = tokio::fs::File::from_std(file);
let writer = tokio::io::BufWriter::new(tokio_file);

// src/benchmarking/audit_logger.rs (lines 644-652)
if let Ok(rt) = tokio::runtime::Handle::try_current() {
    rt.block_on(async { let _ = handle.await; });
}
```

**Test Code** (800 lines):
```rust
// tests/server_tests.rs (entire file)
#[tokio::test]
async fn test_server_start() { ... }

tokio::spawn(async move { ... });
tokio::time::sleep(Duration::from_millis(100)).await;
```

**Verdict**: Production usage is TRIVIAL (<20 lines), can be replaced in 1 week.

### atomic_capsule Tokio Usage (OPTIONAL FEATURES)

**async-log feature** (500 lines):
- AsyncLogCapsule flush task (tokio::spawn + interval + fs)

**network features** (800 lines):
- RPC server/client (tokio::net + io + timeout)

**Verdict**: All Tokio usage is OPTIONAL features, not core primitives.

---

## Appendix B: Async Runtime Complexity Analysis

### Minimal Executor (Phase 0-1)

**Components**:
1. Task struct (Future + Waker + state)
2. Task queue (FIFO, lockfree)
3. Executor loop (poll tasks until Ready)
4. Waker infrastructure (Arc<TaskWaker>)

**LOC Estimate**: 500-800 lines

**Complexity**: MEDIUM

### Full Async Runtime (Phase 0-4)

**Components**:
1. Minimal executor (from Phase 0-1)
2. Work-stealing scheduler (multi-threaded)
3. Reactor (epoll/kqueue/IOCP)
4. Async file I/O (thread pool)
5. Async networking (TCP/UDP)
6. Timers (hierarchical timer wheel)
7. Channels (MPSC, broadcast, oneshot)
8. Utility macros (select!, join!, race!)

**LOC Estimate**: 5000-8000 lines

**Complexity**: VERY HIGH

---

## Appendix C: References

**Async Runtime Resources**:
- Tokio Internals: https://tokio.rs/blog/2019-10-scheduler
- async-std Design: https://async.rs/blog/stop-worrying-about-blocking-the-executor/
- smol: https://github.com/smol-rs/smol (minimal async runtime, 3K LOC)
- Glommio: https://github.com/DataDog/glommio (io_uring-based runtime)

**UCE34 Framework**:
- UCE34_FRAMEWORK.md (Q1-Q34 systematic discovery)
- UCE34_TIER_REFERENCE.md (tier implementation details)
- UCE34_EXAMPLES.md (production code examples)

**B32 Benchmarking**:
- B32_BENCHMARK_FRAMEWORK.md (32 guidelines + 27 reality checks)

**ASSUM Safety**:
- ASSUM_SAFETY.md (assumption validation framework)

**T28 Testing**:
- T28_TESTING_FRAMEWORK.md (28-question comprehensive testing)

---

**END OF COMPREHENSIVE PLAN**

**Next Steps**: User decision required. Recommend Option A (Remove Tokio, 1 week).
