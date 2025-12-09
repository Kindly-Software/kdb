use core::fmt;

use crate::entry::AleEntry;
use crate::hash::{chain_prev_hash, AleKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainMismatch {
    pub index: usize,
    pub expected: u64,
    pub actual: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SequenceGap {
    pub index: usize,
    pub expected: u8,
    pub actual: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifyError {
    Chain(ChainMismatch),
    Sequence(SequenceGap),
}

pub fn verify_chain(
    entries: &[u128],
    key: &AleKey,
    genesis_prev_hash: u64,
) -> Result<(), VerifyError> {
    let mut prev_entry: Option<u128> = None;
    let mut prev_seq: Option<u8> = None;
    for (idx, &raw) in entries.iter().enumerate() {
        let entry = AleEntry::from_raw(raw);
        let meta_bits = entry.meta_bits();
        let meta = entry.meta();
        let expected_hash = match prev_entry {
            Some(prev) => chain_prev_hash(key, prev, meta_bits),
            None => genesis_prev_hash,
        };
        if entry.prev_hash() != expected_hash {
            return Err(VerifyError::Chain(ChainMismatch {
                index: idx,
                expected: expected_hash,
                actual: entry.prev_hash(),
            }));
        }
        if let Some(prev_seq_value) = prev_seq {
            let expected_seq = prev_seq_value.wrapping_add(1);
            if meta.seq != expected_seq {
                return Err(VerifyError::Sequence(SequenceGap {
                    index: idx,
                    expected: expected_seq,
                    actual: meta.seq,
                }));
            }
        }
        prev_entry = Some(raw);
        prev_seq = Some(meta.seq);
    }
    Ok(())
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyError::Chain(m) => write!(
                f,
                "hash mismatch at entry {} (expected 0x{:016x}, found 0x{:016x})",
                m.index, m.expected, m.actual
            ),
            VerifyError::Sequence(g) => write!(
                f,
                "sequence gap at entry {} (expected {}, found {})",
                g.index, g.expected, g.actual
            ),
        }
    }
}
