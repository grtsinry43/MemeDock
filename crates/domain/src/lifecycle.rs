use crate::{
    error::DomainError,
    identity::{CollectionId, StickerId, TagId},
    version::{Generation, Revision, TimestampMs},
};
use serde::{Deserialize, Serialize};

/// Persistable lifecycle fields; timestamps are diagnostic, never conflict authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lifecycle {
    generation: Generation,
    revision: Revision,
    created_at: TimestampMs,
    updated_at: TimestampMs,
    deleted_at: Option<TimestampMs>,
}
impl Lifecycle {
    pub fn new(at: TimestampMs) -> Self {
        Self {
            generation: Generation::INITIAL,
            revision: Revision::LOCAL,
            created_at: at,
            updated_at: at,
            deleted_at: None,
        }
    }
    /// Rehydrate typed state from a current-schema database row or snapshot.
    pub fn from_state(
        generation: Generation,
        revision: Revision,
        created_at: TimestampMs,
        updated_at: TimestampMs,
        deleted_at: Option<TimestampMs>,
    ) -> Self {
        Self {
            generation,
            revision,
            created_at,
            updated_at,
            deleted_at,
        }
    }
    pub const fn generation(&self) -> Generation {
        self.generation
    }
    pub const fn revision(&self) -> Revision {
        self.revision
    }
    pub const fn created_at(&self) -> TimestampMs {
        self.created_at
    }
    pub const fn updated_at(&self) -> TimestampMs {
        self.updated_at
    }
    pub const fn deleted_at(&self) -> Option<TimestampMs> {
        self.deleted_at
    }
    pub const fn is_active(&self) -> bool {
        self.deleted_at.is_none()
    }
    pub fn ensure_active(&self, observed: Generation) -> Result<(), DomainError> {
        if !self.is_active() {
            return Err(DomainError::EntityDeleted);
        }
        if self.generation != observed {
            return Err(DomainError::StaleGeneration);
        }
        Ok(())
    }
    fn ensure_revision(&self, revision: Revision) -> Result<(), DomainError> {
        if revision < self.revision {
            Err(DomainError::StaleRevision)
        } else {
            Ok(())
        }
    }
    pub(crate) fn touch(&mut self, at: TimestampMs, revision: Revision) -> Result<(), DomainError> {
        self.ensure_revision(revision)?;
        self.updated_at = at;
        self.revision = revision;
        Ok(())
    }
    /// Repeating a deletion within the same generation has no additional effect.
    pub fn delete(
        &mut self,
        observed: Generation,
        at: TimestampMs,
        revision: Revision,
    ) -> Result<bool, DomainError> {
        if self.generation != observed {
            return Err(DomainError::StaleGeneration);
        }
        if !self.is_active() {
            return Ok(false);
        }
        self.ensure_revision(revision)?;
        self.deleted_at = Some(at);
        self.touch(at, revision)?;
        Ok(true)
    }
    /// The caller must verify blob availability and online authority when required.
    pub fn restore(
        &mut self,
        expected_deleted: Revision,
        at: TimestampMs,
        revision: Revision,
    ) -> Result<(), DomainError> {
        if self.is_active() {
            return Err(DomainError::EntityActive);
        }
        if self.revision != expected_deleted {
            return Err(DomainError::StaleRevision);
        }
        self.ensure_revision(revision)?;
        let next = self.generation.checked_next()?;
        self.generation = next;
        self.deleted_at = None;
        self.touch(at, revision)
    }
}

/// Typed IDs prevent tombstones referring to the wrong category of identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "entity_kind", content = "entity_id", rename_all = "snake_case")]
pub enum EntityId {
    Sticker(StickerId),
    Collection(CollectionId),
    Tag(TagId),
}

/// Deletion of an identity whose complete metadata may not yet exist.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    #[serde(flatten)]
    entity: EntityId,
    generation: Generation,
    revision: Revision,
    deleted_at: TimestampMs,
}
impl Tombstone {
    pub fn new(
        entity: EntityId,
        generation: Generation,
        revision: Revision,
        deleted_at: TimestampMs,
    ) -> Self {
        Self {
            entity,
            generation,
            revision,
            deleted_at,
        }
    }
    pub const fn entity(&self) -> EntityId {
        self.entity
    }
    pub const fn generation(&self) -> Generation {
        self.generation
    }
    pub const fn revision(&self) -> Revision {
        self.revision
    }
    pub const fn deleted_at(&self) -> TimestampMs {
        self.deleted_at
    }
    pub fn ensure_restore(&self, expected: Revision) -> Result<Generation, DomainError> {
        if expected != self.revision {
            return Err(DomainError::StaleRevision);
        }
        self.generation.checked_next()
    }
}
