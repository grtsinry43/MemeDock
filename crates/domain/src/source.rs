//! Validated local provenance identifiers. Credentials never belong in these records.
use crate::{
    error::DomainError,
    identity::{CollectionId, StickerId},
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceItemId(String);
impl SourceItemId {
    pub fn new(value: String) -> Result<Self, DomainError> {
        if value.is_empty()
            || value.len() > 256
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-=".contains(&b))
        {
            return Err(DomainError::InvalidValue("source item id"));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TelegramPackName(String);
impl TelegramPackName {
    pub fn new(value: String) -> Result<Self, DomainError> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(DomainError::InvalidValue("telegram pack name"));
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcePack {
    pub name: TelegramPackName,
    pub collection: CollectionId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceItem {
    pub id: SourceItemId,
    pub sticker: StickerId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceImportState {
    Available,
    Imported,
    RestoreRequired,
    OriginalMissing,
}
