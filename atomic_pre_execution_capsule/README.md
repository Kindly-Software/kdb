# atomic_pre_execution_capsule

`atomic_pre_execution_capsule` implements the PEX-1024 pre-executed playbook
capsule. The capsule stores four fully staged plays, shared bracket/route
templates, and router defaults inside a single 1024-bit structure so routing
threads can decide and fire orders with one cache-line read.

## Layout

The capsule is eight contiguous 128-bit words (`W0..W7`) aligned to 64 bytes.
`W1..W4` hold the per-play specs, `W5` stores two bracket templates, `W6`
provides four route templates, and `W7` carries tail integrity fields along with
shared defaults. `W0` contains the publish header and is the only word written
with `Release` ordering.

```
W0: commit | stale | even version | seq head | account | created | ttl | forbid | eod
    | play mask | global flags | breaker level | symbol count | spare
W1..W4: play lane (enable, side, anchor, ord type, tif, symbol, qty, price
        offset, route/bracket template ids, slip/lat/ttl/prio, trigger mask)
W5: two 54-bit bracket templates + reserved tail
W6: four 25-bit route templates + reserved tail
W7: checksum16 | version tail | seq tail | slip default | latency default
    | router hints | spare
```

## Writer workflow

1. Assemble `PexDraft` with the four plays, template lanes, and defaults.
2. Call `PexCapsule::publish`, which encodes W1..W7, computes the tail checksum,
   and flips W0 with `store(Ordering::Release)`.
3. To invalidate a snapshot prior to refresh, call `PexCapsule::mark_stale()`.

## Reader workflow

Routers call `PexCapsule::load_snapshot` which enforces the accept rule:

- `commit == 1`, `stale == 0`, and `ver_even` is even.
- Tail checksum matches W1..W6, `ver_even == ver_tail`, and `seq_head == seq_tail`.
- A re-read of `W0` confirms the header did not change during the sample.

The returned `PexSnapshot` exposes structured views for the header, plays,
brackets, routes, and tail defaults.

## Tests

The test suite covers:

- Bit packing for the Topstep-style default playbook described in the build
  plan.
- Publish/read round-tripping under the accept rule.
- Immediate rejection when the capsule is marked stale.

Run the tests from this crate directory with `cargo test`.

## Pipeline helpers

`pipeline::PexWriter` owns a reusable `PexDraft`, advances sequence/version
counters, and keeps lightweight publish metrics. `pipeline::PexRouter` dedups
snapshots, iterates plays in priority order, and tracks poll/play counters. See
`examples/runtime_loop.rs` for an end-to-end loop that stages the default
Topstep playbook, replays trigger frames, and prints router decisions.

## Validation suite

- Property tests (`tests/property_roundtrip.rs`) fuzz the packing logic across
  the allowed bit ranges and assert that decoded snapshots match the drafted
  inputs.
- `tests/concurrency.rs` stress the publish/load protocol with cross-thread
  loops to ensure sequences never regress and snapshots stay coherent under
  contention.
- Unit tests cover layout invariants plus writer/router helpers; benches capture
  publish and snapshot latency (see `cargo bench --bench hot_loop`).

## Operational playbook

`docs/operational_playbook.md` summarizes rollout steps: staging cadence,
router integration, telemetry signals, and guardrails when flipping `stale` or
falling back to AEB-512 capsules.
