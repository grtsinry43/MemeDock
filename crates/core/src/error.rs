use memedock_storage::StorageError;
use std::{error::Error, fmt, io, sync::Arc};

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidInput,
    UnsupportedFormat,
    UnsupportedColorProfile,
    InvalidImage,
    ResourceLimit,
    AlreadyOpen,
    Closed,
    Busy,
    Cancelled,
    NotFound,
    EntityDeleted,
    Conflict,
    UnsupportedSchema,
    CorruptData,
    PermissionDenied,
    StorageFull,
    Io,
    Database,
    Internal,
}
impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::UnsupportedFormat => "unsupported_format",
            Self::UnsupportedColorProfile => "unsupported_color_profile",
            Self::InvalidImage => "invalid_image",
            Self::ResourceLimit => "resource_limit",
            Self::AlreadyOpen => "already_open",
            Self::Closed => "closed",
            Self::Busy => "busy",
            Self::Cancelled => "cancelled",
            Self::NotFound => "not_found",
            Self::EntityDeleted => "entity_deleted",
            Self::Conflict => "conflict",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::CorruptData => "corrupt_data",
            Self::PermissionDenied => "permission_denied",
            Self::StorageFull => "storage_full",
            Self::Io => "io",
            Self::Database => "database",
            Self::Internal => "internal",
        }
    }
}
/// Stable public code/message, with a diagnostic source chain for internal logs.
#[derive(Clone, Debug)]
pub struct CoreError {
    code: ErrorCode,
    message: &'static str,
    source: Option<Arc<dyn Error + Send + Sync>>,
}
impl CoreError {
    pub fn code(&self) -> ErrorCode {
        self.code
    }
    pub fn message(&self) -> &'static str {
        self.message
    }
    pub(crate) fn new(code: ErrorCode, message: &'static str) -> Self {
        Self {
            code,
            message,
            source: None,
        }
    }
    pub(crate) fn caused(
        code: ErrorCode,
        message: &'static str,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            code,
            message,
            source: Some(Arc::new(source)),
        }
    }
    pub(crate) fn internal(message: &'static str) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
}
impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}
impl Error for CoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn Error + 'static))
    }
}
impl From<io::Error> for CoreError {
    fn from(e: io::Error) -> Self {
        let code = match e.kind() {
            io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
            io::ErrorKind::StorageFull => ErrorCode::StorageFull,
            io::ErrorKind::NotFound => ErrorCode::NotFound,
            _ => ErrorCode::Io,
        };
        Self::caused(code, "filesystem operation failed", e)
    }
}
impl From<memedock_domain::error::DomainError> for CoreError {
    fn from(e: memedock_domain::error::DomainError) -> Self {
        use memedock_domain::error::DomainError::*;
        let code = match e {
            EntityDeleted => ErrorCode::EntityDeleted,
            StaleGeneration | StaleRevision | OperationFrozen | EntityActive
            | InvalidTransition => ErrorCode::Conflict,
            _ => ErrorCode::InvalidInput,
        };
        Self::caused(code, "domain operation rejected", e)
    }
}
impl From<StorageError> for CoreError {
    fn from(e: StorageError) -> Self {
        let code = match &e {
            StorageError::Io(io) => match io.kind() {
                io::ErrorKind::NotFound => ErrorCode::NotFound,
                io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
                io::ErrorKind::StorageFull => ErrorCode::StorageFull,
                _ => ErrorCode::Io,
            },
            StorageError::Database(_) => ErrorCode::Database,
            StorageError::UnsupportedSchema => ErrorCode::UnsupportedSchema,
            StorageError::InvalidData { .. } | StorageError::Integrity(_) => ErrorCode::CorruptData,
            StorageError::InvalidInput(_) => ErrorCode::InvalidInput,
            StorageError::Conflict(_) => ErrorCode::Conflict,
            StorageError::Cancelled => ErrorCode::Cancelled,
            StorageError::Domain(e) => return e.clone().into(),
            StorageError::Cleanup { .. } => ErrorCode::Io,
        };
        Self::caused(code, "storage operation failed", e)
    }
}
impl From<tokio::task::JoinError> for CoreError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::caused(ErrorCode::Internal, "worker failed", e)
    }
}
