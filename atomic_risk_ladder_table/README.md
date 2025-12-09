# Atomic Risk Ladder Table (RLT-1024)

`atomic_risk_ladder_table` implements the RLT-1024 policy capsule described in the Risk Ladder Table
build plan. The capsule exposes an 8×128-bit packing that allows breakers, strategies, and routers to
consume a single atomic snapshot containing:

- Global policy metadata (`W0`) with recover scaling and dwell defaults.
- Per-strategy trip thresholds (`W1`, `W3`, `W5`) across ALT/REJ/LOSS/VOL axes.
- Per-strategy action bundles (`W2`, `W4`, `W6`) for levels `L0→L3` (size/slip/latency/route/dwell).
- Integrity tail (`W7`) with checksum, sequence mirroring, and routing hints.

The crate focuses on deterministic bit packing, explicit fixed-point helpers, and publish-side
validation that mirrors the odd→even commit contract. Everything is memory-ordering agnostic so it can
be combined with external atomic writers/readers tailored to your control plane.

## Highlights

- `#[repr(C, align(64))]` layout to avoid false sharing or torn reads.
- Fixed-point helpers for `Q1.7` / `Q2.6` / `Q4.8` encodings.
- Safe, `no_std` compatible API with optional `serde` support (add the `serde` feature).
- Snapshot validator that checks version/sequence coherence and recomputes the tail checksum.
- Property tests for hysteresis math and round-trip encoding.

## Quick Start

```rust
use atomic_risk_ladder_table::{layout::actions::ActionsWordDraft, layout::trips::TripThresholds, Rlt1024};

let mut table = Rlt1024::new();
let mut header = table.header;
header.set_version_even(2);
header.set_seq_head(1);
header.set_recover_scale(atomic_risk_ladder_table::DEFAULT_RECOVER_SCALE_Q1_7);
header.set_strategy_mask(atomic_risk_ladder_table::layout::header::StrategyMask::new(0b111));
table.header = header;

table.strat_a_trips.set_thresholds(TripThresholds::DEFAULT);
table.strat_a_actions.apply_draft(ActionsWordDraft::DEFAULT);

let checksum = table.checksum16();
let mut tail = table.tail;
tail.set_version(table.header.version_even());
tail.set_seq_tail(table.header.seq_head());
tail.set_checksum(checksum);
table.tail = tail;
```

From here the writer can publish `W1→W7` and finally flip `W0` with a release store.
