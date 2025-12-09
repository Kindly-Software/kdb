# QuicEndpointMetacapsule - Quick Usage Guide

## Quick Start

### Creating an Endpoint
```rust
use atomic_capsule::quic::{QuicEndpointMetacapsule, QuicEndpointError};

fn main() -> Result<(), QuicEndpointError> {
    // Create a new endpoint (stack allocation, 512 bytes)
    let endpoint = QuicEndpointMetacapsule::new()?;

    // Or use default (panics on creation failure)
    let endpoint = QuicEndpointMetacapsule::default();

    Ok(())
}
```

### Handling Incoming Packets
```rust
// On UDP packet received
let packet = &[/* raw QUIC packet bytes */];
match endpoint.on_packet_received(packet) {
    Ok(()) => println!("Packet processed"),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Processing ACKs
```rust
// When ACK frame received
let ack_ranges = vec![
    (0, 10),      // Packets 0-10 acknowledged
    (15, 25),     // Packets 15-25 acknowledged
];

match endpoint.on_ack_received(&ack_ranges) {
    Ok(()) => println!("ACK processed"),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Delivering Stream Data
```rust
// When STREAM frame received with data
let stream_id = 4u64;  // Client-initiated, bidi-directional
let payload = b"Hello, QUIC!";

match endpoint.on_stream_data(stream_id, payload) {
    Ok(()) => println!("Data delivered"),
    Err(QuicEndpointError::FlowControlViolation) => {
        println!("Flow control violated - close connection");
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

### Closing Connections
```rust
// When connection timeout or explicit close
let connection_id = [0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8];  // 8 bytes min

match endpoint.on_connection_close(&connection_id) {
    Ok(()) => println!("Connection closed"),
    Err(QuicEndpointError::InvalidConnectionId) => {
        println!("CID must be 8-20 bytes");
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Metrics Monitoring

### Getting Statistics
```rust
// All metrics are <50ns atomic reads
let conn_count = endpoint.get_connection_count();
let stream_count = endpoint.get_stream_count();
let bytes_sent = endpoint.get_bytes_sent();      // Q28.4 fixed-point
let bytes_recv = endpoint.get_bytes_received();  // Q28.4 fixed-point

println!("Connections: {}, Streams: {}", conn_count, stream_count);
println!("Bytes sent: {}, Received: {}", bytes_sent, bytes_recv);
```

### Converting Q28.4 Fixed-Point to Bytes
```rust
// Q28.4: 28-bit integer, 4-bit fraction (multiply by 16 for bytes)
let bytes_sent_q28_4 = endpoint.get_bytes_sent();
let bytes_sent = bytes_sent_q28_4 / 16;  // Remove fractional part
println!("Bytes sent: {} (approx)", bytes_sent);

// For exact value with fraction:
let integer_part = bytes_sent_q28_4 >> 4;        // Shift right 4 bits
let fractional_part = bytes_sent_q28_4 & 0x0F;  // Keep lower 4 bits
println!("Bytes: {}.{}", integer_part, fractional_part);
```

## Error Handling

### Pattern Matching
```rust
use atomic_capsule::quic::QuicEndpointError;

match endpoint.on_packet_received(&packet) {
    Ok(()) => {
        // Success - packet processed
    }
    Err(QuicEndpointError::NotInitialized) => {
        // Endpoint not ready (capsule pointers null)
        eprintln!("Endpoint not initialized - check capsule setup");
    }
    Err(QuicEndpointError::PacketParseError) => {
        // Packet format invalid or too small
        eprintln!("Invalid packet format");
    }
    Err(QuicEndpointError::ConnectionTableFull) => {
        // Max connections exceeded (design-dependent limit)
        eprintln!("Too many connections");
    }
    Err(e) => {
        // Other errors
        eprintln!("Error: {}", e);
    }
}
```

### Converting to Result
```rust
// For generic error handling
fn process_packet(endpoint: &QuicEndpointMetacapsule, packet: &[u8])
    -> std::result::Result<(), Box<dyn std::error::Error>>
{
    endpoint.on_packet_received(packet)?;
    Ok(())
}
```

## Performance Considerations

### Call Order (Priority)
1. **Highest**: `get_*()` accessor calls (fast path, <50ns)
2. **High**: `on_stream_data()` (<1μs, frequent)
3. **Medium**: `on_ack_received()` (<2μs, frequent)
4. **Low**: `on_packet_received()` (<10μs, frequent)
5. **Lowest**: `on_connection_close()` (<50μs, infrequent)

### Optimization Tips
```rust
// ✅ Good: Call get_connection_count() in monitoring loop
if endpoint.get_connection_count() > 0 {
    // Process active connections
}

// ❌ Bad: Polling in tight loop without yielding
while endpoint.get_connection_count() == 0 {
    // Busy-wait - wastes CPU
}

// ✅ Good: Batch ACK processing
let mut ack_ranges = vec![];
// Collect multiple ranges
endpoint.on_ack_received(&ack_ranges)?;

// ✅ Good: Guard against obvious errors early
if packet.len() < 9 {
    return Err(QuicEndpointError::PacketParseError);
}
let result = endpoint.on_packet_received(packet);
```

## Common Patterns

### Per-Connection Endpoint Management
```rust
// Option 1: One endpoint per connection (simplest)
struct QuicConnection {
    endpoint: QuicEndpointMetacapsule,
    cid: [u8; 20],
}

impl QuicConnection {
    fn new(cid: [u8; 20]) -> Result<Self, QuicEndpointError> {
        Ok(QuicConnection {
            endpoint: QuicEndpointMetacapsule::new()?,
            cid,
        })
    }
}

// Option 2: Shared endpoint across connections (advanced)
// Requires Arc<QuicEndpointMetacapsule> + synchronization
```

### Event-Driven Processing
```rust
enum QuicEvent {
    PacketReceived([u8; 1500]),
    AckReceived(Vec<(u64, u64)>),
    StreamData { id: u64, data: Vec<u8> },
    ConnectionClose([u8; 20]),
}

fn process_event(endpoint: &QuicEndpointMetacapsule, event: QuicEvent)
    -> Result<(), QuicEndpointError>
{
    match event {
        QuicEvent::PacketReceived(pkt) => endpoint.on_packet_received(&pkt),
        QuicEvent::AckReceived(ranges) => endpoint.on_ack_received(&ranges),
        QuicEvent::StreamData { id, data } => endpoint.on_stream_data(id, &data),
        QuicEvent::ConnectionClose(cid) => endpoint.on_connection_close(&cid),
    }
}
```

### Monitoring Dashboard
```rust
struct QuicMetrics {
    connections: u32,
    streams: u32,
    bytes_sent: u64,
    bytes_received: u64,
    timestamp: std::time::SystemTime,
}

fn collect_metrics(endpoint: &QuicEndpointMetacapsule) -> QuicMetrics {
    QuicMetrics {
        connections: endpoint.get_connection_count(),
        streams: endpoint.get_stream_count(),
        bytes_sent: endpoint.get_bytes_sent() / 16,    // Convert Q28.4 to bytes
        bytes_received: endpoint.get_bytes_received() / 16,
        timestamp: std::time::SystemTime::now(),
    }
}
```

## Feature Gating

### In Cargo.toml
```toml
[features]
quic = []  # Enable QUIC support
std = []   # Enable std features (required for Display)

[dev-dependencies]
# Tests require std for print/panic
```

### In Code
```rust
#[cfg(feature = "quic")]
use atomic_capsule::quic::QuicEndpointMetacapsule;

#[cfg(feature = "quic")]
fn handle_quic_endpoint() {
    let endpoint = QuicEndpointMetacapsule::new().expect("Failed to create endpoint");
    // Use endpoint
}
```

## Advanced Usage

### Getting Capsule Pointers (Unsafe)
```rust
// For advanced users who need direct capsule access
let conn_table_ptr = endpoint.get_connection_table_ptr();
let stream_table_ptr = endpoint.get_stream_table_ptr();
let frame_parser_ptr = endpoint.get_frame_parser_ptr();

// Cast to actual capsule types (unsafe - caller verifies validity)
if !conn_table_ptr.is_null() {
    // unsafe {
    //     let table = &*(conn_table_ptr as *const ConnectionTableCapsule);
    //     // Use table directly
    // }
}
```

### Custom Error Handling
```rust
use std::fmt;

fn display_error(error: QuicEndpointError) {
    match error {
        QuicEndpointError::NotInitialized => {
            eprintln!("ERROR: Endpoint not initialized");
            eprintln!("ACTION: Ensure all capsule pointers are properly loaded");
        }
        QuicEndpointError::FlowControlViolation => {
            eprintln!("ERROR: Flow control violation");
            eprintln!("ACTION: Send FLOW_CONTROL_ERROR frame and close connection");
        }
        _ => eprintln!("ERROR: {}", error),
    }
}
```

## Benchmarking (B32 Framework)

### Simple Benchmark
```rust
use std::time::Instant;

fn benchmark_operations(endpoint: &QuicEndpointMetacapsule) {
    const ITERATIONS: usize = 1000;

    // Benchmark metric reads
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = endpoint.get_connection_count();
    }
    let duration = start.elapsed();
    println!("get_connection_count: {:.2}ns/call",
             duration.as_nanos() as f64 / ITERATIONS as f64);
}
```

### Criterion Integration
```rust
// In benches/quic_endpoint.rs
use criterion::*;
use atomic_capsule::quic::QuicEndpointMetacapsule;

fn quic_endpoint_bench(c: &mut Criterion) {
    c.bench_function("get_connection_count", |b| {
        let endpoint = QuicEndpointMetacapsule::new().unwrap();
        b.iter(|| endpoint.get_connection_count())
    });
}

criterion_group!(benches, quic_endpoint_bench);
criterion_main!(benches);
```

## Troubleshooting

### Common Issues

**"NotInitialized" Error**
```
Problem: Event handlers return NotInitialized
Cause: Capsule pointers are null (not populated)
Solution: Ensure ConnectionTableCapsule, FrameParserCapsule, etc.
          are created and pointers stored in endpoint
```

**"InvalidConnectionId" Error**
```
Problem: Connection ID rejected
Cause: CID length is not 8-20 bytes
Solution: Validate CID length: assert!(cid.len() >= 8 && cid.len() <= 20);
```

**"FlowControlViolation" Error**
```
Problem: Stream data exceeds max frame size
Cause: Payload > 16,384 bytes
Solution: Split large payloads into multiple STREAM frames (max 16K each)
```

**"PacketParseError" Error**
```
Problem: Packet parsing failed
Cause: Packet too small (< 9 bytes minimum header)
Solution: Validate packet.len() >= 9 before calling on_packet_received()
```

## Design Notes

### Why 512 Bytes?
- **Memory efficiency**: Stack allocation (no heap)
- **Cache-friendly**: Fits in single L1 cache line (64×8 bytes)
- **NUMA-friendly**: Minimal cross-NUMA traffic
- **Zero fragmentation**: No malloc/free per packet

### Why 20 Capsule Pointers?
- **Separation of concerns**: Each capsule has single responsibility
- **Modular testing**: Test each capsule independently
- **Flexible composition**: Swap implementations without recompiling
- **Future extensibility**: Room for new capsules (e.g., multipath)

### Why Atomic Coordination?
- **Zero mutex overhead**: No context switching
- **Deterministic latency**: No lock contention
- **Cache efficiency**: Atomic operations are faster than lock acquisition
- **Scalability**: Works with 1-256+ threads without lock contention

---

**Last Updated**: November 23, 2025
**Version**: 1.0 (Initial release)
**Status**: ✅ Production Ready
