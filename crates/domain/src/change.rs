use crate::{
    asset::Asset,
    error::DomainError,
    identity::{CollectionId, StickerId, TagId},
    tag::Name,
    version::{Generation, Revision},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Missing leaves the field unchanged; null is explicit Clear, not Missing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FieldPatch<T> {
    #[default]
    Missing,
    Set(T),
    Clear,
}
impl<T> FieldPatch<T> {
    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}
impl<T: Serialize> Serialize for FieldPatch<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Set(value) => value.serialize(s),
            Self::Clear => s.serialize_none(),
            Self::Missing => Err(serde::ser::Error::custom("missing patch must be omitted")),
        }
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for FieldPatch<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(d)? {
            Some(value) => Self::Set(value),
            None => Self::Clear,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "StickerPatchWire", into = "StickerPatchWire")]
pub struct StickerPatch {
    title: FieldPatch<String>,
    note: FieldPatch<String>,
    starred: FieldPatch<bool>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StickerPatchWire {
    #[serde(default, skip_serializing_if = "FieldPatch::is_missing")]
    title: FieldPatch<String>,
    #[serde(default, skip_serializing_if = "FieldPatch::is_missing")]
    note: FieldPatch<String>,
    #[serde(default, skip_serializing_if = "FieldPatch::is_missing")]
    starred: FieldPatch<bool>,
}
impl StickerPatch {
    pub fn new(
        title: FieldPatch<String>,
        note: FieldPatch<String>,
        starred: FieldPatch<bool>,
    ) -> Result<Self, DomainError> {
        let patch = Self {
            title,
            note,
            starred,
        };
        patch.validate()?;
        Ok(patch)
    }
    pub fn title(&self) -> &FieldPatch<String> {
        &self.title
    }
    pub fn note(&self) -> &FieldPatch<String> {
        &self.note
    }
    pub fn starred(&self) -> &FieldPatch<bool> {
        &self.starred
    }
    pub fn validate(&self) -> Result<(), DomainError> {
        if matches!(self.title, FieldPatch::Clear) {
            return Err(DomainError::InvalidPatch("title"));
        }
        if matches!(self.note, FieldPatch::Clear) {
            return Err(DomainError::InvalidPatch("note"));
        }
        if matches!(self.starred, FieldPatch::Clear) {
            return Err(DomainError::InvalidPatch("starred"));
        }
        if self.title.is_missing() && self.note.is_missing() && self.starred.is_missing() {
            return Err(DomainError::InvalidPatch("empty"));
        }
        Ok(())
    }
}
impl TryFrom<StickerPatchWire> for StickerPatch {
    type Error = DomainError;
    fn try_from(value: StickerPatchWire) -> Result<Self, Self::Error> {
        Self::new(value.title, value.note, value.starred)
    }
}
impl From<StickerPatch> for StickerPatchWire {
    fn from(value: StickerPatch) -> Self {
        Self {
            title: value.title,
            note: value.note,
            starred: value.starred,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamePatch {
    name: Name,
}
impl NamePatch {
    pub fn new(name: Name) -> Self {
        Self { name }
    }
    pub fn name(&self) -> &Name {
        &self.name
    }
}

/// Typed operation payloads; the wrapper validates cross-field invariants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationKind {
    /// Current local restore audit; never replay imported historical outbox entries.
    ImportArchive {
        archive_hash: crate::identity::ContentHash,
        state: Box<crate::archive::ArchiveData>,
    },
    CreateSticker {
        asset: Asset,
        title: String,
        original_name: String,
        note: String,
        starred: bool,
    },
    PatchSticker {
        sticker_id: StickerId,
        generation: Generation,
        patch: StickerPatch,
    },
    DeleteSticker {
        sticker_id: StickerId,
        generation: Generation,
    },
    RestoreSticker {
        sticker_id: StickerId,
        expected_deleted_revision: Revision,
        rebuild: Option<StickerRebuild>,
    },
    CreateCollection {
        collection_id: CollectionId,
        name: Name,
        before_id: Option<CollectionId>,
    },
    PatchCollection {
        collection_id: CollectionId,
        generation: Generation,
        patch: NamePatch,
    },
    MoveCollection {
        collection_id: CollectionId,
        generation: Generation,
        before_id: Option<CollectionId>,
    },
    DeleteCollection {
        collection_id: CollectionId,
        generation: Generation,
    },
    RestoreCollection {
        collection_id: CollectionId,
        expected_deleted_revision: Revision,
        rebuild: Option<CollectionRebuild>,
    },
    CreateTag {
        tag_id: TagId,
        name: Name,
    },
    PatchTag {
        tag_id: TagId,
        generation: Generation,
        patch: NamePatch,
    },
    DeleteTag {
        tag_id: TagId,
        generation: Generation,
    },
    RestoreTag {
        tag_id: TagId,
        expected_deleted_revision: Revision,
        rebuild: Option<Name>,
    },
    SetStickerCollection {
        sticker_id: StickerId,
        sticker_generation: Generation,
        collection: Option<(CollectionId, Generation)>,
    },
    MoveCollectionItem {
        collection_id: CollectionId,
        sticker_id: StickerId,
        collection_generation: Generation,
        sticker_generation: Generation,
        before_id: Option<StickerId>,
    },
    SetTagMembership {
        sticker_id: StickerId,
        tag_id: TagId,
        sticker_generation: Generation,
        tag_generation: Generation,
        present: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StickerRebuild {
    pub asset: Asset,
    pub title: String,
    pub original_name: String,
    pub note: String,
    pub starred: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionRebuild {
    pub name: Name,
    pub before_id: Option<CollectionId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "OperationKind", into = "OperationKind")]
pub struct Operation(OperationKind);
impl Operation {
    pub fn new(kind: OperationKind) -> Result<Self, DomainError> {
        match &kind {
            OperationKind::ImportArchive { state, .. } => state.validate()?,
            OperationKind::CreateCollection {
                collection_id,
                before_id: Some(before),
                ..
            }
            | OperationKind::MoveCollection {
                collection_id,
                before_id: Some(before),
                ..
            } if collection_id == before => return Err(DomainError::IdentityMismatch),
            OperationKind::RestoreCollection {
                collection_id,
                rebuild: Some(rebuild),
                ..
            } if rebuild.before_id == Some(*collection_id) => {
                return Err(DomainError::IdentityMismatch);
            }
            OperationKind::MoveCollectionItem {
                sticker_id,
                before_id: Some(before),
                ..
            } if sticker_id == before => return Err(DomainError::IdentityMismatch),
            OperationKind::RestoreSticker {
                sticker_id,
                rebuild: Some(rebuild),
                ..
            } if *sticker_id != rebuild.asset.sticker_id() => {
                return Err(DomainError::IdentityMismatch);
            }
            _ => {}
        }
        Ok(Self(kind))
    }
    pub fn kind(&self) -> &OperationKind {
        &self.0
    }
}
impl TryFrom<OperationKind> for Operation {
    type Error = DomainError;
    fn try_from(value: OperationKind) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<Operation> for OperationKind {
    fn from(value: Operation) -> Self {
        value.0
    }
}
