use blake3::Hasher;
use memoffset::offset_of;
use xxhash_rust::xxh32::Xxh32;

use crate::layout::{EtTile, HeaderSection, TILE_SIZE};

const HASH_START: usize = 4; // skip magic prefix
const COMMIT_OFFSET: usize = offset_of!(HeaderSection, commit);
const CHECKSUM_OFFSET: usize = offset_of!(HeaderSection, checksum32);
const CHECKSUM_END: usize = CHECKSUM_OFFSET + core::mem::size_of::<u32>();

/// Computes the XXH32 checksum for the provided tile.
///
/// The caller must ensure `commit` is cleared before invoking this helper so the
/// checksum matches the publish contract. The checksum covers bytes in the
/// range `[0x04..0x400)` (skipping the magic prefix), matching the spec.
pub fn tile_checksum32(tile: &EtTile) -> u32 {
    let bytes = tile_bytes(tile);
    let mut hasher = Xxh32::new(0);
    if HASH_START < COMMIT_OFFSET {
        hasher.update(&bytes[HASH_START..COMMIT_OFFSET]);
    }
    hasher.update(&[0]);
    if COMMIT_OFFSET + 1 < CHECKSUM_OFFSET {
        hasher.update(&bytes[COMMIT_OFFSET + 1..CHECKSUM_OFFSET]);
    }
    hasher.update(&[0, 0, 0, 0]);
    if CHECKSUM_END < TILE_SIZE {
        hasher.update(&bytes[CHECKSUM_END..]);
    }
    hasher.digest()
}

/// Returns the raw tile bytes.
pub fn tile_bytes(tile: &EtTile) -> &[u8] {
    unsafe { core::slice::from_raw_parts(tile as *const _ as *const u8, TILE_SIZE) }
}

/// Returns the raw tile bytes mutably.
pub fn tile_bytes_mut(tile: &mut EtTile) -> &mut [u8] {
    unsafe { core::slice::from_raw_parts_mut(tile as *mut _ as *mut u8, TILE_SIZE) }
}

/// Helper utilities for computing keyed BLAKE3 digests that feed the integrity
/// breadcrumb fields inside the tile.
pub struct TileHash;

impl TileHash {
    /// Produces a 128-bit (16-byte) truncated BLAKE3 hash for the supplied
    /// payload using a 32-byte key.
    pub fn keyed_128(key: &[u8; 32], payload: &[u8]) -> [u8; 16] {
        let mut hasher = Hasher::new_keyed(key);
        hasher.update(payload);
        let mut out = [0u8; 16];
        hasher.finalize_xof().fill(&mut out);
        out
    }

    /// Produces a 64-bit (8-byte) truncated BLAKE3 hash for the supplied
    /// payload using a 32-byte key.
    pub fn keyed_64(key: &[u8; 32], payload: &[u8]) -> [u8; 8] {
        let mut hasher = Hasher::new_keyed(key);
        hasher.update(payload);
        let mut out = [0u8; 8];
        hasher.finalize_xof().fill(&mut out);
        out
    }

    /// Produces an unkeyed 128-bit truncated BLAKE3 hash for a payload.
    pub fn blake3_128(payload: &[u8]) -> [u8; 16] {
        let mut hasher = Hasher::new();
        hasher.update(payload);
        let mut out = [0u8; 16];
        hasher.finalize_xof().fill(&mut out);
        out
    }

    /// Produces an unkeyed 64-bit truncated BLAKE3 hash for a payload.
    pub fn blake3_64(payload: &[u8]) -> [u8; 8] {
        let mut hasher = Hasher::new();
        hasher.update(payload);
        let mut out = [0u8; 8];
        hasher.finalize_xof().fill(&mut out);
        out
    }
}
