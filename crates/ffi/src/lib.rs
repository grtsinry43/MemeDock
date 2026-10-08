//! Owned Kotlin boundary for existing core use cases. No database access or
//! platform lifecycle decisions belong to the production FFI layer.
mod archive;
mod artifacts;
mod batch;
mod config;
mod cursor;
mod dto;
mod error;
mod export;
mod import;
mod library;
mod management;
mod streams;
mod tasks;
mod telegram;

pub use archive::*;
pub use artifacts::*;
pub use batch::*;
pub use config::*;
pub use cursor::*;
pub use dto::*;
pub use error::*;
pub use export::*;
pub use import::*;
pub use library::*;
pub use management::*;
pub use streams::*;
pub use tasks::*;
pub use telegram::*;

uniffi::setup_scaffolding!();
