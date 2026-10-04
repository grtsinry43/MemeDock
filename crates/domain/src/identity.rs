use crate::error::DomainError;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{fmt, str::FromStr};
use uuid::{Uuid, Variant};

/// Full, canonical lowercase SHA-256. Construction does not hash file bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash([u8; 32]);
impl ContentHash {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl FromStr for ContentHash {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64
            || !s
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(DomainError::InvalidHash);
        }
        let mut bytes = [0; 32];
        for (index, pair) in s.as_bytes().chunks_exact(2).enumerate() {
            fn nibble(b: u8) -> u8 {
                if b <= b'9' { b - b'0' } else { b - b'a' + 10 }
            }
            bytes[index] = nibble(pair[0]) * 16 + nibble(pair[1]);
        }
        Ok(Self(bytes))
    }
}
impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}
impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(de::Error::custom)
    }
}

macro_rules! uuid_id {
    ($name:ident, $v7:expr) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Uuid);
        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl FromStr for $name {
            type Err = DomainError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let id =
                    Uuid::parse_str(s).map_err(|_| DomainError::InvalidId(stringify!($name)))?;
                if id.is_nil()
                    || id.get_variant() != Variant::RFC4122
                    || ($v7 && id.get_version_num() != 7)
                {
                    return Err(DomainError::InvalidId(stringify!($name)));
                }
                Ok(Self(id))
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                String::deserialize(d)?.parse().map_err(de::Error::custom)
            }
        }
    };
}
uuid_id!(CollectionId, true);
uuid_id!(TagId, true);
uuid_id!(DeviceId, true);
uuid_id!(OperationId, true);
// Library/epoch identity is UUID in the design; existing RFC UUIDs are accepted.
uuid_id!(LibraryId, false);
uuid_id!(SyncEpoch, false);

/// Sticker identity is exactly the content hash in protocol v1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StickerId(ContentHash);
impl StickerId {
    pub const fn new(hash: ContentHash) -> Self {
        Self(hash)
    }
    pub const fn content_hash(self) -> ContentHash {
        self.0
    }
}
impl fmt::Display for StickerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for StickerId {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}
