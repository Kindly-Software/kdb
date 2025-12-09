//! Minimal append-only persistence for CapsuleCache (no external deps, single-threaded).
//! Format (binary-friendly, length-prefixed):
//! `SET <expiry_ms> <klen> <vlen>\n` followed by klen bytes, `\n`, vlen bytes, `\n`.
//! `DEL <klen>\n` followed by klen bytes, `\n`.

use crate::{sharded::ShardedCache, CapsuleCache};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub trait AofTarget {
    fn put(&self, key: String, value: String, ttl: Duration);
    fn del(&self, key: String);
}

impl AofTarget for CapsuleCache<String> {
    fn put(&self, key: String, value: String, ttl: Duration) {
        let _ = self.insert(key, value, ttl);
    }
    fn del(&self, key: String) {
        let _ = self.remove(&key);
    }
}

impl AofTarget for ShardedCache<String> {
    fn put(&self, key: String, value: String, ttl: Duration) {
        let _ = self.insert(key, value, ttl);
    }
    fn del(&self, key: String) {
        let _ = self.remove(&key);
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
}

/// Append a SET operation to the AOF file.
pub fn append_set(path: &Path, key: &str, value: &str, expiry: Duration) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let expiry_ms = now_ms().saturating_add(expiry.as_millis());
    let header = format!("SET {expiry_ms} {} {}\n", key.len(), value.len());
    file.write_all(header.as_bytes())?;
    file.write_all(key.as_bytes())?;
    file.write_all(b"\n")?;
    file.write_all(value.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

/// Append a DEL operation to the AOF file.
pub fn append_del(path: &Path, key: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let header = format!("DEL {}\n", key.len());
    file.write_all(header.as_bytes())?;
    file.write_all(key.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn read_exact_with_newline<R: Read>(reader: &mut R, len: usize) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

/// Replay the AOF file into the cache (skips expired entries).
pub fn load_aof<T: AofTarget>(path: &Path, cache: &T) -> io::Result<()> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };

    let mut reader = BufReader::new(file);
    let mut line = String::new();

    while {
        line.clear();
        reader.read_line(&mut line)?
    } != 0
    {
        let trimmed = line.trim_end_matches('\n');
        if trimmed.is_empty() {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("SET ") {
            let mut parts = rest.split_whitespace();
            let expiry_ms: u128 = match parts.next().and_then(|p| p.parse().ok()) {
                Some(v) => v,
                None => continue,
            };
            let klen: usize = match parts.next().and_then(|p| p.parse().ok()) {
                Some(v) => v,
                None => continue,
            };
            let vlen: usize = match parts.next().and_then(|p| p.parse().ok()) {
                Some(v) => v,
                None => continue,
            };

            let mut key_buf = read_exact_with_newline(&mut reader, klen)?;
            let mut newline = [0u8; 1];
            reader.read_exact(&mut newline)?; // consume newline
            let mut val_buf = read_exact_with_newline(&mut reader, vlen)?;
            reader.read_exact(&mut newline)?; // consume newline

            let key = match String::from_utf8(key_buf.split_off(0)) {
                Ok(k) => k,
                Err(_) => continue,
            };
            let value = match String::from_utf8(val_buf.split_off(0)) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let now = now_ms();
            if expiry_ms <= now {
                continue;
            }
            let remaining = Duration::from_millis((expiry_ms - now) as u64);
            cache.put(key, value, remaining);
        } else if let Some(rest) = trimmed.strip_prefix("DEL ") {
            let klen: usize = match rest.parse().ok() {
                Some(v) => v,
                None => continue,
            };
            let mut key_buf = read_exact_with_newline(&mut reader, klen)?;
            let mut newline = [0u8; 1];
            reader.read_exact(&mut newline)?; // consume newline
            if let Ok(key) = String::from_utf8(key_buf.split_off(0)) {
                cache.del(key);
            }
        }
    }

    Ok(())
}
