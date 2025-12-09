//! Redis-style command server with capsule architecture (lockfree, zero external deps).
//! Supported commands (inline or RESP array): PING, AUTH <token>, SET <key> <ttl_sec> <val>,
//! GET <key>, DEL <key>, TTL <key>, MSET <k ttl v>..., MGET <k>..., EXPIRE <key> <ttl_sec>,
//! INCR <key>, STATS, SLOWLOG LEN|RESET|[n], FLUSHDB. Persistence via AOF: set `AOF_PATH=/tmp/cache.aof`.

use capsule_cache::{
    persistence,
    sharded::ShardedCache,
    slowlog::{hash_key, SlowLog, SlowOp},
    CapsuleCache,
};
use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
enum Backend {
    Single(Arc<CapsuleCache<String>>),
    Sharded(Arc<ShardedCache<String>>),
}

impl Backend {
    fn insert(&self, key: String, value: String, ttl: Duration) -> Result<(), &'static str> {
        match self {
            Backend::Single(c) => c.insert(key, value, ttl).map_err(|_| "ERR insert failed"),
            Backend::Sharded(s) => s.insert(key, value, ttl).map_err(|_| "ERR insert failed"),
        }
    }

    fn get(&self, key: &str) -> Option<String> {
        match self {
            Backend::Single(c) => c.get(&key.to_string()),
            Backend::Sharded(s) => s.get(&key.to_string()),
        }
    }

    fn remove(&self, key: &str) -> bool {
        match self {
            Backend::Single(c) => c.remove(&key.to_string()).is_some(),
            Backend::Sharded(s) => s.remove(&key.to_string()).is_some(),
        }
    }

    fn ttl(&self, key: &str) -> Option<Duration> {
        match self {
            Backend::Single(c) => c.ttl_remaining(&key.to_string()),
            Backend::Sharded(s) => s.ttl_remaining(&key.to_string()),
        }
    }

    fn expire(&self, key: &str, ttl: Duration) -> bool {
        match self {
            Backend::Single(c) => c.expire(&key.to_string(), ttl),
            Backend::Sharded(s) => s.expire(&key.to_string(), ttl),
        }
    }

    fn incr(&self, key: &str, delta: i64) -> Result<i64, &'static str> {
        match self {
            Backend::Single(c) => c.incr(&key.to_string(), delta),
            Backend::Sharded(s) => s.incr(&key.to_string(), delta),
        }
    }

    fn evict_expired(&self) -> usize {
        match self {
            Backend::Single(c) => c.evict_expired(),
            Backend::Sharded(s) => s.evict_expired(),
        }
    }

    fn stats_summary(
        &self,
    ) -> (
        atomic_capsule::collections::StatsSnapshot,
        (Option<u64>, Option<u64>, Option<u64>, Option<u64>),
    ) {
        match self {
            Backend::Single(c) => c.stats(),
            Backend::Sharded(s) => s.stats(),
        }
    }

    fn clear_all(&self) -> usize {
        match self {
            Backend::Single(c) => c.clear_all(),
            Backend::Sharded(s) => s.clear_all(),
        }
    }

    fn scan_hashes(&self, limit: usize) -> Vec<u64> {
        match self {
            Backend::Single(c) => c.scan_hashes(limit),
            Backend::Sharded(s) => s.scan_hashes(limit),
        }
    }
}

fn main() -> io::Result<()> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:7379".to_string());
    let aof_path = env::var("AOF_PATH").ok();
    let auth_token = env::var("AUTH_TOKEN").ok();
    let shard_count: usize = env::var("SHARDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
        .max(1);
    let shard_capacity: usize = env::var("SHARD_CAPACITY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(16_384);

    let slowlog_threshold_ns = env::var("SLOWLOG_US")
        .ok()
        .and_then(|s| s.parse().ok())
        .map(|us: u64| us * 1_000)
        .unwrap_or(5_000_000); // 5ms default
    let slowlog_path = env::var("SLOWLOG_PATH").ok();
    let slowlog = match SlowLog::with_export(slowlog_threshold_ns, slowlog_path.as_deref()) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("failed to init slowlog export, continuing in-memory only: {e}");
            Arc::new(SlowLog::new(slowlog_threshold_ns))
        }
    };

    let backend = if shard_count > 1 {
        Backend::Sharded(Arc::new(ShardedCache::new(
            shard_count,
            shard_capacity,
        )))
    } else {
        Backend::Single(Arc::new(CapsuleCache::<String>::new()))
    };

    // Best-effort AOF load (single-threaded, no locks).
    if let Some(ref path) = aof_path {
        match &backend {
            Backend::Single(c) => {
                if let Err(e) = persistence::load_aof(path.as_ref(), &**c) {
                    eprintln!("failed to load AOF {path}: {e}");
                }
            }
            Backend::Sharded(s) => {
                if let Err(e) = persistence::load_aof(path.as_ref(), &**s) {
                    eprintln!("failed to load AOF {path}: {e}");
                }
            }
        }
    }

    println!("capsule-cache server listening on {addr} (shards={shard_count})");

    let listener = TcpListener::bind(&addr)?;
    for stream in listener.incoming() {
        let stream = stream?;
        let backend_cloned = backend.clone();
        let auth_required = auth_token.clone();
        handle_connection(
            stream,
            backend_cloned,
            aof_path.clone(),
            auth_required,
            slowlog.clone(),
        )?;
    }

    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    backend: Backend,
    aof_path: Option<String>,
    auth_token: Option<String>,
    slowlog: Arc<SlowLog>,
) -> io::Result<()> {
    let peer = stream.peer_addr()?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut authed = auth_token.is_none();
    let mut window_start = Instant::now();
    let mut window_count: u32 = 0;
    const RATE_LIMIT: u32 = 2000; // ops per second per connection

    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        if line.is_empty() {
            continue;
        }

        // Rate limit
        let now = Instant::now();
        if now.duration_since(window_start) >= Duration::from_secs(1) {
            window_start = now;
            window_count = 0;
        }
        window_count += 1;
        if window_count > RATE_LIMIT {
            let _ = stream.write_all(b"-ERR rate limit\r\n");
            continue;
        }

        let parts = match parse_command(&line, &mut reader) {
            Ok(p) => p,
            Err(_) => {
                let _ = stream.write_all(b"-ERR parse error\r\n");
                continue;
            }
        };
        if parts.is_empty() {
            continue;
        }

        let cmd = parts[0].to_ascii_uppercase();

        if cmd != "AUTH" && !authed {
            let _ = stream.write_all(b"-NOAUTH Authentication required\r\n");
            continue;
        }

        let cmd_start = Instant::now();
        let mut op = SlowOp::Other;
        let mut key_hash = 0u64;
        let mut ok = true;

        let resp = match cmd.as_str() {
            "PING" => "+PONG\r\n".into(),
            "AUTH" => {
                if auth_token.is_none() {
                    "-ERR AUTH not configured\r\n".into()
                } else {
                    match parts.get(1) {
                        Some(tok) if Some(tok) == auth_token.as_ref() => {
                            authed = true;
                            "+OK\r\n".into()
                        }
                        _ => {
                            ok = false;
                            "-ERR invalid password\r\n".into()
                        }
                    }
                }
            }
            "SET" => {
                if parts.len() < 4 {
                    "-ERR wrong number of arguments for SET\r\n".into()
                } else {
                    let key = parts[1].clone();
                    op = SlowOp::Set;
                    key_hash = hash_key(&key);
                    let ttl_s = parts[2].parse::<u64>().unwrap_or(0);
                    let value = parts[3].clone();
                    let ttl = Duration::from_secs(ttl_s);
                    match backend.insert(key.clone(), value.clone(), ttl) {
                        Ok(_) => {
                            if let Some(path) = aof_path.as_deref() {
                                let _ = persistence::append_set(path.as_ref(), &key, &value, ttl);
                            }
                            "+OK\r\n".into()
                        }
                        Err(e) => {
                            format!("-{}\r\n", e)
                        }
                    }
                }
            }
            "GET" => {
                if parts.len() < 2 {
                    "-ERR missing key\r\n".into()
                } else {
                    op = SlowOp::Get;
                    key_hash = hash_key(&parts[1]);
                    match backend.get(&parts[1]) {
                        Some(val) => {
                            format!("${}\r\n{}\r\n", val.len(), val)
                        }
                        None => {
                            "$-1\r\n".into()
                        }
                    }
                }
            }
            "DEL" => {
                if parts.len() < 2 {
                    "-ERR missing key\r\n".into()
                } else {
                    op = SlowOp::Del;
                    key_hash = hash_key(&parts[1]);
                    let deleted = backend.remove(&parts[1]);
                    if deleted {
                        if let Some(path) = aof_path.as_deref() {
                            let _ = persistence::append_del(path.as_ref(), &parts[1]);
                        }
                    }
                    format!(":{}\r\n", deleted as i32)
                }
            }
            "TTL" => {
                if parts.len() < 2 {
                    "-ERR missing key\r\n".into()
                } else {
                    op = SlowOp::Ttl;
                    key_hash = hash_key(&parts[1]);
                    match backend.ttl(&parts[1]) {
                        Some(ttl) => format!(":{}\r\n", ttl.as_secs()),
                        None => ":-2\r\n".into(),
                    }
                }
            }
            "EXPIRE" => {
                if parts.len() < 3 {
                    "-ERR wrong number of arguments for EXPIRE\r\n".into()
                } else {
                    op = SlowOp::Expire;
                    key_hash = hash_key(&parts[1]);
                    let ttl_s = parts[2].parse::<u64>().unwrap_or(0);
                    let ok = backend.expire(&parts[1], Duration::from_secs(ttl_s));
                    if ok {
                        "+OK\r\n".into()
                    } else {
                        ":0\r\n".into()
                    }
                }
            }
            "INCR" => {
                if parts.len() < 2 {
                    "-ERR missing key\r\n".into()
                } else {
                    op = SlowOp::Incr;
                    key_hash = hash_key(&parts[1]);
                    match backend.incr(&parts[1], 1) {
                        Ok(v) => format!(":{}\r\n", v),
                        Err(e) => {
                            ok = false;
                            format!("-{}\r\n", e)
                        }
                    }
                }
            }
            "MSET" => {
                if parts.len() < 4 || parts.len() % 3 != 1 {
                    "-ERR MSET expects tuples of key ttl value\r\n".into()
                } else {
                    op = SlowOp::Mset;
                    key_hash = hash_key(&parts[1]);
                    let mut cmd_ok = true;
                    for chunk in parts[1..].chunks(3) {
                        let key = chunk[0].clone();
                        let ttl = Duration::from_secs(chunk[1].parse::<u64>().unwrap_or(0));
                        let val = chunk[2].clone();
                        if backend.insert(key.clone(), val.clone(), ttl).is_ok() {
                            if let Some(path) = aof_path.as_deref() {
                                let _ = persistence::append_set(path.as_ref(), &key, &val, ttl);
                            }
                        } else {
                            cmd_ok = false;
                        }
                    }
                    if cmd_ok {
                        "+OK\r\n".into()
                    } else {
                        ok = false;
                        "-ERR MSET failed\r\n".into()
                    }
                }
            }
            "MGET" => {
                if parts.len() < 2 {
                    "-ERR missing keys\r\n".into()
                } else {
                    op = SlowOp::Mget;
                    key_hash = hash_key(&parts[1]);
                    let mut vals = Vec::new();
                    for k in parts.iter().skip(1) {
                        match backend.get(k) {
                            Some(v) => {
                                vals.push(format!("${}\r\n{}\r\n", v.len(), v));
                            }
                            None => {
                                vals.push("$-1\r\n".into());
                            }
                        }
                    }
                    format!("*{}\r\n{}", vals.len(), vals.concat())
                }
            }
            "STATS" => {
                let (snap, pct) = backend.stats_summary();
                let (hist_count, hist_overflow) = match &backend {
                    Backend::Single(c) => c.hist_counts(),
                    Backend::Sharded(s) => s.hist_counts(),
                };
                let mut lines = vec![
                    format!("total_requests={}", snap.total_requests),
                    format!("successful={}", snap.successful),
                    format!("failed={}", snap.failed),
                    format!("total_latency_ns={}", snap.total_latency_ns),
                    format!("min_latency_ns={}", snap.min_latency_ns),
                    format!("max_latency_ns={}", snap.max_latency_ns),
                    format!("hist_count={}", hist_count),
                    format!("hist_overflow={}", hist_overflow),
                    format!("slowlog_total={}", slowlog.total()),
                    format!("slowlog_len={}", slowlog.len_since_reset()),
                    format!("slowlog_threshold_ns={}", slowlog.threshold_ns()),
                ];
                if let Some(path) = slowlog.export_path() {
                    lines.push(format!("slowlog_export_path={}", path));
                }
                if let Some(p50) = pct.0 {
                    lines.push(format!("p50_ns={}", p50));
                }
                if let Some(p95) = pct.1 {
                    lines.push(format!("p95_ns={}", p95));
                }
                if let Some(p99) = pct.2 {
                    lines.push(format!("p99_ns={}", p99));
                }
                if let Some(p999) = pct.3 {
                    lines.push(format!("p999_ns={}", p999));
                }
                let body = lines.join("\n") + "\n";
                format!("${}\r\n{}\r\n", body.len(), body)
            }
            "SLOWLOG" => {
                match parts.get(1).map(|s| s.as_str().to_ascii_uppercase()) {
                    Some(ref cmd) if cmd == "LEN" => {
                        format!(":{}\r\n", slowlog.len_since_reset())
                    }
                    Some(ref cmd) if cmd == "RESET" => {
                        slowlog.reset();
                        "+OK\r\n".into()
                    }
                    _ => {
                        let count = parts
                            .get(1)
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(10);
                        let entries = slowlog.recent(count);
                        let mut body = String::new();
                        for e in entries {
                            body.push_str(&format!(
                                "seq={}\tts_ns={}\top={}\tduration_ns={}\tkey_hash={:#x}\tok={}\n",
                                e.seq,
                                e.ts_ns,
                                e.op.as_str(),
                                e.duration_ns,
                                e.key_hash,
                                e.ok as u8
                            ));
                        }
                        format!("${}\r\n{}\r\n", body.len(), body)
                    }
                }
            }
            "SCANHASH" => {
                let count = parts
                    .get(1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(10);
                let hashes = backend.scan_hashes(count);
                let mut body = String::new();
                for h in hashes {
                    body.push_str(&format!("{:#x}\n", h));
                }
                format!("${}\r\n{}\r\n", body.len(), body)
            }
            "FLUSHDB" => {
                let cleared = backend.clear_all();
                format!(":{}\r\n", cleared)
            }
            _ => {
                "-ERR unknown command\r\n".into()
            }
        };

        let elapsed_ns = cmd_start.elapsed().as_nanos() as u64;
        let success = ok && !resp.starts_with("-ERR");
        slowlog.maybe_record(op, key_hash, elapsed_ns, success);

        if let Err(e) = stream.write_all(resp.as_bytes()) {
            eprintln!("write error to {peer}: {e}");
            break;
        }
        backend.evict_expired();
    }

    Ok(())
}

fn parse_command<R: BufRead>(first_line: &str, reader: &mut R) -> io::Result<Vec<String>> {
    if first_line.starts_with('*') {
        parse_resp(first_line, reader)
    } else {
        Ok(first_line
            .trim_end_matches(['\r', '\n'])
            .split_whitespace()
            .map(|s| s.to_string())
            .collect())
    }
}

fn parse_resp<R: BufRead>(first_line: &str, reader: &mut R) -> io::Result<Vec<String>> {
    let count: usize = first_line[1..]
        .trim()
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad array count"))?;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut len_line = String::new();
        if reader.read_line(&mut len_line)? == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "short RESP"));
        }
        if !len_line.starts_with('$') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected bulk string",
            ));
        }
        let len: usize = len_line[1..]
            .trim()
            .parse()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad bulk len"))?;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        let mut crlf = [0u8; 2];
        reader.read_exact(&mut crlf)?; // consume \r\n
        let s = String::from_utf8_lossy(&buf).to_string();
        out.push(s);
    }
    Ok(out)
}
