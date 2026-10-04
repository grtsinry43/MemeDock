use crate::error::DomainError;
use serde::{Deserialize, Serialize};

// SQLite signed integers bound persisted counters even though they are nonnegative.
macro_rules! counter {
    ($name:ident, $minimum:expr) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(try_from = "i64", into = "i64")]
        pub struct $name(i64);
        impl $name {
            pub fn new(value: i64) -> Result<Self, DomainError> {
                if value < $minimum {
                    return Err(DomainError::InvalidValue(stringify!($name)));
                }
                Ok(Self(value))
            }
            pub const fn get(self) -> i64 {
                self.0
            }
            pub fn checked_next(self) -> Result<Self, DomainError> {
                self.0
                    .checked_add(1)
                    .map(Self)
                    .ok_or(DomainError::VersionOverflow)
            }
        }
        impl TryFrom<i64> for $name {
            type Error = DomainError;
            fn try_from(value: i64) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
        impl From<$name> for i64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}
counter!(Generation, 0);
counter!(Revision, 0);
counter!(LocalOrder, 1);
counter!(EventSeq, 1);
counter!(ByteSize, 0);
counter!(UseCount, 0);
impl UseCount {
    pub const FIRST: Self = Self(1);
}
impl Generation {
    pub const INITIAL: Self = Self(0);
}
impl Revision {
    pub const LOCAL: Self = Self(0);
}
impl From<EventSeq> for Revision {
    fn from(value: EventSeq) -> Self {
        Self(value.get())
    }
}

/// UTC Unix milliseconds, including valid instants before the epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimestampMs(i64);
impl TimestampMs {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> i64 {
        self.0
    }
}

macro_rules! version {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "u32", into = "u32")]
        pub struct $name(u32);
        impl $name {
            pub const V1: Self = Self(1);
            pub const fn get(self) -> u32 {
                self.0
            }
        }
        impl TryFrom<u32> for $name {
            type Error = DomainError;
            fn try_from(value: u32) -> Result<Self, Self::Error> {
                if value == 1 {
                    Ok(Self::V1)
                } else {
                    Err(DomainError::UnsupportedVersion)
                }
            }
        }
        impl From<$name> for u32 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}
version!(ProtocolVersion);
version!(ChangeSchemaVersion);
version!(ArchiveFormatVersion);
