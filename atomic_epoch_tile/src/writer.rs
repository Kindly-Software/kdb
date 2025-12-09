use core::ptr;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use crate::integrity::{tile_bytes, tile_checksum32, TileHash};
use crate::layout::{EtTile, TILE_LAYOUT_VERSION, TILE_MAGIC};

/// Scratch buffer that the writer fills before publishing into the ring.
#[derive(Debug, Default)]
pub struct TileShadow {
    tile: EtTile,
}

impl TileShadow {
    /// Creates a cleared shadow tile with the standard magic/layout version.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a mutable view of the underlying tile for population.
    pub fn tile_mut(&mut self) -> &mut EtTile {
        &mut self.tile
    }

    /// Returns an immutable view of the tile.
    pub fn tile(&self) -> &EtTile {
        &self.tile
    }

    /// Clears all fields back to zero while reapplying the tile header magic.
    pub fn reset(&mut self) {
        self.tile = EtTile::default();
    }
}

/// Borrowed access to a tile slot within a ring buffer or mapped region.
pub struct TileSlot<'a> {
    pub index: u16,
    tile: &'a mut EtTile,
}

impl<'a> TileSlot<'a> {
    pub fn new(index: u16, tile: &'a mut EtTile) -> Self {
        Self { index, tile }
    }

    fn tile(&mut self) -> &mut EtTile {
        self.tile
    }
}

/// Result of a successful publish.
#[derive(Debug, Clone, Copy)]
pub struct CommitOutcome {
    pub tile_index: u16,
    pub checksum: u32,
    pub seq_head: u8,
    pub ver_even: u8,
}

/// State machine driving the two-phase tile publication protocol.
pub struct TilePublisher {
    seq_head: u8,
    ver_token: u8,
    prev_tile_hash: [u8; 16],
    hash_key: Option<[u8; 32]>,
}

impl TilePublisher {
    /// Creates a publisher. The optional `hash_key` is fed into BLAKE3 when
    /// deriving the `prev_tile_hash` breadcrumb for the next publish.
    pub fn new(hash_key: Option<[u8; 32]>) -> Self {
        Self {
            seq_head: 0,
            ver_token: 1, // ensure odd for the first publish phase
            prev_tile_hash: [0; 16],
            hash_key,
        }
    }

    /// Seeds the hash chain with a previously persisted tile hash.
    pub fn with_prev_hash(mut self, hash: [u8; 16]) -> Self {
        self.prev_tile_hash = hash;
        self
    }

    pub fn prev_tile_hash(&self) -> [u8; 16] {
        self.prev_tile_hash
    }

    pub fn seq_head(&self) -> u8 {
        self.seq_head
    }

    /// Publishes the populated `shadow` into the provided ring slot.
    ///
    /// The caller must ensure the tile metadata (epoch id, timestamps, policy
    /// markers, digests, etc.) are already staged in the `shadow` before
    /// invoking this method. This function sets the `commit`, `ver_even`,
    /// `seq_head`, and associated tail markers to honour the ET-1kB contract
    /// before copying the tile into the slot.
    pub fn publish_into(
        &mut self,
        mut slot: TileSlot<'_>,
        shadow: &mut TileShadow,
    ) -> CommitOutcome {
        let odd_version = self.ver_token | 1;
        let even_version = odd_version.wrapping_add(1);
        let seq_head = self.seq_head;

        let staged = shadow.tile_mut();
        staged.header.magic = TILE_MAGIC;
        staged.header.layout_version = TILE_LAYOUT_VERSION;
        staged.header.commit = 0;
        staged.header.ver_even = even_version;
        staged.header.seq_head = seq_head;
        staged.header.prev_tile_hash = self.prev_tile_hash;
        staged.log.tail.ver_tail = even_version;
        staged.log.tail.seq_tail = seq_head;
        staged.log.tail.tile_index = slot.index;

        let checksum = tile_checksum32(&staged);

        unsafe {
            ptr::copy_nonoverlapping(staged as *const EtTile, slot.tile() as *mut EtTile, 1);
        }

        // Two-phase commit: checksum → version → commit flag.
        {
            let tile = slot.tile();
            store_u32_release(&mut tile.header.checksum32, checksum);
            store_u8_release(&mut tile.header.ver_even, even_version);
            store_u8_release(&mut tile.header.commit, 1);
        }

        // Refresh hash chain for the next publish.
        let committed_hash = {
            let tile = slot.tile();
            let bytes = tile_bytes(&*tile);
            if let Some(key) = &self.hash_key {
                TileHash::keyed_128(key, bytes)
            } else {
                TileHash::blake3_128(bytes)
            }
        };
        self.prev_tile_hash = committed_hash;

        self.seq_head = self.seq_head.wrapping_add(1);
        self.ver_token = self.ver_token.wrapping_add(2);

        CommitOutcome {
            tile_index: slot.index,
            checksum,
            seq_head,
            ver_even: even_version,
        }
    }
}

#[inline]
fn store_u8_release(cell: &mut u8, value: u8) {
    unsafe { (&*(cell as *mut u8 as *mut AtomicU8)).store(value, Ordering::Release) }
}

#[inline]
fn store_u32_release(cell: &mut u32, value: u32) {
    unsafe { (&*(cell as *mut u32 as *mut AtomicU32)).store(value, Ordering::Release) }
}
