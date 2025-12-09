# Developer Guide

**Getting started with Kindly Coin development**

---

## Quick Start

### Prerequisites

```bash
# Rust (nightly recommended)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default nightly

# Build dependencies
sudo apt-get install build-essential pkg-config libssl-dev
```

### Clone and Build

```bash
# Clone repository
git clone https://github.com/kindly/kindly-coin.git
cd kindly-coin

# Build all crates
cargo build --release

# Run tests
cargo test --all

# Run benchmarks
cargo bench --all
```

---

## Project Structure

```
kindly-coin/
├── kindly_core/          # Core atomic capsule primitives
│   ├── src/
│   │   ├── transaction.rs    # AtomicTransactionCapsule
│   │   ├── block.rs          # AtomicBlockCapsule
│   │   └── account.rs        # AccountStateCapsule
│   └── tests/
│
├── kindly_consensus/     # A-BFT consensus engine
│   ├── src/
│   │   ├── validator.rs      # ValidatorCapsule
│   │   ├── abft.rs           # A-BFT implementation
│   │   └── finality.rs       # Finality detection
│   └── tests/
│
├── kindly_ubi/          # Universal Basic Income
│   ├── src/
│   │   ├── distribution.rs   # UBI distribution
│   │   ├── merkle.rs         # Merkle proof claims
│   │   └── fraud.rs          # Fraud detection
│   └── tests/
│
├── kindly_network/      # P2P networking
│   ├── src/
│   │   ├── p2p.rs           # Libp2p integration
│   │   ├── gossip.rs        # Gossip protocol
│   │   └── txpool.rs        # Transaction pool
│   └── tests/
│
├── kindly_governance/   # KYC/AML, tax collection
│   ├── src/
│   │   ├── kyc.rs           # KYC/AML capsules
│   │   ├── tax.rs           # Atomic tax collection
│   │   └── treasury.rs      # Government treasury
│   └── tests/
│
└── atomic_capsule/      # Foundation crate
    ├── src/
    │   ├── alignment.rs     # Cache alignment
    │   ├── retry.rs         # Retry policies
    │   └── verify.rs        # Compile-time verification
    └── tests/
```

---

## Running a Local Node

### Configuration

Create `node.toml`:

```toml
[node]
mode = "validator"  # or "full" for non-validator
data_dir = "/var/lib/kindly-coin"
log_level = "info"

[network]
listen_addr = "/ip4/0.0.0.0/tcp/9000"
bootstrap_peers = [
    "/ip4/127.0.0.1/tcp/9001/p2p/12D3KooWExample1",
    "/ip4/127.0.0.1/tcp/9002/p2p/12D3KooWExample2"
]

[consensus]
validator_key = "/etc/kindly-coin/validator.key"
stake_amount = "1000000000000"  # 1000 coins

[api]
rest_addr = "127.0.0.1:8080"
websocket_addr = "127.0.0.1:8081"
```

### Start Node

```bash
# Validator node
cargo run --release --bin kindly-validator -- --config node.toml

# Full node (non-validator)
cargo run --release --bin kindly-node -- --config node.toml

# Check status
curl http://localhost:8080/v1/node/status
```

---

## Submitting Transactions

### Using CLI

```bash
# Create wallet
kindly-cli wallet create --name my_wallet

# Check balance
kindly-cli wallet balance --address 0x1234...5678

# Send transaction
kindly-cli tx send \
  --from 0x1234...5678 \
  --to 0xabcd...ef01 \
  --amount 1000000000000 \
  --fee 10000000

# Check transaction status
kindly-cli tx status --tx-id 0xtx...
```

### Using Rust SDK

```rust
use kindly_sdk::{Client, Transaction, Wallet};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to node
    let client = Client::new("http://localhost:8080").await?;

    // Create wallet
    let wallet = Wallet::new_random();

    // Build transaction
    let tx = Transaction::builder()
        .sender(wallet.address())
        .recipient("0xabcd...ef01".parse()?)
        .amount(1_000_000_000_000u64)  // 1000 coins
        .fee(10_000_000u64)
        .nonce(wallet.next_nonce())
        .sign(&wallet.private_key());

    // Submit transaction
    let tx_id = client.submit_transaction(tx).await?;
    println!("Transaction submitted: {}", tx_id);

    // Wait for finality
    let finalized = client.wait_for_finality(tx_id, Duration::from_secs(1)).await?;
    println!("Transaction finalized: {}", finalized);

    Ok(())
}
```

---

## Running Validators

### Generate Validator Key

```bash
# Generate key pair
kindly-cli validator generate-key --output validator.key

# Output:
# Public key:  0xval...
# Private key: /etc/kindly-coin/validator.key (DO NOT SHARE)
```

### Register Validator

```rust
use kindly_sdk::{Client, Validator};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new("http://localhost:8080").await?;

    // Load validator key
    let validator_key = std::fs::read("/etc/kindly-coin/validator.key")?;

    // Register validator (requires stake)
    let validator = Validator::new(validator_key);
    let stake_amount = 1_000_000_000_000u64;  // 1000 coins minimum

    let tx_id = client.register_validator(validator, stake_amount).await?;
    println!("Validator registered: {}", tx_id);

    Ok(())
}
```

### Monitor Validator Performance

```bash
# Check validator status
curl http://localhost:8080/v1/validators/0xval.../status

# Response:
{
  "validator_id": "0xval...",
  "stake": "1000000000000",
  "active": true,
  "blocks_produced": 1500,
  "uptime_percentage": 99.8,
  "phi_score": 1234567
}
```

---

## Integration Testing

### Setup Test Environment

```rust
use kindly_test_utils::{TestNet, TestWallet};

#[tokio::test]
async fn test_transaction_flow() {
    // Create local testnet (3 validators)
    let testnet = TestNet::new(3).await;

    // Create wallets
    let alice = TestWallet::new_funded(1_000_000_000_000u64);
    let bob = TestWallet::new_funded(0);

    // Submit transaction
    let tx = alice.send_to(bob.address(), 500_000_000_000u64);
    let tx_id = testnet.submit(tx).await.unwrap();

    // Wait for finality (<10ms in testnet)
    testnet.wait_for_finality(tx_id).await.unwrap();

    // Verify balances
    assert_eq!(alice.balance().await, 500_000_000_000u64);
    assert_eq!(bob.balance().await, 500_000_000_000u64);
}
```

### Byzantine Fault Testing

```rust
use kindly_test_utils::{TestNet, ByzantineValidator};

#[tokio::test]
async fn test_byzantine_tolerance() {
    // Create testnet with 10 validators
    let mut testnet = TestNet::new(10).await;

    // Make 3 validators Byzantine (30%, below 33% threshold)
    testnet.set_byzantine(0, ByzantineValidator::AlwaysReject);
    testnet.set_byzantine(1, ByzantineValidator::RandomVote);
    testnet.set_byzantine(2, ByzantineValidator::Offline);

    // Submit transaction
    let tx = testnet.create_test_transaction();
    let tx_id = testnet.submit(tx).await.unwrap();

    // Should still finalize (7 honest validators = 70% > 67%)
    testnet.wait_for_finality(tx_id).await.unwrap();

    // Verify consensus safety
    assert!(testnet.verify_no_forks().await);
}
```

---

## Benchmarking

### Transaction Validation Benchmark

```rust
use criterion::{criterion_group, criterion_main, Criterion};
use kindly_core::AtomicTransactionCapsule;

fn transaction_validation_benchmark(c: &mut Criterion) {
    let capsule = AtomicTransactionCapsule::new();
    capsule.publish(create_valid_transaction());

    c.bench_function("tx_validation", |b| {
        b.iter(|| {
            let result = capsule.validate();
            assert!(result.is_valid());
        });
    });
}

criterion_group!(benches, transaction_validation_benchmark);
criterion_main!(benches);
```

### Run Benchmarks

```bash
# All benchmarks
cargo bench --all

# Specific benchmark
cargo bench --bench transaction_validation

# With profiling
cargo bench --bench transaction_validation -- --profile-time 10
```

---

## Debugging

### Enable Tracing

```rust
use tracing::{info, debug, trace};
use tracing_subscriber;

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("Starting Kindly Coin node");

    // Your code here
    debug!("Transaction validated: {:?}", tx);
    trace!("Capsule state: {:?}", capsule);
}
```

### Debug Capsule State

```rust
use kindly_core::AtomicTransactionCapsule;

let capsule = AtomicTransactionCapsule::new();
capsule.publish(tx);

// Debug print
println!("Capsule state: {:#?}", capsule.debug_state());

// Output:
// Capsule state: {
//   commit: 1,
//   version: 8 (even),
//   sequence: 42,
//   sender: 0x1234...5678,
//   recipient: 0xabcd...ef01,
//   amount: 1000000000000,
//   fee: 10000000,
//   generation: 5,
// }
```

---

## Best Practices

### 1. Atomic Capsule Design

**DO**:
```rust
// Use atomic capsule for coordination
pub struct MyStateCapsule {
    state: AtomicU64,
}

impl MyStateCapsule {
    pub fn update(&self, new_state: u64) {
        self.state.store(new_state, Ordering::Release);
    }

    pub fn read(&self) -> u64 {
        self.state.load(Ordering::Relaxed)
    }
}
```

**DON'T**:
```rust
// DON'T use mutex (defeats lockfree design)
pub struct MyState {
    state: Mutex<u64>,  // ❌ NO!
}
```

### 2. Generation Counters

**DO**:
```rust
// Always increment generation on update
pub fn update_with_generation(&self, value: u64) {
    loop {
        let current = self.state.load(Ordering::Acquire);
        let (old_value, old_gen) = unpack(current);

        let new_gen = old_gen.wrapping_add(1);  // ✅ Increment
        let new_state = pack(value, new_gen);

        if self.state.compare_exchange_weak(...).is_ok() {
            break;
        }
    }
}
```

**DON'T**:
```rust
// DON'T forget generation (ABA vulnerability)
pub fn update_without_generation(&self, value: u64) {
    self.state.store(value, Ordering::Release);  // ❌ ABA risk!
}
```

### 3. Two-Phase Commits

**DO**:
```rust
// Two-phase commit for atomicity
pub fn publish(&self, data: Data) {
    let odd_ver = self.version | 1;  // Odd version

    // Phase 1: Write payload
    self.w1.store(data.field1, Ordering::Relaxed);
    self.w2.store(data.field2, Ordering::Relaxed);
    self.w_tail.store(pack_tail(checksum, odd_ver), Ordering::Relaxed);

    // Phase 2: Commit (even version)
    let head = pack_head(commit:1, ver:odd_ver+1);
    self.w_head.store(head, Ordering::Release);  // ✅ Atomic commit
}
```

---

## Troubleshooting

### Node won't start

**Error**: `Failed to bind to 0.0.0.0:9000`

**Solution**:
```bash
# Check if port is in use
sudo lsof -i :9000

# Kill process or change port in config
```

### Transaction rejected

**Error**: `InsufficientFunds`

**Solution**:
```bash
# Check balance
kindly-cli wallet balance --address 0x1234...5678

# Fund wallet from faucet (testnet)
curl -X POST http://testnet-faucet.kindly.coin/drip \
  -d '{"address": "0x1234...5678"}'
```

### Consensus timeout

**Error**: `ConsensusTimeout: <67% votes after 10ms`

**Solution**:
- Check validator network connectivity
- Verify validator keys are correct
- Ensure sufficient stake (1000+ coins)

---

## Resources

### Documentation
- [Atomic Capsule Architecture](ATOMIC_CAPSULE_ARCHITECTURE.md)
- [Consensus (A-BFT)](CONSENSUS_ABFT.md)
- [UBI Distribution](UBI_DISTRIBUTION.md)
- [API Reference](API_REFERENCE.md)

### Community
- Discord: https://discord.gg/kindlycoin
- Forum: https://forum.kindly.coin
- GitHub: https://github.com/kindly/kindly-coin

### Support
- Email: developers@kindly.software
- Bug reports: https://github.com/kindly/kindly-coin/issues
- Feature requests: https://github.com/kindly/kindly-coin/discussions

---

Next: [GOVERNMENT_PILOT_PROGRAM.md](GOVERNMENT_PILOT_PROGRAM.md) - Pilot design
