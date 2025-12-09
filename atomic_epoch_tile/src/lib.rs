//! Epoch Tile (ET-1kB) primitive crate.
//!
//! This crate defines the fixed-layout snapshot tile alongside helper utilities
//! for publishing crash-safe tiles and validating them on the reader side.

pub mod builder;
pub mod integrity;
pub mod layout;
pub mod reader;
pub mod ring;
pub mod session;
pub mod writer;

pub use builder::{
    populate_tile, CountersInputs, HeaderInputs, LogInputs, LogInputsEntry, SymbolInputs,
    TileInputs,
};
pub use integrity::{tile_checksum32, TileHash};
pub use layout::{
    CountersSection, EtTile, HeaderSection, LogEntry, LogSection, SymbolSection, SymbolSlice,
};
pub use reader::{scan_latest_committed, validate_tile, TileValidationError};
pub use ring::{FlushStrategy, TileRing, TileRingMapping};
pub use session::{
    build_tile_inputs, publish_from_feeds, LiveFeeds, SessionMetadata, SYMBOL_FLAG_CAN_SCALE,
    SYMBOL_FLAG_LOCKOUT, SYMBOL_FLAG_REDUCE_ONLY,
};
pub use writer::{CommitOutcome, TilePublisher, TileShadow, TileSlot};

#[cfg(test)]
mod tests;
