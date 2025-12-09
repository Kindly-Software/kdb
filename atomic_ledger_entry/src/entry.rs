use core::fmt;

use crate::layout::{unpack, AleMeta};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AleEntry {
    raw: u128,
}

impl AleEntry {
    pub const fn new(prev_hash: u64, meta: u64) -> Self {
        let upper = (prev_hash as u128) << 64;
        let lower = meta as u128;
        Self { raw: upper | lower }
    }

    pub const fn from_raw(raw: u128) -> Self {
        Self { raw }
    }

    pub const fn raw(self) -> u128 {
        self.raw
    }

    pub const fn prev_hash(self) -> u64 {
        (self.raw >> 64) as u64
    }

    pub const fn meta_bits(self) -> u64 {
        self.raw as u64
    }

    pub fn meta(self) -> AleMeta {
        unpack(self.meta_bits())
    }

    pub fn split(self) -> (u64, u64) {
        (self.prev_hash(), self.meta_bits())
    }
}

impl From<u128> for AleEntry {
    fn from(value: u128) -> Self {
        Self::from_raw(value)
    }
}

impl From<AleEntry> for u128 {
    fn from(entry: AleEntry) -> Self {
        entry.raw
    }
}

impl fmt::Debug for AleEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (hash, meta) = self.split();
        f.debug_struct("AleEntry")
            .field("prev_hash", &format_args!("0x{hash:016x}"))
            .field("meta", &self.meta())
            .field("meta_bits", &format_args!("0x{meta:016x}"))
            .finish()
    }
}
