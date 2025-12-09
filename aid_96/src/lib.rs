#![forbid(unsafe_code)]

//! AID-96 — Atomic IDs packed as `time | node | counter | class` in 12 bytes.
//! The generator is lock-free, monotonic per node, and produces sortable identifiers.

mod base32;
mod clock;
mod layout;
mod node;

pub use base32::{decode as decode_base32, encode as encode_base32, DecodeError};

use clock::MonotonicClock;
use layout::{pack, MAX_COUNTER, MAX_TIME_MS};
use node::node_id;
use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{de::Error as SerdeError, Deserialize, Deserializer, Serialize, Serializer};

/// 12-byte identifier with the layout `[time48 | node16 | counter24 | class8]`.
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct Aid96 {
    bytes: [u8; layout::ID_SIZE],
}

impl Aid96 {
    /// Generate a new identifier for the given class code.
    pub fn new(class: u8) -> Self {
        GENERATOR.generate(class)
    }

    /// Create an identifier from its packed fields.
    pub fn from_parts(time_ms: u64, node_id: u16, counter: u32, class: u8) -> Self {
        assert!(time_ms <= MAX_TIME_MS, "time_ms exceeds 48 bits");
        assert!(counter <= MAX_COUNTER, "counter exceeds 24 bits");
        Self {
            bytes: pack(time_ms, node_id, counter, class),
        }
    }

    /// Borrow the raw 12-byte representation.
    pub fn as_bytes(&self) -> &[u8; layout::ID_SIZE] {
        &self.bytes
    }

    /// Consume the ID and return the raw bytes.
    pub fn into_bytes(self) -> [u8; layout::ID_SIZE] {
        self.bytes
    }

    /// Extract the time component (milliseconds since 2025-01-01T00:00:00Z).
    pub fn time_ms(&self) -> u64 {
        layout::read_u48_be(&self.bytes[0..6])
    }

    /// Extract the node identifier.
    pub fn node_id(&self) -> u16 {
        layout::read_u16_be(&self.bytes[6..8])
    }

    /// Extract the counter (24-bit, shard|sequence).
    pub fn counter(&self) -> u32 {
        layout::read_u24_be(&self.bytes[8..11])
    }

    /// Extract the class byte.
    pub fn class(&self) -> u8 {
        self.bytes[11]
    }

    /// Encode as a 20-character Crockford Base32 string.
    pub fn to_base32(&self) -> String {
        base32::encode(&self.bytes)
    }

    /// Encode with an uppercase prefix (e.g. `"AEB"`). The prefix is separated by `_`.
    pub fn to_base32_with_prefix(&self, prefix: &str) -> String {
        if prefix.is_empty() {
            return self.to_base32();
        }
        let mut out = String::with_capacity(prefix.len() + 1 + 20);
        out.push_str(prefix);
        out.push('_');
        out.push_str(&self.to_base32());
        out
    }

    /// Decode from a 20-character Base32 string.
    pub fn from_base32(input: &str) -> Result<Self, DecodeError> {
        base32::decode(input).map(|bytes| Self { bytes })
    }

    /// Decode from an optional-prefix format `PREFIX_XXXXXXXX...`.
    pub fn from_prefixed_base32(input: &str) -> Result<Self, DecodeError> {
        let trimmed = input.trim();
        let core = match trimmed.rsplit_once('_') {
            Some((prefix, suffix)) if suffix.len() == 20 && !prefix.is_empty() => suffix,
            Some((_prefix, suffix)) if suffix.len() != 20 => {
                return Err(DecodeError::InvalidLength {
                    expected: 20,
                    found: suffix.len(),
                });
            }
            _ => trimmed,
        };
        Self::from_base32(core)
    }
}

impl fmt::Debug for Aid96 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Aid96")
            .field("time_ms", &self.time_ms())
            .field("node_id", &format_args!("0x{:04X}", self.node_id()))
            .field("counter", &format_args!("0x{:06X}", self.counter()))
            .field("class", &format_args!("0x{:02X}", self.class()))
            .field("base32", &self.to_base32())
            .finish()
    }
}

impl fmt::Display for Aid96 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_base32())
    }
}

impl From<[u8; layout::ID_SIZE]> for Aid96 {
    fn from(bytes: [u8; layout::ID_SIZE]) -> Self {
        Self { bytes }
    }
}

#[cfg(feature = "serde")]
impl Serialize for Aid96 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_base32())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Aid96 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Aid96::from_prefixed_base32(&value).map_err(SerdeError::custom)
    }
}

impl FromStr for Aid96 {
    type Err = DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_prefixed_base32(s)
    }
}

/// Free function convenience wrapper for `Aid96::new`.
pub fn new_aid(class: u8) -> Aid96 {
    Aid96::new(class)
}

/// Class codes reserved for AID-96 capsules.
pub mod class {
    pub const UNSPECIFIED: u8 = 0x00;
    pub const AEB: u8 = 0x01;
    pub const AHC: u8 = 0x02;
    pub const APC: u8 = 0x03;
    pub const APM: u8 = 0x04;
    pub const AVS: u8 = 0x05;
    pub const ALT: u8 = 0x06;
    pub const DOS: u8 = 0x07;
    pub const ASF: u8 = 0x08;
    pub const ECO: u8 = 0x09;
    pub const RLT: u8 = 0x0A;
    pub const ALE: u8 = 0x0B;
    pub const ET: u8 = 0x0C;
    pub const ARE: u8 = 0x0D;
    pub const ACT: u8 = 0x0E;
    pub const PEX: u8 = 0x0F;
    pub const ACB: u8 = 0x10;
}

struct AidGenerator {
    clock: MonotonicClock,
}

impl AidGenerator {
    const fn new() -> Self {
        Self {
            clock: MonotonicClock::new(),
        }
    }

    fn generate(&self, class: u8) -> Aid96 {
        loop {
            let now_ms = self.clock.now();
            if let Some((shard_id, seq)) = THREAD_STATE.with(|state| state.try_next(now_ms)) {
                let counter = ((shard_id as u32) << 16) | seq as u32;
                let bytes = pack(now_ms, node_id(), counter, class);
                return Aid96 { bytes };
            }
            std::hint::spin_loop();
        }
    }
}

struct ThreadState {
    shard_id: u8,
    last_ms: Cell<u64>,
    seq: Cell<u16>,
}

impl ThreadState {
    fn new() -> Self {
        Self {
            shard_id: compute_shard_id(),
            last_ms: Cell::new(u64::MAX),
            seq: Cell::new(0),
        }
    }

    fn try_next(&self, now_ms: u64) -> Option<(u8, u16)> {
        let last = self.last_ms.get();
        if last == now_ms {
            let next = self.seq.get().wrapping_add(1);
            self.seq.set(next);
            if next == 0 {
                return None;
            }
            Some((self.shard_id, next))
        } else {
            self.last_ms.set(now_ms);
            self.seq.set(0);
            Some((self.shard_id, 0))
        }
    }
}

thread_local! {
    static THREAD_STATE: ThreadState = ThreadState::new();
}

fn compute_shard_id() -> u8 {
    let thread_id = std::thread::current().id();
    let mut hasher = DefaultHasher::new();
    thread_id.hash(&mut hasher);
    (hasher.finish() & 0xFF) as u8
}

static GENERATOR: AidGenerator = AidGenerator::new();

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::thread;

    #[test]
    fn generated_ids_are_monotonic_on_single_thread() {
        let mut last = Aid96::new(class::UNSPECIFIED);
        for _ in 0..10_000 {
            let current = Aid96::new(class::UNSPECIFIED);
            assert!(current.time_ms() >= last.time_ms());
            if current.time_ms() == last.time_ms() {
                assert!(current.counter() != last.counter());
            }
            last = current;
        }
    }

    #[test]
    fn multithreaded_generation_has_no_duplicates() {
        const THREADS: usize = 8;
        const IDS_PER_THREAD: usize = 2_000;

        let mut handles = Vec::new();
        for _ in 0..THREADS {
            handles.push(thread::spawn(|| {
                let mut ids = Vec::with_capacity(IDS_PER_THREAD);
                for _ in 0..IDS_PER_THREAD {
                    ids.push(Aid96::new(class::ALE));
                }
                ids
            }));
        }

        let mut all = BTreeSet::new();
        for handle in handles {
            for id in handle.join().expect("thread panicked") {
                let inserted = all.insert(id.into_bytes());
                assert!(inserted, "duplicate ID detected");
            }
        }
    }

    #[test]
    fn base32_round_trip_with_prefix() {
        let id = Aid96::new(class::AEB);
        let encoded = id.to_base32_with_prefix("AEB");
        let parsed = Aid96::from_prefixed_base32(&encoded).expect("parse should succeed");
        assert_eq!(id.into_bytes(), parsed.into_bytes());
    }

    #[test]
    fn from_str_accepts_plain_and_prefixed() {
        let id = Aid96::new(class::DOS);
        let encoded = id.to_base32();
        let parsed_plain = encoded.parse::<Aid96>().expect("parse plain");
        assert_eq!(parsed_plain.into_bytes(), id.into_bytes());

        let encoded_prefixed = id.to_base32_with_prefix("DOS");
        let parsed_prefixed = encoded_prefixed.parse::<Aid96>().expect("parse prefixed");
        assert_eq!(parsed_prefixed.into_bytes(), id.into_bytes());
    }

    #[test]
    fn aid_from_parts_rejects_out_of_range() {
        let result = std::panic::catch_unwind(|| Aid96::from_parts(MAX_TIME_MS + 1, 0, 0, 0));
        assert!(result.is_err());
        let result = std::panic::catch_unwind(|| Aid96::from_parts(0, 0, MAX_COUNTER + 1, 0));
        assert!(result.is_err());
    }
}
