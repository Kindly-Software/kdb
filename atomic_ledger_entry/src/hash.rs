use core::fmt;

use blake3::Hasher;

#[derive(Clone)]
pub struct AleKey {
    bytes: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyError {
    InvalidLength(usize),
}

impl AleKey {
    pub const LENGTH: usize = 32;

    pub fn new(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    pub fn from_slice(data: &[u8]) -> Result<Self, KeyError> {
        match data.len() {
            32 => {
                let mut bytes = [0u8; 32];
                bytes.copy_from_slice(data);
                Ok(Self::new(bytes))
            }
            16 => {
                let mut bytes = [0u8; 32];
                bytes[..16].copy_from_slice(data);
                bytes[16..].copy_from_slice(data);
                Ok(Self::new(bytes))
            }
            len => Err(KeyError::InvalidLength(len)),
        }
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
}

impl fmt::Debug for AleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AleKey{***}")
    }
}

pub fn chain_prev_hash(key: &AleKey, prev_entry: u128, meta_bits: u64) -> u64 {
    let mut hasher = Hasher::new_keyed(key.as_bytes());
    hasher.update(&prev_entry.to_be_bytes());
    hasher.update(&meta_bits.to_be_bytes());
    truncate64(hasher.finalize().as_bytes())
}

pub fn derive_genesis_hash(key: &AleKey, context: &[u8]) -> u64 {
    let mut hasher = Hasher::new_keyed(key.as_bytes());
    hasher.update(b"ALE|seed|");
    hasher.update(context);
    truncate64(hasher.finalize().as_bytes())
}

fn truncate64(bytes: &[u8; 32]) -> u64 {
    let mut out = [0u8; 8];
    out.copy_from_slice(&bytes[..8]);
    u64::from_be_bytes(out)
}
