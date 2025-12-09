use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;

use memmap2::{Mmap, MmapMut};

use crate::layout::{EtTile, TILE_SIZE};
use crate::writer::TileSlot;

/// Strategy used when flushing the memory-mapped ring to disk.
#[derive(Debug, Clone, Copy)]
pub enum FlushStrategy {
    Async,
    Sync,
}

/// Mutable view over an ET tile ring backed by a memory-mapped file.
pub struct TileRing {
    _file: File,
    map: MmapMut,
    tile_count: usize,
}

impl TileRing {
    /// Creates (or truncates) a ring file with `tile_count` entries and memory maps it.
    pub fn create<P: AsRef<Path>>(path: P, tile_count: usize) -> io::Result<Self> {
        if tile_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "tile_count must be greater than zero",
            ));
        }

        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        Self::init_file(&mut file, tile_count)?;
        let map = unsafe { MmapMut::map_mut(&file)? };
        Ok(Self {
            _file: file,
            map,
            tile_count,
        })
    }

    /// Opens an existing ring file mapping it mutably. The file must already be
    /// sized to `tile_count * TILE_SIZE`.
    pub fn open<P: AsRef<Path>>(path: P, tile_count: usize) -> io::Result<Self> {
        if tile_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "tile_count must be greater than zero",
            ));
        }

        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let metadata = file.metadata()?;
        let expected_len = Self::expected_len(tile_count)?;
        if metadata.len() != expected_len as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ring file size {} does not match expected length {}",
                    metadata.len(),
                    expected_len
                ),
            ));
        }
        let map = unsafe { MmapMut::map_mut(&file)? };
        Ok(Self {
            _file: file,
            map,
            tile_count,
        })
    }

    /// Returns the number of tiles in the ring.
    pub fn len(&self) -> usize {
        self.tile_count
    }

    /// Provides mutable access to the underlying tiles slice.
    pub fn tiles_mut(&mut self) -> &mut [EtTile] {
        unsafe {
            core::slice::from_raw_parts_mut(self.map.as_mut_ptr() as *mut EtTile, self.tile_count)
        }
    }

    /// Returns a mutable tile slot suitable for publishing.
    pub fn tile_slot(&mut self, index: usize) -> TileSlot<'_> {
        let index = index % self.tile_count;
        let tiles = self.tiles_mut();
        TileSlot::new(index as u16, &mut tiles[index])
    }

    /// Flushes the memory-mapped region using the requested strategy.
    pub fn flush(&mut self, strategy: FlushStrategy) -> io::Result<()> {
        match strategy {
            FlushStrategy::Async => self.map.flush_async(),
            FlushStrategy::Sync => self.map.flush(),
        }
    }

    fn init_file(file: &mut File, tile_count: usize) -> io::Result<()> {
        let expected_len = Self::expected_len(tile_count)? as u64;
        file.set_len(expected_len)?;
        file.seek(SeekFrom::Start(0))?;
        let zero_tile = [0u8; TILE_SIZE];
        file.write_all(&zero_tile)?; // touch first page to guarantee allocation
        Ok(())
    }

    fn expected_len(tile_count: usize) -> io::Result<usize> {
        tile_count
            .checked_mul(TILE_SIZE)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "ring too large"))
    }
}

/// Read-only mapping of a tile ring.
pub struct TileRingMapping {
    map: Mmap,
    tile_count: usize,
}

impl TileRingMapping {
    /// Opens an existing ring file read-only.
    pub fn open<P: AsRef<Path>>(path: P, tile_count: usize) -> io::Result<Self> {
        if tile_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "tile_count must be greater than zero",
            ));
        }

        let file = OpenOptions::new().read(true).open(path)?;
        let metadata = file.metadata()?;
        let expected_len = tile_count
            .checked_mul(TILE_SIZE)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "ring too large"))?;
        if metadata.len() != expected_len as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ring file size {} does not match expected length {}",
                    metadata.len(),
                    expected_len
                ),
            ));
        }
        let map = unsafe { Mmap::map(&file)? };
        Ok(Self { map, tile_count })
    }

    /// Borrows the tiles slice for inspection.
    pub fn tiles(&self) -> &[EtTile] {
        unsafe { core::slice::from_raw_parts(self.map.as_ptr() as *const EtTile, self.tile_count) }
    }
}
