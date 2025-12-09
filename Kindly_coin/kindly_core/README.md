# kindly_core

**Core atomic capsule primitives for Kindly Coin cryptocurrency**

This crate provides the foundational atomic capsules for Kindly Coin:

## Capsules

### AtomicTransactionCapsule (ATC-512)
**Purpose**: Atomic transaction state with deterministic validation

**Layout**: 512 bits (64 bytes), 128-byte aligned
- Head (64 bits): commit flag, version, tx hash, sender, nonce
- Data (64 bits): recipient, amount, fee, timestamp
- Signature (64 bits): Ed25519 signature (r, s)
- Tail (64 bits): version tail, checksum, status, generation

**Performance Target**: <500ns transaction validation

**Key Features**:
- Two-phase commit for atomic publication
- Generation counters for TOCTOU prevention
- Stale flag for reliability detection
- Built-in circuit breaker integration

### AtomicBlockCapsule (ABC-1024)
**Purpose**: Atomic block state with instant finality detection

**Layout**: 1024 bits (128 bytes), 128-byte aligned
- Header: version, height, timestamp, validator
- Merkle root: transaction Merkle tree root
- State root: account state Merkle tree root
- Finality: finality proof and generation counter

**Performance Target**: <1μs block validation

### AccountStateCapsule (ASC-256)
**Purpose**: Lockfree account balance and nonce tracking

**Layout**: 256 bits (32 bytes), 128-byte aligned
- Channel A: balance (52 bits) + generation (12 bits)
- Channel B: nonce (32 bits) + last tx timestamp (32 bits)
- Version control: two-phase commit version
- Padding: cache line alignment

**Performance Target**: <100ns account updates

## Performance

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Transaction validation | <500ns | 2M+ TPS |
| Block validation | <1μs | 1M+ blocks/sec |
| Account update | <100ns | 10M+ updates/sec |

## Usage

```rust
use kindly_core::{AtomicTransactionCapsule, TransactionData};

// Create transaction capsule
let tx_capsule = AtomicTransactionCapsule::new();

// Publish transaction atomically
let tx_data = TransactionData {
    sender: sender_address,
    recipient: recipient_address,
    amount: 1000,
    fee: 10,
    nonce: 42,
};

tx_capsule.publish(tx_data, signature)?;

// Validate transaction (read-only, <500ns)
if tx_capsule.is_valid() {
    let tx = tx_capsule.read()?;
    process_transaction(tx);
}
```

## Safety

All capsules follow the ASSUM safety framework:

- `#ASSUME_ALIGNMENT`: 128-byte cache line alignment prevents false sharing
- `#ASSUME_TWO_PHASE_COMMIT`: Version parity ensures atomic visibility
- `#ASSUME_GENERATION_COUNTER`: Monotonic counters prevent ABA problems
- `#VERIFY_LOCKFREE`: 100% lockfree guarantee (no mutex/RwLock)

## Testing

```bash
# Run unit tests
cargo test

# Run property tests
cargo test --features proptest

# Run benchmarks
cargo bench
```

## License

Licensed under MIT OR Apache-2.0
