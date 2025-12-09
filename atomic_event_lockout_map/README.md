# ECO-1024 — Event Lockout Map

`atomic_event_lockout_map` implements the ECO-1024 capsule: an eight-word,
1024-bit atomic snapshot that answers "Is this minute safe to trade?" while
reporting the active event windows, breaker guidance, and session clamps in a
single relaxed load.

The capsule is laid out as eight successive 128-bit words (W0..W7) with
`#[repr(C, align(64))]` so writers can stage a snapshot, flip the commit bit with
one release store, and let readers hydrate an up-to-date view without locks or
pointer chasing.

## What it Publishes

* **Minute bitmap (512 bits)** anchored by `origin_min_ct`, covering the target
  trading window (RTH + buffer by default). Bit = 1 means "allowed to open".
* **Event slots (8 × 32 bits)** describing CPI/FOMC/news/maintenance windows with
  severity and recommended breaker actions (L1-L3).
* **Session clamps & metadata** including `forbid_after` / `eod_flat`, account
  routing metadata, and coarse creation time.
* **Integrity tail** with checksum, head/tail version match, "what's next"
  lockout/resume hints, and active action/severity summations.

## Writer Path

The writer (`EcoWriter`) merges session rules, recurring baseline windows,
manual pauses, and up to 8 event lockouts into a snapshot:

```rust
use atomic_event_lockout_map::{
    AccountScope, BuildRequest, EventAction, EventKind, EventSeverity, EventWindow, EcoWriter,
    GlobalFlag,
};

let mut writer = EcoWriter::new(AccountScope::new(17, 1))
    .with_origin_minute(480)
    .with_mask_length(512)
    .with_session_clamps(905, 910)
    .with_baseline_window(510, 905); // 08:30→15:05 CT

let events = [
    EventWindow::new(450, 457, EventSeverity::High, EventAction::ForbidNew, 0, EventKind::Other),
    EventWindow::econ(780, 795, EventSeverity::Medium, EventAction::Degrade),
];

let draft = writer.build(BuildRequest {
    now_min_ct: 531,
    age_8ms: 120,
    created_ms_coarse: 12_345,
    events: &events,
    global_flags: GlobalFlag::empty(),
    manual_pause: false,
    day_of_week: 2,
    holiday_flag: false,
});

writer.slot().publish(&draft);
```

The builder stages W1..W7, computes a rolling checksum, copies the odd tail
version into W7, and releases W0 with the even version. Manual pauses can zero
out the bitmap (`GlobalFlag::PAUSED`) without clearing stored event layouts.

### Feed Publisher

`EcoPublisher` injects live feed inputs and yields flag transitions for
telemetry or logging. The property suite includes Proptest cases for overlapping
events, baseline coverage, and pause transitions so regressions surface quickly:

```rust
use atomic_event_lockout_map::{
    AccountScope, EcoPublisher, EventAction, EventSeverity, EventWindow, FlagDiff, MinuteRange,
    PublishOutcome, PublisherConfig, SessionClamps, SnapshotInputs,
};

let mut publisher = EcoPublisher::new(PublisherConfig::new(
    AccountScope::new(17, 1),
    480,
    512,
    SessionClamps::new(Some(905), Some(910)),
));

let baseline = [MinuteRange::new(480, 905)];
let events = [EventWindow::econ(780, 795, EventSeverity::Medium, EventAction::Degrade)];

let PublishOutcome { snapshot, flag_diff } = publisher.publish(SnapshotInputs {
    baseline_windows: &baseline,
    events: &events,
    now_min_ct: 531,
    age_8ms: 64,
    created_ms_coarse: 12_345,
    global_flags: GlobalFlag::empty(),
    manual_pause: false,
    session_clamps: SessionClamps::new(Some(905), Some(910)),
    day_of_week: 2,
    holiday_flag: false,
});

if flag_diff.has_changes() {
    println!("ECO flags changed: set={:?} cleared={:?}", flag_diff.set, flag_diff.cleared);
}
```

## Reader Path

```rust
if let Some(snapshot) = slot.load_relaxed() {
    if snapshot.is_allowed_now() {
        // proceed; minute bit == 1 and we're before session clamps
    }

    if snapshot.active_action().at_least(EventAction::ForbidNew) {
        // route reduce-only, or escalate breaker according to L2/L3 policy
    }
}
```

Readers perform a single snapshot pass: load W0, early-exit on odd / stale
versions, load W7, validate head/tail versions and checksum, then decode any
bitmap bit or event window they care about.

## Tests

The crate ships with unit tests for packing, event saturation, bitmap math, and
next-lockout / resume derivation. Property tests (`proptest`) validate flag
deltas, mask coverage, and pause transitions. Run the suite with:

```
cargo test
```

## License

Licensed under either of

* Apache License, Version 2.0
* MIT license

at your option.
