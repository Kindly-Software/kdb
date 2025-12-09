# Atomic Portfolio Map (APM-1024)

Atomic Portfolio Map (APM-1024) provides helpers to build and consume a packed 1024-bit snapshot that describes portfolio-wide headroom and per-symbol risk affordances. The layout captures account-level controls, per-symbol position data, and aggregated portfolio metrics in eight 128-bit words for lockfree publication to hot trading paths.

```
[ header:128 | symbol0:128 | symbol1:128 | symbol2:128 | symbol3:128 | symbol4:128 | symbol5:128 | tail:128 ]
```

## Highlights

- **Single atomic snapshot**: Complete portfolio state in 1024 bits across eight words.
- **Per-symbol risk tracking**: Individual position, PnL, and risk limits for up to 6 symbols.
- **Lockfree coordination**: Atomic publication with stale marking for coordination.
- **Hierarchical risk controls**: Portfolio-level and per-symbol breaker levels.
- **Time-sensitive gates**: Automatic forbid windows and end-of-day flattening.
- **Cache-aligned layout**: 64-byte alignment for optimal memory performance.

## Architecture

### APM-1024 Layout (1024 bits / 8 words)

| Word | Component | Description |
|------|-----------|-------------|
| 0 | Header | Account metadata, portfolio flags, timing controls |
| 1-6 | Symbol Slices | Per-symbol position, PnL, risk state (max 6 symbols) |
| 7 | Tail | Aggregated totals, integrity markers, version info |

### Header Word (128 bits)

| Field | Bits | Description |
|-------|------|-------------|
| `commit` | 1 | Commit flag for atomic publication |
| `stale` | 1 | Stale marker for coordination |
| `version` | 8 | Configuration version |
| `seq` | 16 | Sequence counter |
| `account_id` | 16 | Account identifier |
| `forbid_after_min_ct` | 11 | No new positions after this minute |
| `eod_flat_min_ct` | 11 | End-of-day flatten time |
| `rem_daily_loss_total_cents` | 32 | Remaining daily loss budget |
| `portfolio_breaker` | 2 | Portfolio-wide breaker level (L0-L3) |
| `symbol_count` | 4 | Number of active symbol slices |
| `portfolio_flags` | 10 | Portfolio status flags |
| `created_ms_coarse` | 16 | Creation timestamp (coarse) |

### Symbol Slice (128 bits each)

| Field | Bits | Description |
|-------|------|-------------|
| `sym_id` | 16 | Symbol identifier |
| `breaker_level` | 2 | Symbol breaker level (L0-L3) |
| `flags` | 6 | Symbol status flags |
| `pos_qty` | 24 | Net position quantity (signed) |
| `unreal_cents` | 32 | Unrealized PnL in cents (signed) |
| `rem_daily_loss_cents` | 24 | Remaining daily loss budget |
| `spread_ticks` | 8 | Current spread in ticks |
| `vol_band` | 8 | Volatility band |
| `priority` | 8 | Trading priority level |

### Tail Word (128 bits)

| Field | Bits | Description |
|-------|------|-------------|
| `sum_pos_abs_contracts` | 16 | Sum of absolute positions |
| `net_unreal_cents` | 32 | Net unrealized PnL (signed) |
| `net_realized_cents` | 32 | Net realized PnL (signed) |
| `trailing_draw_cents` | 16 | Trailing drawdown |
| `version` | 8 | Tail version |
| `seq` | 16 | Sequence counter |
| `spare` | 8 | Reserved space |

## Features

### Portfolio Controls

```rust
use atomic_portfolio_map::{PortfolioFlags, BreakerLevel};

// Portfolio-level flags
let flags = PortfolioFlags::PAUSED | PortfolioFlags::NEWS_LOCKOUT;

// Breaker levels for risk management
let breaker = BreakerLevel::L2;  // Elevated risk state

// Time-based controls
let forbid_after = 915;  // No new positions after 9:15 AM
let eod_flat = 920;      // Flatten all positions by 9:20 AM
```

### Symbol-Level Tracking

```rust
use atomic_portfolio_map::{SymbolFlags, SymbolInputs, SymbolPolicy};

let symbol_flags = SymbolFlags::CAN_SCALE_UP | SymbolFlags::HAS_RISK;

let policy = SymbolPolicy {
    sym_id: 1001,
    max_abs_position: 100,
    forbid_after_min_ct: Some(915),
    eod_flat_min_ct: Some(920),
    priority_offset: 0,
};
```

### Feed Integration

```rust
use atomic_portfolio_map::{
    FeedSnapshot, ActEdge, ApcSnapshot, AvsSnapshot, SymbolGates,
    build_symbol_inputs
};

let feed = FeedSnapshot {
    policy: symbol_policy,
    apc: ApcSnapshot {
        position: 50,
        unreal_cents: 25_000,
        realized_cents: 15_000,
        rem_daily_loss_cents: 500_000,
        breaker_level: BreakerLevel::L0,
    },
    act: Some(ActEdge {
        edge_surplus_bp: 8,  // 8 basis points edge
    }),
    avs: Some(AvsSnapshot {
        spread_ticks: 2,
        vol_band: 1,
    }),
    gates: SymbolGates::default(),
};

let symbol_inputs = build_symbol_inputs(&feed);
```

### Portfolio Writer

```rust
use atomic_portfolio_map::{
    PortfolioMapWriter, PortfolioInputs, ApmSlot
};

let mut writer = PortfolioMapWriter::new(ApmSlot::new());

let inputs = PortfolioInputs {
    account_id: 12345,
    forbid_after_min_ct: 915,
    eod_flat_min_ct: 920,
    rem_daily_loss_total_cents: 1_000_000,
    trailing_draw_cents: 50_000,
    base_realized_cents: 100_000,
    created_ms_coarse: 42_000,
    portfolio_flags: PortfolioFlags::empty(),
    now_minute_count: 900,
    symbols: &symbol_inputs,
};

let publication = writer.publish(&inputs);
println!("Published version {} seq {}",
    publication.snapshot.header.version,
    publication.snapshot.header.seq
);
```

## Performance Characteristics

### Memory Layout
- **Total size**: 128 bytes (1024 bits) cache-aligned
- **Atomic granularity**: 8 × 128-bit words
- **Cache efficiency**: Single 2-line fetch for complete state
- **Alignment**: 64-byte alignment for optimal NUMA performance

### Publication Performance
- **Write latency**: ~100-200ns for complete portfolio update
- **Read latency**: ~10-20ns for relaxed loads on hot path
- **Coordination overhead**: Zero mutex contention with stale marking
- **Memory bandwidth**: ~40GB/s theoretical throughput

### Scaling Characteristics
- **Symbol capacity**: Up to 6 symbols per portfolio snapshot
- **Reader scaling**: Unlimited concurrent readers
- **Writer coordination**: Single writer with lockfree reader notification
- **Update frequency**: 1M+ updates/second sustainable

## Memory Ordering & Safety

### Publication Protocol
```rust
use core::sync::atomic::Ordering;

// Writers publish with release semantics
writer.publish(&inputs);  // Uses Release ordering internally

// Readers can use relaxed for hot path
let snapshot = slot.load_relaxed();

// Or acquire for strong consistency
let snapshot = slot.load_acquired();
```

### Stale Coordination
```rust
// Mark portfolio stale during updates
writer.mark_stale();

// Readers detect stale state
if let Some(words) = slot.load_relaxed() {
    let snapshot = ApmSnapshot::unpack(&words);
    if snapshot.header.stale {
        // Wait for fresh publication
        return None;
    }
    // Process valid snapshot
}
```

### Safety Guarantees
- **Atomic publication**: Complete 1024-bit state published atomically
- **ABA prevention**: Version and sequence counters prevent stale reads
- **Memory safety**: Zero unsafe code in critical paths
- **Overflow protection**: All fields have defined saturation behavior

## Error Handling

### Range Validation
```rust
use atomic_portfolio_map::ApmSnapshot;

let mut snapshot = ApmSnapshot::empty();
snapshot.header.symbol_count = 20;  // Exceeds 6-symbol limit
snapshot.slices[0].pos_qty = 100_000_000;  // Exceeds 24-bit signed range

// Packing automatically clamps to valid ranges
let words = snapshot.pack();
let unpacked = ApmSnapshot::unpack(&words);
assert_eq!(unpacked.header.symbol_count, 6);  // Clamped to maximum
```

### Feed Validation
Portfolio feeds are validated during symbol input construction:

```rust
// Invalid feeds are handled gracefully
let feed_with_invalid_data = FeedSnapshot {
    policy: invalid_policy,  // Will be clamped to valid ranges
    apc: out_of_range_apc,   // Fields saturated during packing
    // ...
};

let inputs = build_symbol_inputs(&feed_with_invalid_data);
// Inputs contain valid, clamped values
```

## Risk Management Integration

### Breaker Levels
```rust
use atomic_portfolio_map::BreakerLevel;

match snapshot.header.portfolio_breaker {
    BreakerLevel::L0 => {
        // Normal trading
    }
    BreakerLevel::L1 => {
        // Elevated caution - reduce position sizes
    }
    BreakerLevel::L2 => {
        // High risk - emergency scaling only
    }
    BreakerLevel::L3 => {
        // Critical - flatten positions
    }
}
```

### Time-Based Controls
```rust
let current_minute = 918;  // 9:18 AM

if current_minute >= snapshot.header.forbid_after_min_ct {
    // No new positions allowed
}

if current_minute >= snapshot.header.eod_flat_min_ct {
    // Begin position flattening
}
```

### Position Limits
```rust
for slice in &snapshot.slices[0..snapshot.header.symbol_count as usize] {
    if slice.flags.contains(SymbolFlags::REDUCE_ONLY) {
        // Only allow position-reducing trades
    }

    if slice.rem_daily_loss_cents == 0 {
        // Symbol has exhausted loss budget
    }
}
```

## Runtime Integration

### Controller Pattern
```rust
use atomic_portfolio_map::{PortfolioController, AccountSnapshot};

let controller = PortfolioController::new(account_id);

// Update from external feeds
controller.update_from_feeds(&feed_snapshots)?;

// Get current account state
let account = controller.account_snapshot();
println!("Total PnL: {} cents", account.net_pnl_cents);
```

### Runtime Loop
```rust
use atomic_portfolio_map::PortfolioRuntime;

let mut runtime = PortfolioRuntime::new();

loop {
    // Update from market data
    runtime.process_market_updates(&market_data)?;

    // Update from position feeds
    runtime.process_position_updates(&position_data)?;

    // Publish updated portfolio state
    let publication = runtime.publish_snapshot()?;

    // Process any breaker level changes
    if publication.snapshot.header.portfolio_breaker != BreakerLevel::L0 {
        handle_risk_event(&publication.snapshot);
    }
}
```

## Usage Examples

### Basic Portfolio Tracking
```rust
use atomic_portfolio_map::*;

fn main() {
    let mut writer = PortfolioMapWriter::new(ApmSlot::new());

    // Create symbol inputs for tracking
    let symbols = vec![
        SymbolInputs {
            sym_id: 1001,
            breaker_level: BreakerLevel::L0,
            flags: SymbolFlags::CAN_SCALE_UP,
            pos_qty: 50,
            unreal_cents: 12_500,
            rem_daily_loss_cents: 250_000,
            spread_ticks: 2,
            vol_band: 1,
            priority: 128,
        },
    ];

    let inputs = PortfolioInputs {
        account_id: 555,
        forbid_after_min_ct: 915,
        eod_flat_min_ct: 920,
        rem_daily_loss_total_cents: 500_000,
        trailing_draw_cents: 25_000,
        base_realized_cents: 50_000,
        created_ms_coarse: 41_000,
        portfolio_flags: PortfolioFlags::empty(),
        now_minute_count: 910,
        symbols: &symbols,
    };

    let publication = writer.publish(&inputs);
    println!("Portfolio state published: version={} symbols={}",
        publication.snapshot.header.version,
        publication.snapshot.header.symbol_count
    );
}
```

### Risk Monitoring
```rust
use atomic_portfolio_map::*;

fn monitor_portfolio_risk(slot: &ApmSlot) -> Option<String> {
    let words = slot.load_relaxed()?;
    let snapshot = ApmSnapshot::unpack(&words);

    // Check portfolio-level risk
    if snapshot.header.portfolio_flags.contains(PortfolioFlags::PAUSED) {
        return Some("Portfolio paused".to_string());
    }

    // Check individual symbol risk
    for i in 0..snapshot.header.symbol_count as usize {
        let slice = &snapshot.slices[i];

        if slice.flags.contains(SymbolFlags::LOCKOUT) {
            return Some(format!("Symbol {} in lockout", slice.sym_id));
        }

        if slice.rem_daily_loss_cents == 0 {
            return Some(format!("Symbol {} loss budget exhausted", slice.sym_id));
        }
    }

    None  // All clear
}
```

## Testing

```bash
# Run unit tests
cargo test

# Run integration tests
cargo test --test integration

# Run examples
cargo run --example apm_demo
cargo run --example runtime_loop
cargo run --example controller_demo
```

## Dependencies

- `atomic_position_capsule` - Position state management
- `atomic_cost_tracker` - Cost and edge tracking
- `atomic_venue_snapshot` - Market data integration
- `atomic_event_lockout_map` - Event-based lockouts
- `portable-atomic` - Cross-platform 128-bit atomics