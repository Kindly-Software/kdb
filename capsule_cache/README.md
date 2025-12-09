# Capsule Cache

Redis-style cache built with the computational capsule architecture (UCE34 + Chaos). Lockfree, cache-line aligned, generation-tagged; no mutex/RwLock anywhere.

## Capsules Used
- `LockfreeCacheCapsule` (T6 Mixed: atomic map + Q16.16 TTL).
- `StatsCapsule64` (T1 Atomic) for hits/misses/latency.
- `HistogramCapsule` (T1) for latency percentiles (per shard + aggregated).
- `RingBufferCapsule` (T5 Streaming) for slowlog (bounded, lockfree).
- Optional security capsules via feature flags: HMAC integrity (`integrity`), multi-tenant isolation (`multi-tenant`), AES-256-GCM (`encryption`).
- Optional distributed layer (T8): consistent hashing + batching via `distributed-*` flags.

## Features
- `std` (default): required for hashing/time.
- `integrity`, `multi-tenant`, `encryption`, `security-full`: forward to atomic_capsule cache security.
- `distributed`, `distributed-compression`, `distributed-audit`, `distributed-histogram`, `distributed-all`: forward to distributed cache primitives.

## Quick Start (local)
```rust
use capsule_cache::CapsuleCache;
use std::time::Duration;

let cache = CapsuleCache::<String>::new();
cache.insert("k".into(), "v".into(), Duration::from_secs(60)).unwrap();
assert_eq!(cache.get(&"k".into()), Some("v".into()));
cache.remove(&"k".into());
```

### Command Server (Redis-style)
```
cargo run -p capsule_cache --bin server -- 127.0.0.1:7379
# In another shell:
printf "SET foo 60 bar\nGET foo\nTTL foo\nDEL foo\n" | nc 127.0.0.1 7379
```
- RESP array support: `printf '*2\r\n$4\r\nPING\r\n$4\r\nPONG\r\n' | nc ...`
- Auth: set `AUTH_TOKEN=secret` (then `AUTH secret` first).
- Sharding: `SHARDS=4 SHARD_CAPACITY=4096` to modulo-distribute keys across shards.
- Rate limit: 2000 ops/sec per connection (drop with `-ERR rate limit`).
- Extended verbs: `MSET k1 60 v1 k2 60 v2`, `MGET k1 k2`, `EXPIRE k1 30`, `INCR count`, `STATS`, `SLOWLOG LEN|RESET|[n]`, `FLUSHDB`.
- Observability: `STATS` reports hits/misses/sets/dels/errors + aggregated p50/p95/p99/p999 latency (ns), histogram counts/overflow, and slowlog counters/export path. `SLOWLOG [n]` dumps recent slow entries (seq/op/key_hash/duration); `SLOWLOG LEN`/`RESET` manage the window.
- Slowlog: `SLOWLOG_US=5000` (default 5ms) controls threshold; bounded ring (16K entries). Optional export: set `SLOWLOG_PATH=/tmp/slowlog.tsv` for background flush (no tokio).
- Admin: `FLUSHDB` clears all slots (lockfree). KEYS/SCAN are intentionally omitted (keys are not stored alongside slots).
- KEYS-lite: `SCANHASH [count]` returns up to `count` key hashes (hex) for non-expired slots (approximate, may include overwrites).
- Distributed (in-process): `DistributedCache::new(nodes, shards_per_node, cap_per_shard, replication_factor)` provides quorum replication without external deps; routes by consistent hash and replicates writes to `replication_factor` nodes.

### Append-Only Persistence
- Set `AOF_PATH=/tmp/capsule-cache.aof` to replay on startup and append SET/DEL mutations.
- Format: length-prefixed (binary-safe) headers: `SET <expiry_ms> <klen> <vlen>\n` + bytes + newline; `DEL <klen>\n` + bytes + newline.

## UCE34 / Chaos Guardrails
- 100% lockfree (SWeMR single-writer/many-readers, commit-flip generations).
- Cache-line alignment: slots are padded to prevent false sharing.
- Deterministic TTL via Q16.16 fixed-point math (no FP drift).
- Verification: rely on `atomic_capsule` `#[derive(ComputationalCapsule)]` and clippy capsule lints (P0-P1).

## Next Steps
1) Observability polish: shard-aware histograms in `STATS`, optional async log capsule for slow commands.
2) Distributed mode behind `distributed-*`: consistent hashing + quorum write/read, keep lockfree.
3) Benchmarks + systemd smoke: <120ns hit / <220ns insert targets, bounded `evict_expired`.

## Benchmarks
- Quick microbench (nightly): `cargo bench -p capsule_cache --example micro_bench`
- Or run inline: `cargo run -p capsule_cache --example micro_bench` (prints avg ns/op for hit/insert).

## Smoke
- `scripts/capsule_cache_smoke.sh` runs PING/SET/GET/TTL/STATS/SLOWLOG/SCANHASH/FLUSHDB against a local server and tears it down.

## Systemd Template
- `scripts/capsule_cache.service` provides a minimal systemd unit. Env vars: `AUTH_TOKEN`, `AOF_PATH` (default `/var/lib/capsule_cache/aof.log`), `SLOWLOG_PATH` (default `/var/log/capsule_cache/slowlog.tsv`), `SHARDS`, `SHARD_CAPACITY`. Adjust `User/Group/WorkingDirectory` to your host.
