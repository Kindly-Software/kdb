# Atomic Ledger Entry (ALE-128)

`atomic_ledger_entry` implements the ALE-128 tamper-evident audit line format: a
single 16-byte record that chains trading events with a keyed hash. Writers can
append in constant time, while readers verify integrity with a single forward
scan.

## Highlights

- Bit-exact packing for the 64-bit metadata word (`AleMeta`) with helpers for the
  documented event taxonomy.
- Keyed chaining via BLAKE3 (`chain_prev_hash` / `derive_genesis_hash`) with
  transparent `AleEntry` accessors for readers.
- Single-writer ring (`AleRing` + `Writer`) with sequence tracking and
  validators that catch tampering or gaps on the first mismatch.
- Optional streaming runtime (`LedgerStreamBuilder`, feature `stream`) that
  spawns a drain thread, exposes lock-free producer handles, and keeps stats on
  appended vs. rejected events.
- CLI verifier (`alechk`, feature `cli`) that reads raw or hex ledgers, derives
  the genesis hash, and reports the first chain or sequence violation.

## Quick Start

```rust
use atomic_ledger_entry::{
    AleEvent, AleKey, LedgerStreamBuilder, Route2, StreamStats,
};

let key = AleKey::new(*b"example-key-material-000000000000");
let stream = LedgerStreamBuilder::new(key.clone())
    .ring_capacity(1024)
    .queue_capacity(4096)
    .genesis_context(b"ALE|2024-07-24|acct-42")
    .spawn()
    .expect("ledger stream");
let producer = stream.producer();
producer
    .enqueue_blocking(AleEvent::order_sent(
        1_723_000_001_000_000_000,
        1,
        77,
        Route2::Maker,
        25,
    ))
    .expect("queued");
let stats: StreamStats = stream.shutdown().expect("writer joined");
assert_eq!(stats.meta_errors, 0);
```

Enable the CLI and stream runtime with `cargo install --features "cli"` or
selective `--features stream` when embedding the runtime.
