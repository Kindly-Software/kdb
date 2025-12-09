#![no_std]

//! ALE-128 packs a tamper-evident audit entry into one 16-byte word.
//! The high 64 bits store the chained hash of the previous entry plus
//! the current metadata, and the low 64 bits hold the compact fields
//! defined by the ledger layout.

extern crate alloc;
#[cfg(any(test, feature = "std"))]
extern crate std;

pub use crate::entry::AleEntry;
pub use crate::event::{AleEvent, EventCodes};
pub use crate::hash::{chain_prev_hash, derive_genesis_hash, AleKey, KeyError};
pub use crate::layout::{clamp_payload, AleMeta, MetaError, Route2, PAYLOAD_MAX, PAYLOAD_MIN};
#[cfg(feature = "stream")]
pub use crate::stream::{
    LedgerProducer, LedgerStream, LedgerStreamBuilder, StreamBuildError, StreamJoinError,
    StreamStats,
};
pub use crate::validator::{verify_chain, ChainMismatch, SequenceGap, VerifyError};
pub use crate::writer::{AleRing, Writer, WriterConfig};

mod entry;
mod event;
mod hash;
mod layout;
#[cfg(feature = "stream")]
mod stream;
mod validator;
mod writer;

#[cfg(test)]
mod tests;
