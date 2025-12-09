#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![doc = include_str!("../README.md")]

mod capsule;

pub mod layout;

pub use capsule::{Avs128, Avs128Snapshot, PackedAvs, AtomicVenueSnapshotWithBreaker, MarketQualityThresholds};

#[cfg(feature = "network")]
pub use capsule::{AvsNet, AvsNetSnapshot};

#[cfg(feature = "std")]
pub mod writer;

#[cfg(feature = "std")]
pub use writer::{AvsWriter, WriterConfig, WriterInput};

#[cfg(feature = "std")]
pub mod analysis;

#[cfg(feature = "std")]
pub use analysis::{SnapshotStats, SnapshotStatsBuilder};
