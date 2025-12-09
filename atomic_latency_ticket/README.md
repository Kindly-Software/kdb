# ALT-128 — Atomic Latency Ticket

`atomic_latency_ticket` implements the ALT-128 capsule: a single-cache-line, 128-bit
health snapshot that gives breakers, strategies, and routers an atomic read on
path latency, jitter, reject/cancel pressure, packet loss, and queue position.

## Field Layout

All values saturate on overflow and pack into the following bit lanes:

```
[ f2d_us_q2_u12 | d2a_us_q2_u12 | a2f_us_q2_u12 | rej_rate_bp_u10
| cxl_rate_bp_u10 | loss_bp_u10 | jitter_us_q2_u12 | qpos_q012_u12
| flags_u8 | ver_u8 | seq_u10 | age_8ms_u12 ]
```

* Latency lanes (`*_us_q2_u12`) store microseconds in 0.5 µs units (max ≈ 8.19 ms).
* Rate lanes hold basis points over a sliding window.
* Queue position is encoded as Q0.12 (0.0–1.0 percentile).
* Age tracks milliseconds / 8 up to ~32.7 seconds.

## Writer Side

```rust
use atomic_latency_ticket::{AltAtomic, AltSample, FLAG_SLOW_ACK};
use core::time::Duration;

let alt = AltAtomic::new();
let sample = AltSample {
    feed_to_decision_us: 420,
    decision_to_ack_us: 3_200,
    ack_to_first_fill_us: 42_000,
    reject_rate_bps: 180,
    cancel_rate_bps: 950,
    loss_rate_bps: 12,
    jitter_us: 5_800,
    queue_position: 0.58,
    flags: FLAG_SLOW_ACK,
    version: 1,
    sequence: 77,
    age_ms: 240,
};
alt.publish_sample(sample); // single store(Release)
```

Hook the publisher into order send / ack / fill events and a short periodic task
(250–500 ms) to refresh feed latency, loss, queues, and age ticks.

### Latency sidecar helper

`AltWriter` keeps the EWMA state, sliding rejection/cancel window, and dynamic
latency budgets for you. Feed it router/strategy hooks and call `publish()` on
your heartbeat:

```rust
use atomic_latency_ticket::{AltSlot, AltWriter, AltWriterConfig};

let slot = AltSlot::new();
let mut writer = AltWriter::from_slot(&slot, AltWriterConfig::default());

// during your loop
writer.record_feed_to_decision(feed_to_decision_us);
writer.on_order_send(now_ns);
writer.on_order_ack(now_ack_ns);
writer.on_first_fill(now_fill_ns);
writer.on_order_reject();
writer.update_queue_position(queue_percentile);
writer.record_loss_bps(loss_probe_bps);
let snapshot = writer.publish(now_tick_ns);
```

`snapshot` gives you a decoded view for metrics or logging, while the atomic
lane is updated for readers with a single release-store.

## Reader Side

```rust
let snapshot = alt.load_relaxed().snapshot();
if snapshot.is_stale(2_000) {
    // wind down risk — ticket is stale
}
if snapshot.flags & FLAG_SLOW_ACK != 0 || snapshot.decision_to_ack_us > budget_ms {
    breaker.trip_l1();
}
```

Readers perform one `load(Relaxed)` and branch on quantized fields; no pointer
chasing or acquire fences are required unless follow-up state is protected by the
same ticket.

## Flags

`FLAG_SLOW_ACK | FLAG_SLOW_FILL | FLAG_HIGH_JITTER | FLAG_HIGH_LOSS |
FLAG_RATE_LIMIT | FLAG_BACKPRESSURE | FLAG_GATE_CANCEL | FLAG_SPARE`

Each flag gives the router/breaker a constant-time hint to degrade, switch to
taker, or pause.

## Testing

Unit tests cover packing/unpacking, quantization saturation, and atomic publish
semantics. Property tests ensure the layout stays bijective for in-range values.
Run the suite with:

```
cargo test
```

## License

Licensed under either of

* Apache License, Version 2.0
* MIT license

at your option.
