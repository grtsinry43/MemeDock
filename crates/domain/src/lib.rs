//! Platform-independent identities, state transitions and versioned contracts.
//!
//! Filesystem verification, database transactions, authorization and scheduling
//! belong to storage/core. Deserializing metadata does not verify image bytes.

pub mod asset;
pub mod change;
pub mod collection;
pub mod error;
pub mod export;
pub mod identity;
pub mod lifecycle;
pub mod local;
pub mod ordering;
pub mod relation;
pub mod sticker;
pub mod sync;
pub mod tag;
pub mod version;
