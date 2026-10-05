//! Local persistence boundaries. Call blocking file operations on core's bounded
//! blocking executor, and serialize write transactions in core's coordinator.

pub mod artifacts;
pub mod db;
mod entities;
pub mod error;
pub mod files;
mod mapping;
pub mod queries;
pub mod transaction;

pub use db::LibraryDatabase;
pub use error::{Result, StorageError};
