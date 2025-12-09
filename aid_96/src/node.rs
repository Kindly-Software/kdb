use blake3::Hasher;
use std::fs::File;
use std::io::{Read, Result as IoResult};
use std::sync::OnceLock;
use std::time::SystemTime;

static NODE_ID: OnceLock<u16> = OnceLock::new();

pub fn node_id() -> u16 {
    *NODE_ID.get_or_init(compute_node_id)
}

fn compute_node_id() -> u16 {
    let hostname = hostname::get()
        .map(|host| host.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown-host".into());

    let boot_uuid = random_boot_uuid();
    let pid = std::process::id().to_le_bytes();
    let binary_hash = binary_hash().unwrap_or_else(|_| blake3::hash(b"aid-96-fallback").into());

    let mut hasher = Hasher::new();
    hasher.update(hostname.as_bytes());
    hasher.update(&boot_uuid);
    hasher.update(&pid);
    hasher.update(&binary_hash);

    let digest = hasher.finalize();
    let bytes = digest.as_bytes();
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn random_boot_uuid() -> [u8; 16] {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() {
        let fallback = fallback_entropy();
        bytes.copy_from_slice(&fallback[..16]);
    }
    bytes
}

fn fallback_entropy() -> [u8; 32] {
    let mut hasher = Hasher::new();
    let now = format!("{:?}", SystemTime::now());
    hasher.update(now.as_bytes());
    let thread_id = format!("{:?}", std::thread::current().id());
    hasher.update(thread_id.as_bytes());
    hasher.finalize().into()
}

fn binary_hash() -> IoResult<[u8; 32]> {
    let path = std::env::current_exe()?;
    let mut file = File::open(path)?;
    let mut hasher = Hasher::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_is_stable_within_process() {
        let first = node_id();
        for _ in 0..50 {
            assert_eq!(first, node_id());
        }
    }
}
