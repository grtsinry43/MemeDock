use std::fmt;

/// Stable, non-platform-specific reasons for rejecting domain input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainError {
    InvalidHash,
    InvalidId(&'static str),
    InvalidValue(&'static str),
    UnsupportedVersion,
    InvalidSortKey,
    InvalidBounds,
    EntityDeleted,
    EntityActive,
    StaleGeneration,
    StaleRevision,
    VersionOverflow,
    InvalidPatch(&'static str),
    IdentityMismatch,
    InvalidTransition,
    OperationFrozen,
}

impl DomainError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidHash => "invalid_hash",
            Self::InvalidId(_) => "invalid_id",
            Self::InvalidValue(_) => "invalid_value",
            Self::UnsupportedVersion => "unsupported_version",
            Self::InvalidSortKey => "invalid_sort_key",
            Self::InvalidBounds => "invalid_bounds",
            Self::EntityDeleted => "entity_deleted",
            Self::EntityActive => "entity_active",
            Self::StaleGeneration => "stale_generation",
            Self::StaleRevision => "stale_revision",
            Self::VersionOverflow => "version_overflow",
            Self::InvalidPatch(_) => "invalid_patch",
            Self::IdentityMismatch => "identity_mismatch",
            Self::InvalidTransition => "invalid_transition",
            Self::OperationFrozen => "operation_frozen",
        }
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())?;
        match self {
            Self::InvalidId(field) | Self::InvalidValue(field) | Self::InvalidPatch(field) => {
                write!(f, ": {field}")
            }
            _ => Ok(()),
        }
    }
}
impl std::error::Error for DomainError {}
