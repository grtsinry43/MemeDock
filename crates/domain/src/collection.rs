use crate::{
    change::NamePatch,
    error::DomainError,
    identity::CollectionId,
    lifecycle::Lifecycle,
    ordering::SortKey,
    tag::Name,
    version::{Generation, Revision, TimestampMs},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    id: CollectionId,
    name: Name,
    sort_key: SortKey,
    #[serde(flatten)]
    lifecycle: Lifecycle,
}
impl Collection {
    pub fn new(id: CollectionId, name: Name, sort_key: SortKey, at: TimestampMs) -> Self {
        Self {
            id,
            name,
            sort_key,
            lifecycle: Lifecycle::new(at),
        }
    }
    pub fn from_state(
        id: CollectionId,
        name: Name,
        sort_key: SortKey,
        lifecycle: Lifecycle,
    ) -> Self {
        Self {
            id,
            name,
            sort_key,
            lifecycle,
        }
    }
    pub const fn id(&self) -> CollectionId {
        self.id
    }
    pub fn name(&self) -> &Name {
        &self.name
    }
    pub fn sort_key(&self) -> &SortKey {
        &self.sort_key
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
    /// Position intent is resolved by core/server before applying a canonical key.
    pub fn set_sort_key(
        &mut self,
        observed: Generation,
        key: SortKey,
        at: TimestampMs,
        revision: Revision,
    ) -> Result<(), DomainError> {
        self.lifecycle.ensure_active(observed)?;
        self.lifecycle.touch(at, revision)?;
        self.sort_key = key;
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
