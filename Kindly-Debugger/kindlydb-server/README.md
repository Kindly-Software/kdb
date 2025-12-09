# KindlyDB Server

HTTP REST API server for user and audit storage, designed for kdb-signup integration.

## Architecture

- **Tier**: T1 Atomic (UCE34/Chaos compliant)
- **Storage**: In-memory with lockfree coordination
- **Metrics**: Prometheus-compatible `/metrics` endpoint
- **Port**: 8083 (configurable via `PORT` env var)

## Chaos Compliance

- 100% lockfree ID generation (`AtomicU64`)
- Generation counters for TOCTOU prevention
- No mutex in hot path (RwLock only for cold storage operations)
- Cache-aligned atomic counters

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check with metrics |
| GET | `/metrics` | Prometheus metrics |
| POST | `/users` | Create user (returns 201 with id, 409 if exists) |
| GET | `/users?email_hash={hash}` | Find user by email hash |
| GET | `/users/{id}` | Get user by ID |
| PUT | `/users/{id}` | Update user |
| POST | `/audit` | Log audit entry |
| GET | `/audit?user_id={id}` | Get audit trail for user |

## Deployment (kindly-hub)

The server is deployed at `kindly-hub:8083`:

```bash
# Check status
ssh samuel@kindly-hub "sudo systemctl status kindlydb-server"

# View logs
ssh samuel@kindly-hub "sudo journalctl -u kindlydb-server -f"

# Restart
ssh samuel@kindly-hub "sudo systemctl restart kindlydb-server"
```

## Build

```bash
cd /home/samuel/Primitives/Kindly-Debugger/kindlydb-server
RUSTFLAGS="-C target-cpu=znver3" cargo build --release
```

Binary: `target/release/kindlydb-server` (3.8MB optimized)

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | 8083 | HTTP port |
| `RUST_LOG` | `kindlydb_server=info` | Log level |

## SystemD Service

Located at `/etc/systemd/system/kindlydb-server.service` on kindly-hub.

Configuration:
- User: samuel
- WorkingDirectory: /opt/kindlydb
- Port: 8083 (via Environment directive)

## Metrics

Prometheus metrics exposed at `/metrics`:

```
kindlydb_users_total      # Total users created (counter)
kindlydb_audit_entries_total  # Total audit entries (counter)
kindlydb_uptime_seconds   # Server uptime (gauge)
```

## Integration with kdb-signup

kdb-signup connects to KindlyDB via the `KINDLYDB_URL` environment variable:

```bash
# In /opt/kdb-signup/kdb-signup.env
KINDLYDB_URL=http://localhost:8083
```

Features enabled:
- **Analytics**: Track who signed up
- **Duplicate prevention**: Reject same email twice (409 Conflict)
- **Pending token survival**: Tokens persist across restarts
- **Q34 audit trail**: Hash-chained audit entries

## License

Proprietary - Kindly Software
