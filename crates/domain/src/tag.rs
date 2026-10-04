use crate::{
    change::NamePatch,
    error::DomainError,
    identity::TagId,
    lifecycle::Lifecycle,
    version::{Generation, Revision, TimestampMs},
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use unicode_normalization::UnicodeNormalization;

/// Normalization v1: NFKC, Unicode lowercase, collapse/trim whitespace.
/// Original user text is retained separately. This is not case folding/tokenization.
pub fn normalize_text(text: &str) -> String {
    text.nfkc()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Name(String);
impl Name {
    pub fn new(value: String) -> Result<Self, DomainError> {
        if normalize_text(&value).is_empty() {
            return Err(DomainError::InvalidValue("name"));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn normalized(&self) -> String {
        normalize_text(&self.0)
    }
}
impl FromStr for Name {
    type Err = DomainError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value.to_owned())
    }
}
impl TryFrom<String> for Name {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<Name> for String {
    fn from(value: Name) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "TagWire", into = "TagWire")]
pub struct Tag {
    id: TagId,
    name: Name,
    lifecycle: Lifecycle,
}
#[derive(Clone, Serialize, Deserialize)]
struct TagWire {
    id: TagId,
    name: Name,
    normalized_name: String,
    #[serde(flatten)]
    lifecycle: Lifecycle,
}
impl TryFrom<TagWire> for Tag {
    type Error = DomainError;
    fn try_from(value: TagWire) -> Result<Self, Self::Error> {
        if value.name.normalized() != value.normalized_name {
            return Err(DomainError::InvalidValue("normalized_name"));
        }
        Ok(Self {
            id: value.id,
            name: value.name,
            lifecycle: value.lifecycle,
        })
    }
}
impl From<Tag> for TagWire {
    fn from(value: Tag) -> Self {
        Self {
            id: value.id,
            normalized_name: value.name.normalized(),
            name: value.name,
            lifecycle: value.lifecycle,
        }
    }
}
impl Tag {
    pub fn new(id: TagId, name: Name, at: TimestampMs) -> Self {
        Self {
            id,
            name,
            lifecycle: Lifecycle::new(at),
        }
    }
    pub fn from_state(
        id: TagId,
        name: Name,
        normalized_name: String,
        lifecycle: Lifecycle,
    ) -> Result<Self, DomainError> {
        Self::try_from(TagWire {
            id,
            name,
            normalized_name,
            lifecycle,
        })
    }
    pub const fn id(&self) -> TagId {
        self.id
    }
    pub fn name(&self) -> &Name {
        &self.name
    }
    pub fn normalized_name(&self) -> String {
        self.name.normalized()
    }
    pub fn lifecycle(&self) -> &Lifecycle {
        &self.lifecycle
    }
    pub fn patch(
        &mut self,
        observed: Generation,
        patch: &NamePatch,
        at: TimestampMs,
        revision: Revision,
    ) -> Result<(), DomainError> {
        self.lifecycle.ensure_active(observed)?;
        self.lifecycle.touch(at, revision)?;
        self.name = patch.name().clone();
        Ok(())
    }
    pub fn delete(
        &mut self,
        observed: Generation,
        at: TimestampMs,
        revision: Revision,
    ) -> Result<bool, DomainError> {
        self.lifecycle.delete(observed, at, revision)
    }
    /// Caller verifies original availability for stickers and server authority when synced.
    pub fn restore(
        &mut self,
        expected_deleted: Revision,
        at: TimestampMs,
        revision: Revision,
    ) -> Result<(), DomainError> {
        self.lifecycle.restore(expected_deleted, at, revision)
    }
}
