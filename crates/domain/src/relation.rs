use crate::{
    collection::Collection,
    error::DomainError,
    identity::{CollectionId, StickerId, TagId},
    ordering::SortKey,
    sticker::Sticker,
    tag::Tag,
    version::{Generation, Revision, TimestampMs},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionItem {
    collection_id: CollectionId,
    sticker_id: StickerId,
    collection_generation: Generation,
    sticker_generation: Generation,
    present: bool,
    sort_key: SortKey,
    revision: Revision,
    updated_at: TimestampMs,
}
impl CollectionItem {
    pub fn new(
        collection: &Collection,
        sticker: &Sticker,
        sort_key: SortKey,
        present: bool,
        revision: Revision,
        at: TimestampMs,
    ) -> Result<Self, DomainError> {
        collection
            .lifecycle()
            .ensure_active(collection.lifecycle().generation())?;
        sticker
            .lifecycle()
            .ensure_active(sticker.lifecycle().generation())?;
        Ok(Self {
            collection_id: collection.id(),
            sticker_id: sticker.id(),
            collection_generation: collection.lifecycle().generation(),
            sticker_generation: sticker.lifecycle().generation(),
            present,
            sort_key,
            revision,
            updated_at: at,
        })
    }
    /// Historical rows may reference deleted endpoints or an earlier generation.
    pub fn from_state(
        collection: (CollectionId, Generation),
        sticker: (StickerId, Generation),
        present: bool,
        sort_key: SortKey,
        revision: Revision,
        updated_at: TimestampMs,
    ) -> Self {
        Self {
            collection_id: collection.0,
            collection_generation: collection.1,
            sticker_id: sticker.0,
            sticker_generation: sticker.1,
            present,
            sort_key,
            revision,
            updated_at,
        }
    }
    pub const fn collection_id(&self) -> CollectionId {
        self.collection_id
    }
    pub const fn sticker_id(&self) -> StickerId {
        self.sticker_id
    }
    pub const fn collection_generation(&self) -> Generation {
        self.collection_generation
    }
    pub const fn sticker_generation(&self) -> Generation {
        self.sticker_generation
    }
    pub const fn present(&self) -> bool {
        self.present
    }
    pub fn sort_key(&self) -> &SortKey {
        &self.sort_key
    }
    pub const fn revision(&self) -> Revision {
        self.revision
    }
    pub const fn updated_at(&self) -> TimestampMs {
        self.updated_at
    }
    pub fn is_effective(&self, collection: &Collection, sticker: &Sticker) -> bool {
        self.present
            && collection.id() == self.collection_id
            && sticker.id() == self.sticker_id
            && collection.lifecycle().is_active()
            && sticker.lifecycle().is_active()
            && collection.lifecycle().generation() == self.collection_generation
            && sticker.lifecycle().generation() == self.sticker_generation
    }
    /// Changing an existing relationship cannot accidentally rebase old-generation work.
    pub fn set_present(
        &mut self,
        collection: &Collection,
        sticker: &Sticker,
        present: bool,
        revision: Revision,
        at: TimestampMs,
    ) -> Result<(), DomainError> {
        self.ensure_endpoints(collection, sticker)?;
        if revision < self.revision {
            return Err(DomainError::StaleRevision);
        }
        self.present = present;
        self.revision = revision;
        self.updated_at = at;
        Ok(())
    }
    pub fn move_to(
        &mut self,
        collection: &Collection,
        sticker: &Sticker,
        sort_key: SortKey,
        revision: Revision,
        at: TimestampMs,
    ) -> Result<(), DomainError> {
        self.ensure_endpoints(collection, sticker)?;
        if !self.present {
            return Err(DomainError::InvalidTransition);
        }
        if revision < self.revision {
            return Err(DomainError::StaleRevision);
        }
        self.sort_key = sort_key;
        self.revision = revision;
        self.updated_at = at;
        Ok(())
    }
    fn ensure_endpoints(
        &self,
        collection: &Collection,
        sticker: &Sticker,
    ) -> Result<(), DomainError> {
        if collection.id() != self.collection_id || sticker.id() != self.sticker_id {
            return Err(DomainError::IdentityMismatch);
        }
        collection
            .lifecycle()
            .ensure_active(self.collection_generation)?;
        sticker.lifecycle().ensure_active(self.sticker_generation)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StickerTag {
    sticker_id: StickerId,
    tag_id: TagId,
    sticker_generation: Generation,
    tag_generation: Generation,
    present: bool,
    revision: Revision,
    updated_at: TimestampMs,
}
impl StickerTag {
    pub fn new(
        sticker: &Sticker,
        tag: &Tag,
        present: bool,
        revision: Revision,
        at: TimestampMs,
    ) -> Result<Self, DomainError> {
        sticker
            .lifecycle()
            .ensure_active(sticker.lifecycle().generation())?;
        tag.lifecycle()
            .ensure_active(tag.lifecycle().generation())?;
        Ok(Self {
            sticker_id: sticker.id(),
            tag_id: tag.id(),
            sticker_generation: sticker.lifecycle().generation(),
            tag_generation: tag.lifecycle().generation(),
            present,
            revision,
            updated_at: at,
        })
    }
    pub fn from_state(
        sticker: (StickerId, Generation),
        tag: (TagId, Generation),
        present: bool,
        revision: Revision,
        updated_at: TimestampMs,
    ) -> Self {
        Self {
            sticker_id: sticker.0,
            sticker_generation: sticker.1,
            tag_id: tag.0,
            tag_generation: tag.1,
            present,
            revision,
            updated_at,
        }
    }
    pub const fn sticker_id(&self) -> StickerId {
        self.sticker_id
    }
    pub const fn tag_id(&self) -> TagId {
        self.tag_id
    }
    pub const fn sticker_generation(&self) -> Generation {
        self.sticker_generation
    }
    pub const fn tag_generation(&self) -> Generation {
        self.tag_generation
    }
    pub const fn present(&self) -> bool {
        self.present
    }
    pub const fn revision(&self) -> Revision {
        self.revision
    }
    pub const fn updated_at(&self) -> TimestampMs {
        self.updated_at
    }
    pub fn is_effective(&self, sticker: &Sticker, tag: &Tag) -> bool {
        self.present
            && sticker.id() == self.sticker_id
            && tag.id() == self.tag_id
            && sticker.lifecycle().is_active()
            && tag.lifecycle().is_active()
            && sticker.lifecycle().generation() == self.sticker_generation
            && tag.lifecycle().generation() == self.tag_generation
    }
    pub fn set_present(
        &mut self,
        sticker: &Sticker,
        tag: &Tag,
        present: bool,
        revision: Revision,
        at: TimestampMs,
    ) -> Result<(), DomainError> {
        if sticker.id() != self.sticker_id || tag.id() != self.tag_id {
            return Err(DomainError::IdentityMismatch);
        }
        sticker.lifecycle().ensure_active(self.sticker_generation)?;
        tag.lifecycle().ensure_active(self.tag_generation)?;
        if revision < self.revision {
            return Err(DomainError::StaleRevision);
        }
        self.present = present;
        self.revision = revision;
        self.updated_at = at;
        Ok(())
    }
}
