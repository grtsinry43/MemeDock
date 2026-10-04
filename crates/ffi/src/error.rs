use std::{error::Error, fmt};

pub type Result<T> = std::result::Result<T, BridgeError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ErrorCode {
    InvalidInput,
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
impl From<memedock_core::ErrorCode> for ErrorCode {
    fn from(code: memedock_core::ErrorCode) -> Self {
        use memedock_core::ErrorCode as C;
        match code {
            C::InvalidInput => Self::InvalidInput,
            C::AlreadyOpen => Self::AlreadyOpen,
            C::Closed => Self::Closed,
            C::Busy => Self::Busy,
            C::Cancelled => Self::Cancelled,
            C::NotFound => Self::NotFound,
            C::EntityDeleted => Self::EntityDeleted,
            C::Conflict => Self::Conflict,
            C::UnsupportedSchema => Self::UnsupportedSchema,
            C::CorruptData => Self::CorruptData,
            C::PermissionDenied => Self::PermissionDenied,
            C::StorageFull => Self::StorageFull,
            C::Io => Self::Io,
            C::Database => Self::Database,
            C::Internal => Self::Internal,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Error)]
pub enum BridgeError {
    Failure { code: ErrorCode, detail: String },
}
impl BridgeError {
    pub(crate) fn new(code: ErrorCode, message: &'static str) -> Self {
        Self::Failure {
            code,
            detail: message.into(),
        }
    }
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Failure { code, .. } => *code,
        }
    }
}
impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failure { code, detail } => write!(f, "{code:?}: {detail}"),
        }
    }
}
impl Error for BridgeError {}
impl From<memedock_core::CoreError> for BridgeError {
    fn from(error: memedock_core::CoreError) -> Self {
        Self::Failure {
            code: error.code().into(),
            detail: error.message().into(),
        }
    }
}
impl From<memedock_domain::error::DomainError> for BridgeError {
    fn from(error: memedock_domain::error::DomainError) -> Self {
        memedock_core::CoreError::from(error).into()
    }
}
