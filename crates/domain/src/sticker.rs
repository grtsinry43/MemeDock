use crate::{
    asset::Asset,
    change::{FieldPatch, StickerPatch},
    error::DomainError,
    identity::StickerId,
    lifecycle::Lifecycle,
    version::{Generation, Revision, TimestampMs},
};
use serde::{Deserialize, Serialize};

/// Mutable metadata for exactly one content-addressed original in protocol v1.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sticker {
    id: StickerId,
    title: String,
    original_name: String,
    note: String,
    starred: bool,
    #[serde(flatten)]
    lifecycle: Lifecycle,
}
impl Sticker {
    pub fn new(asset: &Asset, title: String, original_name: String, at: TimestampMs) -> Self {
        Self {
            id: asset.sticker_id(),
            title,
            original_name,
            note: String::new(),
            starred: false,
            lifecycle: Lifecycle::new(at),
        }
    }
    pub fn from_state(
        id: StickerId,
        title: String,
        original_name: String,
        note: String,
        starred: bool,
        lifecycle: Lifecycle,
    ) -> Self {
        Self {
            id,
            title,
            original_name,
            note,
            starred,
            lifecycle,
        }
    }
    pub const fn id(&self) -> StickerId {
        self.id
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn original_name(&self) -> &str {
        &self.original_name
    }
    pub fn note(&self) -> &str {
        &self.note
    }
    pub const fn starred(&self) -> bool {
        self.starred
    }
    pub fn lifecycle(&self) -> &Lifecycle {
        &self.lifecycle
    }
    pub fn patch(
        &mut self,
        observed: Generation,
        patch: &StickerPatch,
        at: TimestampMs,
        revision: Revision,
    ) -> Result<(), DomainError> {
        self.lifecycle.ensure_active(observed)?;
        // Validate before any mutation so failures leave the entity unchanged.
        patch.validate()?;
        self.lifecycle.touch(at, revision)?;
        if let FieldPatch::Set(value) = patch.title() {
            self.title.clone_from(value);
        }
        if let FieldPatch::Set(value) = patch.note() {
            self.note.clone_from(value);
        }
        if let FieldPatch::Set(value) = patch.starred() {
            self.starred = *value;
        }
        Ok(())
    }
    /// Duplicate import never changes edited metadata or revives deleted content.
    pub fn classify_import(&self, asset: &Asset) -> Result<ImportDisposition, DomainError> {
        if self.id != asset.sticker_id() {
            return Err(DomainError::IdentityMismatch);
        }
        Ok(if self.lifecycle.is_active() {
            ImportDisposition::Reuse
        } else {
            ImportDisposition::RestoreRequired
        })
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportDisposition {
    Reuse,
    RestoreRequired,
}
