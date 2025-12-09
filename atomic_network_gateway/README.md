# Atomic Network Gateway

A lockfree network gateway primitive designed for high-frequency trading systems, implementing UCE32 framework principles with sub-100μs latency requirements.

## UCE32 Framework Application

**Q28 (Simplicity)**: Simple API hiding complex lockfree coordination
**Q29 (Constraints)**: <100μs latency, network bandwidth limits, socket fd limits
**Q30 (Validation)**: Benchmarked throughput with statistical confidence
**Q31 (Rust Transform)**: Zero-copy parsing, atomic coordination, fearless concurrency
**Q32 (Nightly)**: Const generics for compile-time buffer optimization

## Features

### Core Components

- **OrderGateway**: Lockfree order routing with atomic sequence generation
- **MarketDataGateway**: High-throughput lockfree data ingestion
- **SessionManager**: Atomic session lifecycle management with capacity constraints
- **NetworkGateway**: Unified interface combining all components

### Advanced Capabilities

- **GenerationCounter**: TOCTOU prevention through monotonic versioning
- **MessageHeader**: Zero-copy parsing with FIX-like protocol support
- **MessageBuffer**: Compile-time sized buffers with lockfree allocation
- **100% Lockfree Architecture**: No mutex, no RwLock - atomics only

## Performance Benchmarks

```
order_send              time:   [32.342 ns 32.569 ns 32.818 ns]
order_ack               time:   [11.688 ns 11.778 ns 11.857 ns]
market_data_process     time:   [34.569 ns 34.798 ns 35.062 ns]
generation_next         time:   [8.6897 ns 8.7535 ns 8.8189 ns]
generation_current      time:   [244.81 ps 246.56 ps 247.95 ps]
buffer_reserve          time:   [1.0471 ns 1.0661 ns 1.0868 ns]
```

**All operations meet sub-100μs latency requirement with nanosecond performance.**

## Safety Analysis (ASSUM Framework)

### #ASSUME: Network operations can be lockfree with atomic coordination
**#VERIFY**:
- ✓ Generation counters prevent TOCTOU races
- ✓ Memory ordering (Acquire/Release) ensures synchronization
- ✓ Cache-aligned structures (64-byte alignment) prevent false sharing
- ✓ No blocking I/O in hot path

### #ASSUME: Zero-copy parsing is safe with packed structs
**#VERIFY**:
- ✓ Length validation before unsafe pointer operations
- ✓ repr(C, packed) ensures predictable memory layout
- ✓ read_unaligned used for safe unaligned access

## Usage Example

```rust
use atomic_network_gateway::NetworkGateway;

// Create gateway with session capacity constraint
let gateway = NetworkGateway::new(1000);
gateway.start()?;

// Create trading session
let session_id = gateway.sessions.create_session()?;

// Send order with atomic sequence generation
let order_seq = gateway.orders.send_order(
    session_id as u32,
    b"BUY 100 MSFT @ 300.50"
)?;

// Process market data with zero-copy parsing
let md_seq = gateway.market_data.process_market_data(&market_data_bytes)?;

// Process acknowledgments
gateway.orders.process_ack(order_seq)?;

// Get performance statistics
let order_stats = gateway.orders.stats();
let md_stats = gateway.market_data.stats();
```

## Architecture Principles

### Q31 Rust Transformations Applied

1. **Ownership**: Data protection becomes automatic through Drop trait
2. **Zero-cost**: Protection with no runtime overhead via repr(transparent)
3. **Type System**: Gradual state transitions enforced by compile-time checks
4. **Concurrency**: Fearless parallel processing with Send/Sync guarantees
5. **Memory Safety**: Secure operations without leaks via automatic cleanup

### Q32 Nightly Features Utilized

- **Const Generics**: Compile-time buffer sizing for hardware optimization
- **Atomic Operations**: Enhanced memory ordering guarantees
- **Packed Structs**: Zero-copy message parsing with predictable layout

## Testing

```bash
cargo test                    # Run unit and integration tests
cargo bench                   # Run performance benchmarks
cargo test --release          # Test optimized builds
```

### Test Coverage

- ✓ Unit tests for all components
- ✓ Concurrent stress testing (10 threads × 100 operations)
- ✓ Memory safety validation
- ✓ Error path testing
- ✓ State transition validation
- ✓ Performance regression detection

## Real-World Constraints (Q29)

- **Network Bandwidth**: Optimized message formats minimize bytes on wire
- **OS Socket Limits**: Session capacity management prevents fd exhaustion
- **Hardware Latency**: Cache-aligned structures reduce memory access overhead
- **NUMA Effects**: 64-byte alignment matches typical cache line sizes

## Integration

This primitive integrates with other atomic primitives in the trading system:

- **atomic_risk_envelope**: Risk checking before order execution
- **atomic_position_capsule**: Position tracking post-execution
- **atomic_venue_snapshot**: Market data aggregation and processing

## License

MIT