#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::declare_interior_mutable_const)]
#![doc = include_str!("../README.md")]

mod capsule;

pub mod layout;

pub use capsule::{
    Dos1024, Dos1024Snapshot, DosHeader, DosInstrument, DosInstrumentDerived, DosInstrumentHeader,
    DosLevel, DosSummary, PackedDos1024,
};

#[cfg(feature = "std")]
/// High-level writer utilities and sweep/trend helpers (requires the `std` feature).
pub mod writer;

#[cfg(feature = "std")]
pub use writer::{DosWriter, InstrumentInput, LevelInput, WriterConfig, WriterInput};
