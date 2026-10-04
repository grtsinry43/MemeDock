use std::{error::Error, fmt, io};

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug)]
pub enum StorageError {
    Database(sea_orm::DbErr),
    InvalidData {
        table: &'static str,
        source: serde_json::Error,
    },
    Domain(memedock_domain::error::DomainError),
    Io(io::Error),
    Cleanup {
        source: Box<StorageError>,
        cleanup: io::Error,
    },
    UnsupportedSchema,
    InvalidInput(&'static str),
    Conflict(&'static str),
    Integrity(&'static str),
    Cancelled,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(e) => write!(f, "database: {e}"),
            Self::InvalidData { table, source } => write!(f, "invalid {table} row: {source}"),
            Self::Domain(e) => write!(f, "domain: {e}"),
            Self::Io(e) => write!(f, "filesystem: {e}"),
            Self::Cleanup { source, cleanup } => {
                write!(f, "{source}; staging cleanup also failed: {cleanup}")
            }
            Self::UnsupportedSchema => f.write_str("unsupported database schema"),
            Self::InvalidInput(s) => write!(f, "invalid input: {s}"),
            Self::Conflict(s) => write!(f, "conflict: {s}"),
            Self::Integrity(s) => write!(f, "integrity failure: {s}"),
            Self::Cancelled => f.write_str("operation cancelled"),
        }
    }
}
impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(e) => Some(e),
            Self::InvalidData { source, .. } => Some(source),
            Self::Domain(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Cleanup { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
impl From<sea_orm::DbErr> for StorageError {
    fn from(e: sea_orm::DbErr) -> Self {
        Self::Database(e)
    }
}
impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<memedock_domain::error::DomainError> for StorageError {
    fn from(e: memedock_domain::error::DomainError) -> Self {
        Self::Domain(e)
    }
}
