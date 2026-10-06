//! Owned library execution independent of the caller's async runtime.
//! Platforms retain a Library for the process lifetime, close it explicitly,
//! and retain/cancel individual tasks and subscriptions as needed.
mod archive;
mod artifacts;
pub mod config;
pub mod error;
pub mod events;
mod images;
mod library;
mod registry;
mod runtime;
pub mod tasks;
mod use_cases;
mod writes;

pub use archive::{ArchiveInput, BackupFile, PreparedArchive};
pub use artifacts::{ArtifactLease, ExportArtifact};
pub use config::{LibraryConfig, ResourceLimits};
pub use error::{CoreError, ErrorCode, Result};
pub use library::{Library, LibraryState};
pub use memedock_domain::archive::{RestoreMode, RestoreSummary};
pub use memedock_storage::db::LibraryIdentity;
pub use memedock_storage::queries::{SpaceStatistics, StickerQuery, StickerSort};
pub use use_cases::archive_restore::RestoreResult;
pub use use_cases::import::{ImportInput, ImportOptions, ImportOutcome, ImportStatus};
pub use use_cases::maintenance::VerifiedOriginal;
pub use use_cases::query::StickerResource;
pub use use_cases::query::{QueryCursor, QueryRequest, QueryResponse, RequestId, StickerDetail};
pub use use_cases::restore::RestoreSuggestions;
pub use use_cases::thumbnail::Thumbnail;
