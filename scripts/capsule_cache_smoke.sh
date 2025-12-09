#!/usr/bin/env bash
set -euo pipefail

# Simple smoke test for capsule-cache server (no external deps).
# Commands: PING, SET/GET/TTL, STATS, SLOWLOG LEN, SCANHASH, FLUSHDB.

ADDR=${1:-127.0.0.1:7379}
BIN="cargo run -p capsule_cache --bin server --"

echo "[smoke] starting server at $ADDR"
$BIN "$ADDR" > /tmp/capsule_cache_smoke.log 2>&1 &
SERVER_PID=$!
trap 'kill $SERVER_PID >/dev/null 2>&1 || true' EXIT

sleep 1

cmd() {
  printf "%s\n" "$1" | nc -w 1 ${ADDR/:/ } || true
}

echo "[smoke] PING"
cmd "PING"

echo "[smoke] SET/GET/TTL"
cmd "SET foo 5 bar"
cmd "GET foo"
cmd "TTL foo"

echo "[smoke] STATS"
cmd "STATS"

echo "[smoke] SLOWLOG LEN"
cmd "SLOWLOG LEN"

echo "[smoke] SCANHASH 5"
cmd "SCANHASH 5"

echo "[smoke] FLUSHDB"
cmd "FLUSHDB"

echo "[smoke] stopping"
kill $SERVER_PID >/dev/null 2>&1 || true
wait $SERVER_PID 2>/dev/null || true

echo "[smoke] done (logs: /tmp/capsule_cache_smoke.log)"
