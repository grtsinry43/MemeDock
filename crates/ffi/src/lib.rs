//! Owned Kotlin boundary for existing core use cases. No database access or
//! platform lifecycle decisions belong to the production FFI layer.
mod config;
mod cursor;
mod dto;
mod error;
mod import;
mod library;
mod streams;
mod tasks;

pub use config::*;
pub use cursor::*;
pub use dto::*;
pub use error::*;
pub use import::*;
pub use library::*;
pub use streams::*;
pub use tasks::*;

uniffi::setup_scaffolding!();
