# Computational Capsule Taxonomy
**Complete classification system for cache-aware computation primitives**

**Source**: `/home/samuel/Docs/The Computational Capsule.md`
**Purpose**: Systematic capsule selection guide for performance optimization

---

## Classification Framework

### Tier 1: Atomic Capsules (Coordination)
**Latency**: <100ns
**Size**: 64-1024 bits
**Use Case**: Lockfree coordination, emergency protection
**Alignment**: 64B or 128B (cache line)
**Memory Ordering**: Relaxed/Acquire/Release based on coordination needs

#### Proven Patterns (Production-Validated)

| Pattern | Size | Alignment | Purpose | Latency (ns) | Speedup |
|---------|------|-----------|---------|--------------|---------|
| **ACB-64** | 64-bit | 128B | Circuit breaker | 9.8 | 3.3x vs mutex |
| **APC-512** | 512-bit | 128B | Position tracking | 22.3 | 3.1x vs DashMap |
| **RLT-1024** | 1024-bit | 128B | Risk limits | 28.7 | 2-3.5x vs multi-field |
| **AEB-512** | 512-bit | 128B | Order execution | 47.2 | State machine safety |
| **PNL-512** | 512-bit | 128B | P&L tracking | 83.4 | 2.4x vs FP+mutex |

**When to Use**:
- Sub-microsecond decision latency required
- Lockfree coordination between threads
- Emergency protection systems (circuit breakers)
- Real-time risk management
- High-frequency order execution

**When NOT to Use**:
- Latency tolerance >1ms
- Single-threaded applications
- Infrequent updates (< 1Hz)
- Variable-size data structures

---

### Tier 2: SIMD Capsules (Vectorized Computation)
**Latency**: <500ns
**Size**: 128-512 bits (SIMD register-aligned)
**Use Case**: Parallel computation, batch processing
**Alignment**: 16B/32B/64B (SIMD register width)
**Vectorization**: f32x4, f32x8, f64x4, i32x4, i64x4

#### Proven Patterns (Production-Validated)

| Pattern | SIMD Width | Alignment | Purpose | Latency | Speedup | Status |
|---------|------------|-----------|---------|---------|---------|--------|
| **SVC-256** | f64x4 | 32B | Venue scoring (8 venues) | ~100ns | 4x parallel | Validated |
| **HLC-256** | f64x4 | 32B | Hebbian learning | 2.5ns/conn | 19x (exceptional) | Validated |
| **SPP-512** | f32x8 | 64B | Position projection | ~200ns | 8x parallel | Validated |
| **SFE-256** | f32x8 | 32B | Feature extraction | ~150ns | 6-8x | Validated |
| **SMV-128** | i32x4 | 16B | Market data validation | ~50ns | 4x parallel | Validated |
| **STB-256** | f32x8 | 32B | Rate limiting (8 buckets) | ~85ns | 4x (production) | **Implemented ✅** |

**Phase 2 Implementation** (kindly_mcp):
- ✅ **SimdTokenBucketBatch** (STB-256): f32x8 rate limiting, 8 buckets in parallel
- ✅ File: `crates/kindly_mcp/src/rate_limiting/simd.rs`
- ✅ Verification: `verify_alignment!(SimdTokenBucketBatch, 32)`
- ✅ Testing: 9 unit tests + 429-line T28 Tier 2 property tests
- ✅ Feature-gated: `portable_simd` with scalar fallback

**When to Use**:
- Vectorizable operations (same operation on multiple data)
- Batch processing (4-16 elements)
- Mathematical operations (distance, dot product, normalization)
- Feature engineering pipelines
- Neural network forward/backward passes

**When NOT to Use**:
- Irregular control flow (lots of branches)
- Data dependencies between elements
- Variable-length processing
- Memory-bound operations (not compute-bound)

**SIMD Cheat Sheet**:
```rust
// f32x4: 4 floats in parallel (128-bit SIMD)
let a = f32x4::from_array([1.0, 2.0, 3.0, 4.0]);
let b = f32x4::splat(2.0);
let result = a * b;  // [2.0, 4.0, 6.0, 8.0]

// f64x4: 4 doubles in parallel (256-bit SIMD, AVX2)
let scores = f64x4::from_array(venue_scores);
let normalized = scores / scores.reduce_sum();

// f32x8: 8 floats in parallel (256-bit SIMD, AVX2)
let positions = f32x8::from_array(pos_array);
let adjusted = positions * f32x8::splat(multiplier);
```

---

### Tier 3: Fixed-Point Capsules (Deterministic Precision)
**Latency**: <100ns
**Size**: 64-256 bits
**Use Case**: Exact decimal arithmetic, financial calculations
**Precision**: Q8.8 (1/256 bp), Q16.16 (1/65536), Q24.8 (1/256 with large range)
**Determinism**: Zero floating-point rounding errors

#### Proven Patterns (Production-Validated)

| Pattern | Format | Precision | Purpose | Latency (ns) | Advantage | Status |
|---------|--------|-----------|---------|--------------|-----------|--------|
| **FP-Q88-64** | Q8.8 | 0.004 bp | Basis points | ~10 | Deterministic | Validated |
| **FP-Q1616-64** | Q16.16 | 0.000015 | High precision | ~15 | Wide range | Validated |
| **FP-Q248-64** | Q24.8 | 0.004 bp | Large values | ~12 | Tick precision | Validated |
| **PNL-512** | Q8.8 multi | 0.004 bp/field | P&L tracking | 83.4 | 8 symbols | Validated |
| **PricingCapsule** | Q16.16 | $0.000015 | Stripe pricing | <20 | Deterministic | **Implemented ✅** |

**Phase 2 Implementation** (kindly_mcp):
- ✅ **PricingCapsule** (Q16.16): Deterministic Stripe tier pricing
- ✅ File: `crates/kindly_mcp/src/pricing/fixed_point.rs`
- ✅ Verification: `verify_capsule!(PricingCapsule, 64, 64)`
- ✅ Tiers: Free ($0.00), Pro ($4.99), Enterprise ($99.99)
- ✅ Overflow: Protected via `checked_mul`
- ⏳ Testing: Unit tests pending, property tests needed

**Fixed-Point Formats**:

| Format | Integer Bits | Fractional Bits | Scale | Range | Precision |
|--------|--------------|-----------------|-------|-------|-----------|
| Q8.8   | 8            | 8               | 256   | ±128  | 0.004 bp  |
| Q16.16 | 16           | 16              | 65536 | ±32768| 0.000015  |
| Q24.8  | 24           | 8               | 256   | ±8.4M | 0.004 bp  |
| Q4.12  | 4            | 12              | 4096  | ±8    | 0.00024   |

**When to Use**:
- Financial calculations (P&L, pricing, fees)
- Exact decimal arithmetic required
- Reproducible results across platforms
- Regulatory compliance (deterministic audit trails)
- Tick-based calculations (futures, options)

**When NOT to Use**:
- Scientific computing (wide dynamic range)
- Machine learning (approximations acceptable)
- Very large or very small numbers (outside fixed-point range)
- Transcendental functions (sin, cos, log, exp)

**Fixed-Point Operations**:
```rust
// Q8.8 format (scale = 256)
const SCALE: i64 = 256;

// Convert float to fixed-point
let price_fixed = (price * SCALE as f64) as i64;

// Fixed-point multiplication (rescale after multiply)
let product = (a * b) / SCALE;

// Fixed-point division (prescale before divide)
let quotient = (a * SCALE) / b;

// VWAP calculation (deterministic)
let vwap = (old_qty * old_price + new_qty * new_price) / (old_qty + new_qty);

// Convert back to float
let price_float = price_fixed as f64 / SCALE as f64;
```

---

### Tier 4: Batch Capsules (Throughput Processing)
**Latency**: 1-100μs
**Size**: 1KB-64KB
**Use Case**: High-throughput batch processing
**Alignment**: 64B (cache line)
**Strategy**: Amortize overhead across many elements

#### Production Patterns (Phase 3) ✅

| Pattern | Size | Elements | Purpose | Throughput | Status |
|---------|------|----------|---------|------------|--------|
| **EndpointBatchCapsule (EBC-1K)** | 1KB | 16 endpoints | Endpoint discovery | 10-20× sequential | Implemented ✅ |

**Phase 3 Implementation** (kindly_mcp):
- ✅ **EndpointBatchCapsule** (1KB): 16-item batch endpoint discovery
- ✅ File: `crates/kindly_mcp/src/batch/endpoint_batch.rs`
- ✅ Verification: `verify_capsule!(EndpointBatchCapsule, 64, 1024)`
- ✅ Testing: 10 unit tests
- ✅ L1 Cache Fit: 16 × 64B = 1KB (optimal)

#### Proposed Patterns (Future)

| Pattern | Size | Elements | Purpose | Throughput | Use Case |
|---------|------|----------|---------|------------|----------|
| **BFE-4K** | 4KB | 512 features | Feature extraction | 10K/sec | ML pipelines |
| **BTI-16K** | 16KB | 2K ticks | Trade ingestion | 100K/sec | Market data |
| **BPC-8K** | 8KB | 1K positions | Position calculation | 50K/sec | Risk aggregation |
| **BRA-32K** | 32KB | 4K records | Risk aggregation | 20K/sec | Portfolio analysis |
| **BMV-64K** | 64KB | 8K values | Market validation | 500K/sec | Data quality |

**When to Use**:
- High-throughput processing (>1K ops/sec)
- Amortizable overhead (setup cost spreads across batch)
- Independent operations (no data dependencies)
- Memory bandwidth optimization
- Prefetch-friendly access patterns

**When NOT to Use**:
- Real-time latency requirements (<1ms)
- Small batch sizes (<100 elements)
- Sequential dependencies between elements
- Variable processing time per element

---

### Tier 5: Streaming Capsules (Continuous Computation)
**Latency**: Configurable (window-dependent)
**Size**: Variable (windowed)
**Use Case**: Continuous windowed computation
**Window Types**: Time-based, count-based, session-based
**State Management**: Ring buffers, sliding windows

#### Production Patterns (Phase 3) ✅

| Pattern | Window Type | Purpose | Latency | Status |
|---------|-------------|---------|---------|--------|
| **StreamingMetricsCapsule (SMC-256)** | Time-based (60s) | API usage metrics | <10ns/record | Implemented ✅ |

**Phase 3 Implementation** (kindly_mcp):
- ✅ **StreamingMetricsCapsule** (256B): 60-second moving window for API metrics
- ✅ File: `crates/kindly_mcp/src/streaming/api_metrics.rs`
- ✅ Verification: `verify_capsule!(StreamingMetricsCapsule, 128, 256)`
- ✅ Testing: 4 unit tests
- ✅ Atomic Ring Buffer: 60 × AtomicU32 (lockfree)

#### Proposed Patterns (Future)

| Pattern | Window Type | Purpose | Latency | Use Case |
|---------|-------------|---------|---------|----------|
| **SMA-W** | Time-based | Moving average | 1ms/update | Trend indicators |
| **SCV-W** | Count-based | Continuous validation | 500μs/check | Data quality |
| **SVR-W** | Session-based | Volatility regime | 10ms/update | Risk adaptation |
| **SFD-W** | Time-based | Fractal detection | 5ms/update | Pattern recognition |
| **SAD-W** | Count-based | Anomaly detection | 2ms/check | Outlier detection |

**When to Use**:
- Continuous data streams
- Windowed aggregations (moving averages, volatility)
- Event-driven processing
- Temporal pattern detection
- Real-time monitoring

**When NOT to Use**:
- Batch processing (use Tier 4)
- One-shot computations
- Random access patterns
- No temporal correlation between events

---

## Tier 6: Mixed Capsules (Hybrid Coordination + Computation)
**Latency**: Variable (depends on composition)
**Size**: Variable (composite structure)
**Use Case**: Complex workflows combining multiple tiers
**Composition**: Atomic coordination + SIMD computation + Fixed-point precision

#### Proposed Patterns (Design Phase)

| Pattern | Tiers | Size | Purpose | Complexity |
|---------|-------|------|---------|------------|
| **ACP-512** | Atomic + SIMD | 512-bit | Coordinated parallel scoring | Medium |
| **BCB-8K** | Batch + Atomic | 8KB | Batch with circuit breaker | Medium |
| **SCA-256** | SIMD + Atomic | 256-bit | SIMD with coordination | High |
| **FPS-1K** | Fixed-point + SIMD | 1KB | Deterministic batch P&L | High |

**When to Use**:
- Complex workflows requiring multiple optimization strategies
- Coordination + computation in single hot path
- Performance-critical mixed operations

**When NOT to Use**:
- Simple single-tier problems
- Unclear performance requirements
- Premature optimization

---

## Tier 7: GPU/Accelerator Capsules (Massive Parallelism) [RESEARCH]

**Status**: UNEXPLOITED ❌
**Latency**: 100μs-10ms (kernel launch overhead)
**Throughput**: 100-1000× for parallel workloads
**Size**: Variable (CPU shadow + GPU device memory)
**Use Case**: Embarrassingly parallel computation
**Hardware**: CUDA, Vulkan Compute, OpenCL, Metal

#### Architecture

```rust
#[repr(C, align(128))]
pub struct GpuCapsule<T, const N: usize> {
    cpu_shadow: [T; N],        // CPU-visible copy
    gpu_buffer: CudaDevicePtr, // GPU memory
    sync_state: AtomicU64,     // CPU-GPU sync
}
```

#### Use Cases

1. **Matrix Operations**: 100-1000× speedup for large matrices (N ≥ 1024×1024)
2. **Hash Computation**: 10000× speedup for parallel hash cracking
3. **Image Processing**: 100-500× speedup for filters, convolutions
4. **Ray Tracing**: 100-1000× speedup for parallel ray intersection
5. **Monte Carlo**: 1000× speedup for parallel random simulations

#### When to Use
- Large datasets (N > 10000)
- Embarrassingly parallel (no data dependencies)
- Computation-bound (not memory-bound)
- Can tolerate 100μs kernel launch overhead

#### When NOT to Use
- Small datasets (N < 1000) - CPU faster due to transfer overhead
- Sequential algorithms
- Latency-critical (<1ms total)
- Irregular control flow (lots of branches)

**Validation Required**: B32 benchmark on NVIDIA A100/H100

---

## Tier 8: Network Capsules (Zero-Copy I/O) [RESEARCH]

**Status**: UNEXPLOITED ❌
**Latency**: Sub-microsecond packet processing
**Throughput**: 5-10× network throughput
**Size**: 4KB-64KB (ring buffers)
**Use Case**: Zero-copy packet processing, kernel bypass
**Hardware**: DPDK, io_uring, XDP (eBPF)

#### Architecture

```rust
#[repr(C, align(4096))] // Page-aligned for zero-copy DMA
pub struct NetworkCapsule {
    packet_ring: [PacketDescriptor; 2048],
    head: AtomicU64,  // Producer (NIC)
    tail: AtomicU64,  // Consumer (app)
}
```

#### Use Cases

1. **HFT Market Data**: 5-10× throughput for tick-by-tick ingestion
2. **CDN Edge**: Sub-microsecond HTTP request parsing
3. **Network Monitoring**: 10Gbps+ packet inspection
4. **Load Balancers**: Zero-copy L4/L7 routing
5. **VPN Gateways**: Inline encryption without copies

#### Performance Gap

**Traditional Network I/O** (3 copies):
- NIC → kernel buffer (DMA): 100ns
- Kernel → socket buffer: 100ns (copy)
- Socket → user space: 100ns (copy)
- **Total**: 300ns overhead per packet

**Zero-Copy Network Capsule**:
- NIC → capsule ring buffer (DMA): 100ns
- User reads from ring: 10ns (atomic read)
- **Total**: 110ns (2.7× faster)

#### When to Use
- High packet rate (>100K pps)
- Low latency requirements (<100μs)
- Control over full network stack

#### When NOT to Use
- Standard socket I/O sufficient
- Portability required
- Complex protocol stacks

**Validation Required**: B32 benchmark with DPDK on 10Gbps NIC

---

## Tier 9: Persistent Capsules (Crash-Safe Storage) [RESEARCH]

**Status**: UNEXPLOITED ❌
**Latency**: 10μs write (NVMe), 100ns read (cached)
**Throughput**: 10-100× vs traditional databases
**Size**: 4KB-16MB (page-aligned)
**Use Case**: Crash-safe atomic state, embedded databases
**Hardware**: NVMe SSD, memory-mapped files, WAL

#### Architecture

```rust
#[repr(C, align(4096))] // Page-aligned for mmap
pub struct PersistentCapsule<T> {
    data: T,
    checksum: u64,           // CRC32 for corruption detection
    generation: AtomicU64,
    wal_offset: AtomicU64,   // Write-ahead log position
}
```

#### Use Cases

1. **Embedded Databases**: 10-100× faster than SQLite (no serialization)
2. **Crash-Safe State Machines**: Survive power loss
3. **Configuration Storage**: Atomic updates with rollback
4. **Checkpoint/Restore**: Fast application recovery (<10ms)
5. **Event Sourcing**: Durable event log

#### Performance Comparison

**SQLite with ACID**:
- Index lookup: 50μs
- Transaction: 200μs-2ms overhead
- Write throughput: ~1K ops/sec

**Persistent Capsule**:
- Memory-mapped read: 100ns (cached)
- WAL write: 10μs (NVMe)
- Write throughput: ~100K ops/sec
- **Speedup**: 10-100×

#### When to Use
- Need durability without database overhead
- Crash-safety required
- Fast recovery critical (<10ms)
- Known data structure size

#### When NOT to Use
- Complex queries required (use database)
- ACID transactions across multiple records
- Variable-size data structures

**Validation Required**: B32 benchmark vs SQLite on NVMe

---

## Tier 10: Probabilistic Capsules (Approximate Data Structures) [RESEARCH]

**Status**: UNEXPLOITED ❌
**Latency**: <100ns operations
**Space Reduction**: 100-1000× vs exact structures
**Accuracy**: 99%+ (configurable error rate)
**Use Case**: Cardinality estimation, frequency counting, membership testing
**Algorithms**: HyperLogLog, Count-Min Sketch, Bloom filters

#### HyperLogLog Capsule (Cardinality Estimation)

```rust
#[repr(C, align(64))]
pub struct HyperLogLogCapsule {
    registers: [AtomicU8; 2048],  // 2KB for billions of values
    hash_seed: u64,
}
```

**Space Savings**: 2KB vs GB for exact set (1000× reduction)
**Error Rate**: ±2% standard error
**Use Case**: Unique visitor counting, database cardinality

#### Count-Min Sketch Capsule (Frequency Counting)

```rust
#[repr(C, align(64))]
pub struct CountMinSketchCapsule {
    counters: [[AtomicU32; 2048]; 4],  // 8KB
    hash_seeds: [u64; 4],
}
```

**Space Savings**: 8KB vs GB for exact hash table (100× reduction)
**Error Rate**: Overestimates by ≤ε with probability 1-δ
**Use Case**: Hot keys detection, frequency estimation

#### Bloom Filter Capsule (Membership Testing)

```rust
#[repr(C, align(64))]
pub struct BloomFilterCapsule {
    bits: [AtomicU64; 2048],  // 16KB bit array (128K bits)
    hash_seeds: [u64; 3],
}
```

**Space Savings**: 16KB vs MB for exact set (100× reduction)
**False Positive Rate**: 1% with 3 hash functions
**Use Case**: Cache admission, deduplication detection

#### Use Cases

1. **Analytics**: Unique visitors (HyperLogLog)
2. **Deduplication**: Duplicate detection (Bloom filter)
3. **Hot Keys**: Frequency estimation (Count-Min Sketch)
4. **Caching**: Admission policy (Bloom filter)
5. **Anomaly Detection**: Rare events (Count-Min Sketch)

#### When to Use
- Exact precision not required
- Space/speed more important than accuracy
- Acceptable error rate (1-2%)
- Massive scale (billions of items)

#### When NOT to Use
- Need exact results
- Cannot tolerate false positives
- Small datasets (exact structures fast enough)

**Validation Required**: B32 benchmark accuracy and space savings

---

## Tier 6 (Mixed) - Extended Patterns [UCE33 ANALYSIS]

**Note**: UCE33 analysis identified **20+ unexplored Tier 6 combinations** with compound speedups from 24× to 2000×. See `/home/samuel/Primitives/Docs/COMPUTATIONAL_CAPSULE_UCE33_ANALYSIS.md` for complete details.

### High-Value Tier 6 Combinations

#### 1. Atomic + Fixed-Point + SIMD (24× potential)
```rust
#[repr(C, align(128))]
pub struct DeterministicParallelPnlCapsule {
    breaker_state: AtomicU64,     // Tier 1: Atomic coordination
    pnl_fixed: [AtomicI64; 8],    // Tier 3: Q8.8 fixed-point
    positions: [f64; 8],           // Tier 2: SIMD-aligned
}
```
**Use Case**: Deterministic parallel portfolio P&L
**Speedup**: 3× (Atomic) × 2× (Fixed-Point) × 4× (SIMD) = 24×

#### 2. Batch + SIMD + Compressed (120× potential)
```rust
#[repr(C, align(64))]
pub struct CompressedBatchCapsule {
    batch: [LogEntry; 512],       // Tier 4: Batch (32KB)
    compressed: Vec<u8>,          // LZ4 compression (10KB)
    // Tier 2: SIMD parsing (8 entries parallel)
}
```
**Use Case**: High-throughput compressed log processing
**Speedup**: 10× (Batch) × 4× (SIMD) × 3× (Compression) = 120×

#### 3. Streaming + Atomic + SIMD (12× coordination)
```rust
#[repr(C, align(128))]
pub struct StreamingMetricsCapsuleSimd {
    window: [AtomicU32; 60],      // Tier 5: Streaming (60-second)
    current_idx: AtomicU8,        // Tier 1: Atomic coordination
    aggregation_buffer: [u32; 16], // Tier 2: SIMD (u32x8)
}
```
**Use Case**: Real-time metrics dashboard with parallel aggregation
**Speedup**: 3× (Atomic) × 4× (SIMD) = 12×

#### 4. Persistent + Atomic + SIMD (210× potential)
```rust
#[repr(C, align(4096))]
pub struct PersistentOlapCapsule {
    mmap_table: MmapMut,          // Tier 9: Persistent (zero-copy)
    txn_state: AtomicU64,         // Tier 1: Atomic coordination
    scan_buffer: [f32; 256],      // Tier 2: SIMD query execution
}
```
**Use Case**: Crash-safe OLAP database with vectorized queries
**Speedup**: 10× (Persistent) × 3× (Atomic) × 7× (SIMD) = 210×

#### 5. GPU + Fixed-Point + Batch (2000× potential)
```rust
#[repr(C, align(128))]
pub struct QuantizedGpuBatchCapsule {
    gpu_weights: CudaDevicePtr,   // Tier 7: GPU (1000+ CUDA cores)
    quantized_weights: Vec<i8>,   // Tier 3: INT8 quantization
    batch_inputs: Vec<[f32; 784]>,// Tier 4: Batch (1024 samples)
}
```
**Use Case**: Quantized neural network training on GPU
**Speedup**: 100× (GPU) × 2× (Fixed-Point) × 10× (Batch) = 2000×

### Additional Tier 6 Patterns (15+ More)

See UCE33 analysis document for:
6. Atomic + Batch + SIMD (48× potential)
7. Fixed-Point + SIMD + Streaming (24× potential)
8. Persistent + Batch + Compressed (300× potential)
9. GPU + SIMD + Streaming (400× potential)
10. Network + Atomic + SIMD (60× potential)
11. Probabilistic + SIMD + Batch (400× potential)
12. Encrypted + Atomic + Fixed-Point (12× potential)
13. NUMA + Atomic + SIMD (24× potential)
14. Huge Page + Batch + Streaming (100× potential)
15. AVX-512 + Fixed-Point + Batch (48× potential)
16. AMX + Batch + Persistent (500× potential)
17. AES-NI + Network + Atomic (30× potential)
18. GPU + Probabilistic + Batch (10000× potential)
19. Persistent + Probabilistic + Streaming (1000× potential)
20. All-Tier Hybrid (100000× potential for specialized workloads)

**Each pattern requires**: UCE33 Q33 analysis, B32 benchmark validation, T28 testing, production use case

---

## Selection Decision Tree

```
┌─ Need coordination between threads?
│  └─ YES: Latency < 100ns?
│     └─ YES: **Tier 1: Atomic Capsule**
│     └─ NO:  Consider Tier 6 (Atomic + other tier)
│
├─ Vectorizable computation (same op on multiple data)?
│  └─ YES: Batch size 4-16 elements?
│     └─ YES: **Tier 2: SIMD Capsule**
│     └─ NO:  Consider Tier 4 (Batch) or Tier 6 (Mixed)
│
├─ Exact decimal arithmetic required?
│  └─ YES: Financial/regulatory calculations?
│     └─ YES: **Tier 3: Fixed-Point Capsule**
│     └─ NO:  Consider floating-point (not a capsule)
│
├─ High-throughput batch processing?
│  └─ YES: Independent operations, >100 elements?
│     └─ YES: **Tier 4: Batch Capsule**
│     └─ NO:  Consider Tier 2 (SIMD) for smaller batches
│
├─ Continuous stream with windowing?
│  └─ YES: Temporal aggregation needed?
│     └─ YES: **Tier 5: Streaming Capsule**
│     └─ NO:  Consider event-based processing
│
└─ Complex workflow combining multiple strategies?
   └─ YES: **Tier 6: Mixed Capsule**
   └─ NO:  Re-analyze requirements
```

---

## Pattern Naming Convention

**Format**: `{Tier}{Purpose}-{Size}`

**Tier Prefixes**:
- `A`: Atomic (Tier 1) - e.g., ACB-64 (Atomic Circuit Breaker)
- `S`: SIMD (Tier 2) - e.g., SVC-256 (SIMD Venue Comparison)
- `FP`: Fixed-Point (Tier 3) - e.g., FP-Q88-64 (Fixed-Point Q8.8)
- `B`: Batch (Tier 4) - e.g., BFE-4K (Batch Feature Extraction)
- `S`: Streaming (Tier 5) - e.g., SMA-W (Streaming Moving Average)
- Mixed (Tier 6): Use combined prefixes - e.g., ACP-512 (Atomic + SIMD Coordinated Parallel)

**Examples**:
- **ACB-64**: Atomic Circuit Breaker, 64 bits
- **SVC-256**: SIMD Venue Comparison, 256 bits (f64x4)
- **FP-Q88-64**: Fixed-Point Q8.8 format, 64 bits
- **BFE-4K**: Batch Feature Extraction, 4KB
- **SMA-W**: Streaming Moving Average, Windowed
- **ACP-512**: Atomic + SIMD Coordinated Parallel, 512 bits

---

## Performance Expectations (B32 Reality Check)

### Realistic Speedup Ranges

| Tier | Typical Speedup | Exceptional Speedup | Validation Method |
|------|-----------------|---------------------|-------------------|
| Tier 1 (Atomic) | 3-10x vs mutex | 10x+ under heavy contention | B32 benchmarks, 95% CI |
| Tier 2 (SIMD) | 2-4x vs scalar | 6-8x cache+vectorization | Statistical validation |
| Tier 3 (Fixed-Point) | 2-5x vs FP | 10x+ regulatory/audit | Determinism verification |
| Tier 4 (Batch) | 10-50x vs one-at-a-time | 100x+ overhead amortization | Throughput measurement |
| Tier 5 (Streaming) | Configurable | Depends on window size | Latency profiling |
| Tier 6 (Mixed) | Compound (multiply speedups) | Highly variable | Component benchmarking |

### B32 Hardware Reality Checks

**H1-H10**: Fundamental hardware constraints
- H1: Cache line fetches dominate sub-100ns operations
- H2: SIMD limited by memory bandwidth, not compute
- H3: Atomic operations have 5-15ns hardware latency floor
- H4: False sharing can negate 90%+ of alignment benefits
- H5: Unaligned loads cost 3-5x latency penalty

**H11-H20**: Performance measurement best practices
- H11: Always measure baseline with same hardware/compiler
- H12: Use 95% confidence intervals, minimum 1000 iterations
- H13: Report p50/p95/p99/p999 (tail latency matters)
- H14: Compare against optimized baseline, not strawman
- H15: Validate reproducibility across runs

**H21-H27**: Realistic expectations
- H21: 10-50% improvements are common and valuable
- H22: 2-10x speedups require significant algorithmic changes
- H23: 100x+ claims require extraordinary validation
- H24: SIMD typically 2-4x (not 8x or 16x in practice)
- H25: Cache optimization often 10-50%, rarely more

---

## Cross-Reference Matrix

### Tier Compatibility

| Primary Tier | Compatible Tiers | Integration Pattern | Complexity |
|--------------|------------------|---------------------|------------|
| Tier 1 (Atomic) | All tiers | Atomic coordination of other computations | Low-Medium |
| Tier 2 (SIMD) | Tier 1, 3, 4 | Vectorized computation with coordination | Medium |
| Tier 3 (Fixed-Point) | Tier 1, 2 | Deterministic arithmetic with SIMD | Medium |
| Tier 4 (Batch) | Tier 1, 2, 3 | Batch processing with SIMD/Fixed-point | Medium-High |
| Tier 5 (Streaming) | Tier 1, 2, 3 | Continuous computation with capsules | High |

### Use Case Matrix

| Use Case | Primary Tier | Secondary Tier | Example Pattern |
|----------|--------------|----------------|-----------------|
| HFT circuit breaker | Tier 1 (Atomic) | - | ACB-64 |
| Multi-venue scoring | Tier 2 (SIMD) | Tier 1 (coordination) | SVC-256 |
| P&L tracking | Tier 3 (Fixed-Point) | Tier 1 (coordination) | PNL-512 |
| Feature engineering | Tier 4 (Batch) | Tier 2 (SIMD) | BFE-4K |
| Real-time monitoring | Tier 5 (Streaming) | Tier 1 (coordination) | SMA-W |

---

## Further Reading

### Documentation
- `/home/samuel/Docs/The Computational Capsule.md` - Complete architecture guide
- `/home/samuel/Primitives/docs/ATOMIC_CAPSULE_PATTERNS.md` - Production atomic patterns
- `/home/samuel/Primitives/docs/SIMD_CAPSULE_QUICK_START.md` - SIMD implementation guide

### Framework References
1. **UCE32 Framework**: Q28-Q32 systematic analysis
2. **ASSUM Safety**: Safety assumption validation
3. **B32 Benchmarking**: Performance validation methodology
4. **T28 Testing**: Comprehensive testing strategy

---

## Conclusion

**Computational capsule taxonomy** provides systematic selection guidance:

1. **Tier 1 (Atomic)**: <100ns lockfree coordination (3-10x speedup)
2. **Tier 2 (SIMD)**: Vectorized computation (2-8x speedup)
3. **Tier 3 (Fixed-Point)**: Deterministic precision (2-10x speedup)
4. **Tier 4 (Batch)**: Throughput processing (10-100x speedup)
5. **Tier 5 (Streaming)**: Continuous computation (configurable)
6. **Tier 6 (Mixed)**: Hybrid coordination + computation (compound speedups)

**Selection Process**:
1. Identify performance bottleneck via profiling
2. Classify bottleneck characteristics (coordination, computation, precision)
3. Select appropriate tier from decision tree
4. Validate with B32 framework (statistical rigor)
5. Integrate with existing system

**Performance Reality**:
- Most improvements: 10-50% (common and valuable)
- Good improvements: 2-10x (algorithmic changes)
- Exceptional improvements: 10x+ (requires extensive validation)

**Remember**: Choose the simplest tier that meets requirements. Premature optimization to higher tiers adds complexity without proportional benefit.
