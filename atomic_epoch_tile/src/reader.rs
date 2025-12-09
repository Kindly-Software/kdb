use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use thiserror::Error;

use crate::integrity::tile_checksum32;
use crate::layout::{EtTile, TILE_LAYOUT_VERSION, TILE_MAGIC};

/// Errors that may arise when validating a tile read from the ring.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TileValidationError {
    #[error("tile not yet committed")]
    NotCommitted,
    #[error("tile magic mismatch: found {found:?}")]
    MagicMismatch { found: [u8; 4] },
    #[error("layout version mismatch: found {found}, expected {expected}")]
    LayoutMismatch { found: u8, expected: u8 },
    #[error("expected even version flag, found {0}")]
    VersionNotEven(u8),
    #[error("checksum mismatch: stored {stored:#010x}, computed {computed:#010x}")]
    ChecksumMismatch { stored: u32, computed: u32 },
    #[error("tail/head version mismatch: head {head}, tail {tail}")]
    TailVersionMismatch { head: u8, tail: u8 },
    #[error("tail/head sequence mismatch: head {head}, tail {tail}")]
    TailSequenceMismatch { head: u8, tail: u8 },
}

/// Validates a tile according to the reader contract.
pub fn validate_tile(tile: &EtTile) -> Result<(), TileValidationError> {
    if tile.header.magic != TILE_MAGIC {
        return Err(TileValidationError::MagicMismatch {
            found: tile.header.magic,
        });
    }

    if tile.header.layout_version != TILE_LAYOUT_VERSION {
        return Err(TileValidationError::LayoutMismatch {
            found: tile.header.layout_version,
            expected: TILE_LAYOUT_VERSION,
        });
    }

    let commit = load_u8(&tile.header.commit);
    if commit != 1 {
        return Err(TileValidationError::NotCommitted);
    }

    let ver_even = load_u8(&tile.header.ver_even);
    if ver_even & 0x1 == 1 {
        return Err(TileValidationError::VersionNotEven(ver_even));
    }

    let stored_checksum = load_u32(&tile.header.checksum32);
    let computed_checksum = tile_checksum32(tile);
    if stored_checksum != computed_checksum {
        return Err(TileValidationError::ChecksumMismatch {
            stored: stored_checksum,
            computed: computed_checksum,
        });
    }

    if tile.log.tail.ver_tail != ver_even {
        return Err(TileValidationError::TailVersionMismatch {
            head: ver_even,
            tail: tile.log.tail.ver_tail,
        });
    }

    if tile.log.tail.seq_tail != tile.header.seq_head {
        return Err(TileValidationError::TailSequenceMismatch {
            head: tile.header.seq_head,
            tail: tile.log.tail.seq_tail,
        });
    }

    Ok(())
}

/// Walks the ring from the provided index (inclusive, wrapping) to locate the
/// most recent committed tile.
pub fn scan_latest_committed<'a>(
    ring: &'a [EtTile],
    start_index: usize,
) -> Option<(usize, &'a EtTile)> {
    if ring.is_empty() {
        return None;
    }

    let mut idx = start_index.min(ring.len() - 1);
    for _ in 0..ring.len() {
        let tile = &ring[idx];
        if validate_tile(tile).is_ok() {
            return Some((idx, tile));
        }
        if idx == 0 {
            idx = ring.len() - 1;
        } else {
            idx -= 1;
        }
    }

    None
}

fn load_u8(cell: &u8) -> u8 {
    unsafe { (&*(cell as *const u8 as *const AtomicU8)).load(Ordering::Acquire) }
}

fn load_u32(cell: &u32) -> u32 {
    unsafe { (&*(cell as *const u32 as *const AtomicU32)).load(Ordering::Acquire) }
}
