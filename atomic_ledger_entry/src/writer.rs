use alloc::boxed::Box;
use alloc::vec::Vec;

use core::sync::atomic::{AtomicUsize, Ordering};

use portable_atomic::AtomicU128;

use crate::entry::AleEntry;
use crate::event::AleEvent;
use crate::hash::{chain_prev_hash, AleKey};
use crate::layout::MetaError;

pub struct AleRing {
    slots: Box<[AtomicU128]>,
    publish: AtomicUsize,
    pub(crate) mask: usize,
}

impl AleRing {
    pub fn with_capacity_pow2(capacity: usize) -> Self {
        assert!(
            capacity.is_power_of_two(),
            "capacity must be a power of two"
        );
        assert!(capacity > 0, "capacity must be non-zero");
        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push(AtomicU128::new(0));
        }
        Self {
            slots: slots.into_boxed_slice(),
            publish: AtomicUsize::new(0),
            mask: capacity - 1,
        }
    }

    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    pub fn published(&self) -> usize {
        self.publish.load(Ordering::Acquire)
    }

    pub fn load(&self, index: usize, ordering: Ordering) -> u128 {
        self.slots[index & self.mask].load(ordering)
    }

    pub fn store(&self, index: usize, value: u128, ordering: Ordering) {
        self.slots[index & self.mask].store(value, ordering);
    }

    pub fn as_slice(&self) -> &[AtomicU128] {
        &self.slots
    }
}

#[derive(Clone, Copy, Default)]
pub struct WriterConfig {
    pub head: usize,
    pub initial_seq: u8,
    pub genesis_prev_hash: u64,
    pub last_entry: Option<u128>,
}

pub struct Writer<'a> {
    ring: &'a AleRing,
    key: AleKey,
    head: usize,
    seq: u8,
    prev_entry: Option<u128>,
    genesis_prev_hash: u64,
}

impl<'a> Writer<'a> {
    pub fn new(ring: &'a AleRing, key: &AleKey, config: WriterConfig) -> Self {
        ring.publish.store(config.head, Ordering::Release);
        Self {
            ring,
            key: key.clone(),
            head: config.head,
            seq: config.initial_seq,
            prev_entry: config.last_entry,
            genesis_prev_hash: config.genesis_prev_hash,
        }
    }

    pub fn position(&self) -> usize {
        self.head
    }

    pub fn last_entry(&self) -> Option<AleEntry> {
        self.prev_entry.map(AleEntry::from)
    }

    pub fn append(&mut self, event: AleEvent) -> Result<AleEntry, MetaError> {
        let seq = self.seq.wrapping_add(1);
        self.seq = seq;
        let meta = event.into_meta(seq);
        let bits = meta.pack()?;
        let prev_hash = match self.prev_entry {
            Some(prev) => chain_prev_hash(&self.key, prev, bits),
            None => self.genesis_prev_hash,
        };
        let entry = AleEntry::new(prev_hash, bits);
        let slot_index = self.head & self.ring.mask;
        self.ring.slots[slot_index].store(entry.raw(), Ordering::Relaxed);
        self.head = self.head.wrapping_add(1);
        self.prev_entry = Some(entry.raw());
        self.ring.publish.store(self.head, Ordering::Release);
        Ok(entry)
    }
}
