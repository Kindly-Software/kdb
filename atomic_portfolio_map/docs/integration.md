# Integrating APM-1024 in a Live Runtime

This note sketches the pieces you need to wire when promoting the demo harnesses
into production. The goal is a single writer thread that publishes
`FeedSnapshot` objects into `PortfolioController`, while any number of readers
consume `PortfolioRuntime`'s slot with relaxed loads.

## Data sources and adapters

1. **Position feed (APC-512)** – implement [`feed::ApcFeed`] so the runtime can
   request the latest `ApcSnapshot` for each configured symbol. The helper
   [`SharedApcFeed`](crate::SharedApcFeed) provides a thread-safe cache you can
   populate from your existing APC plumbing.
2. **Edge surplus (ACT-128)** – implement [`feed::ActFeed`] to expose
   `ActEdge { edge_surplus_bp }`. [`SharedActFeed`](crate::SharedActFeed) mirrors
   the pattern and gives you a place to publish ACT surpluses.
3. **Venue state (AVS-128)** – implement [`feed::AvsFeed`] to surface spread and
   volatility band metadata. [`SharedAvsFeed`](crate::SharedAvsFeed) makes this
   optional feed easy to wire up; omit it while bootstrapping if necessary.
4. **Policy & gating** – populate [`SymbolPolicy`] from static configuration and
   derive [`SymbolGates`] (news lockouts, eco breakers, manual overrides) inside
   your session controller. `SymbolGates::from_eco` converts an
   [`atomic_event_lockout_map::EcoSnapshot`] into all four booleans so you can
   merge ECO, manual, and supervisory overrides in one place. The helper
   [`FeedAssembler`] converts these into a ready-to-pack [`FeedSnapshot`].

## Publishing flow

1. Instantiate `PortfolioMapWriter` with a dedicated `ApmSlot` aligned and
   shared between the writer thread and readers.
2. Create a `PortfolioController` with your stale timeout. The controller
   handles even/odd version choreography and exposes `tick()` to mark stale when
   the timeout elapses.
3. Wrap the feeds and controller inside `PortfolioRuntime`:

```rust
let assembler = FeedAssembler { apc: &apc_feed, act: Some(&act_feed), avs: Some(&avs_feed) };
let controller = PortfolioController::new(writer, Duration::from_millis(2_000));
let mut runtime = PortfolioRuntime::new(controller, assembler, policies);
```

See `examples/runtime_loop.rs` for a complete mock loop that pushes updates
into the shared feeds, publishes snapshots, and drives the stale timer.

4. On each publish cadence (e.g., driven by APC ticks or a 50 ms timer):
   - Pull the latest ECO snapshot (if available) into a `SymbolGates` via
     `SymbolGates::from_eco`, then overlay manual and route-specific overrides.
     The helper [`SharedEcoFeed`](crate::SharedEcoFeed) mirrors the other feed caches, and
     [`PortfolioRuntime::publish_cycle_with_eco`](crate::runtime::PortfolioRuntime::publish_cycle_with_eco)
     wires the conversion automatically when supplied with an `EcoFeed`.
   - Call `runtime.publish_cycle` (or `publish_cycle_with_eco`) with the account snapshot. If any
     mandatory feed is missing, the method returns `None`; keep the last good snapshot until
     data resumes.
5. Drive `runtime.tick(now_ms)` from a monotonic clock at regular intervals so
   stale snapshots are invalidated automatically when data pauses.

## Reader contract

Readers touch only the `ApmSlot` exposed by the controller or runtime. They
should call `load_relaxed()` (or `load_snapshot_relaxed()`) and honour the
commit/stale semantics:

- If `None`, either the feeds are stale or a new publish is in progress.
- When `Some`, the packed words already contain all per-symbol gates, priority
  scores, and portfolio totals.

## Operational notes

- Version numbers advance in even increments; `mark_stale()` flips the header to
  an odd version with `commit = 0` so readers instantly drop the snapshot.
- Always publish policies in deterministic order so the runtime never reallocates
  its symbol buffer on the hot path.
- The assembler expects APC to be present for every policy. If your backend can
  drop APC updates, guard the publish with a recentness check and skip the
  publish cycle instead of sending partial data.

With these hooks in place, the runtime harness mirrors the behaviour in
`examples/controller_demo.rs`, but with live feeds replacing the mock sources.

## Direct capsule/slot adapters

For most teams the simplest integration path is to register live capsules or slots with the
provided adapters:

```rust
use atomic_portfolio_map::{
    adapters::{ActSlotFeed, CapsuleApcFeed}, SharedAvsFeed,
    FeedAssembler,
};

let apc_capsules = CapsuleApcFeed::new();
apc_capsules.register(sym_id, apc_arc.clone());

let act_slots = ActSlotFeed::new();
act_slots.register(sym_id, act_slot_arc.clone());

let avs_feed = SharedAvsFeed::new();
let assembler = FeedAssembler::new(
    Arc::new(apc_capsules),
    Some(Arc::new(act_slots)),
    Some(Arc::new(avs_feed)),
);
```

`CapsuleApcFeed` converts `atomic_position_capsule::Snapshot` values into the lightweight
`ApcSnapshot` used by APM, while `ActSlotFeed` derives the edge surplus directly from
`atomic_cost_tracker::ActSlot`. Register each symbol once and continue updating the upstream
capsule/slot as usual; the runtime calls `load()`/`load_acquire()` under the hood on every
publish cycle.
