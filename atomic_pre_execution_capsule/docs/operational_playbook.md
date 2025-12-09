# PEX-1024 Operational Playbook

This document captures the steps required to roll PEX-1024 to production and
keep it healthy in day-to-day operations.

## 1. Staging cadence

- Writers should refresh the capsule on regime changes or every 50–150 ms.
- Use `PexWriter`'s reusable draft and let it manage sequence/version flips.
- While a play is firing, either set `stale` immediately or publish a follow-up
  draft with the lane disabled so routers do not re-trigger.

## 2. Router integration

- Poll snapshots with `PexRouter::for_each_play` (or `poll_snapshot` if the
  caller needs custom iteration). Only even versions with matching head/tail
  survive the accept rule.
- Sort plays by `priority` (already handled by `PexRouter`) and verify trigger
  masks against the precomputed bitset before firing.
- When breakers or lockouts toggle, prefer re-publishing to reflect the new
  eligible play mask instead of mutating at the router side.

## 3. Telemetry

- Capture `PexWriter::stats().publishes` and `.stale_marks` to monitor staging
  cadence; unexpected stalls should raise alerts.
- From the router, export `RouterStats::polls`, `accepted_snapshots`, and
  `plays_considered` to confirm the router observes every publish and is not
  spinning on stale snapshots.
- Track downstream latency from publish to route decision; if the tail defaults
  (`slip_cap_default_bp`, `lat_budget_default_us`) are tightened, ensure the
  metrics stay within band.

## 4. Safety rails

- Guard production rollout with a feature flag: fall back to the existing
  AEB-512 pipeline when `PexRouter` stops accepting snapshots.
- On critical errors, have the writer flip `stale` (Release store) so routers
  instantly ignore the capsule while the writer refreshes state.
- Keep bracket and route templates immutable per publish; if templates need
  structural changes, publish a stale capsule, update templates, and then
  publish a fresh draft to avoid mixed reader views.

## 5. Testing checklist

- Run `cargo test` (unit + property + concurrency) before every deploy.
- Run `cargo bench --bench hot_loop` on the target hardware to confirm publish
  and snapshot latency stay within the 80–120 ns router budget.
- Replay historical order-book data against the pipeline (see
  `examples/runtime_loop.rs` as a starting point) and verify only the intended
  plays fire under each trigger mask.

## 6. Incident response

- If routers start skipping snapshots, inspect the head/tail mismatch counters
  and checksum validity; a mismatch indicates a writer bug or memory corruption.
- If plays over-fire, audit trigger masks and TTLs in the published snapshot (use
  `PexSnapshot` views) before adjusting router logic.
- Keep a runbook entry documenting how to disable individual lanes via
  `play_mask_override` for emergency throttling.
