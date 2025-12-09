## SCANHASH (Key Hash Scan)

Purpose: Provide a bounded, lockfree key listing without storing full keys in slots. Returns up to N key hashes (hex) for non-expired slots; duplicates possible when keys are overwritten.

Command:
- `SCANHASH [count]` → RESP bulk string body with newline-separated hex hashes.

Implementation:
- Uses `LockfreeCacheCapsule::scan_hashes(limit)` to walk slots and collect key hashes that are non-empty and unexpired.
- Sharded mode distributes the limit across shards and truncates to the requested count.

Notes:
- Approximate: may include overwritten keys; does not guarantee full coverage if count < total keys.
- Keys themselves are not stored; hashes are for observability/debugging.
- Pairs well with `STATS`/`SLOWLOG` for ops insight.
