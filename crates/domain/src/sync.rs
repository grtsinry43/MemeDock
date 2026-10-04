//! Versioned sync contracts only; no HTTP client, credentials or sync runtime.
use crate::{
    asset::Asset,
    change::Operation,
    collection::Collection,
    error::DomainError,
    identity::{CollectionId, DeviceId, LibraryId, OperationId, StickerId, SyncEpoch},
    lifecycle::{EntityId, Tombstone},
    ordering::SortKey,
    relation::{CollectionItem, StickerTag},
    sticker::Sticker,
    tag::{Name, Tag},
    version::{EventSeq, ProtocolVersion, Revision, TimestampMs},
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Public device metadata; token/hash and credential storage stay on the server/platform.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    id: DeviceId,
    name: Name,
    created_at: TimestampMs,
    revoked_at: Option<TimestampMs>,
}
impl Device {
    pub fn new(id: DeviceId, name: Name, at: TimestampMs) -> Self {
        Self {
            id,
            name,
            created_at: at,
            revoked_at: None,
        }
    }
    pub const fn id(&self) -> DeviceId {
        self.id
    }
    pub fn name(&self) -> &Name {
        &self.name
    }
    pub const fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }
    pub const fn created_at(&self) -> TimestampMs {
        self.created_at
    }
    pub const fn revoked_at(&self) -> Option<TimestampMs> {
        self.revoked_at
    }
    pub fn revoke(&mut self, at: TimestampMs) {
        if self.revoked_at.is_none() {
            self.revoked_at = Some(at);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationEnvelope {
    protocol_version: ProtocolVersion,
    library_id: LibraryId,
    sync_epoch: SyncEpoch,
    device_id: DeviceId,
    op_id: OperationId,
    client_time_ms: TimestampMs,
    operation: Operation,
}
impl OperationEnvelope {
    pub fn new(
        library_id: LibraryId,
        sync_epoch: SyncEpoch,
        device_id: DeviceId,
        op_id: OperationId,
        client_time_ms: TimestampMs,
        operation: Operation,
    ) -> Self {
        Self {
            protocol_version: ProtocolVersion::V1,
            library_id,
            sync_epoch,
            device_id,
            op_id,
            client_time_ms,
            operation,
        }
    }
    pub const fn library_id(&self) -> LibraryId {
        self.library_id
    }
    pub const fn sync_epoch(&self) -> SyncEpoch {
        self.sync_epoch
    }
    pub const fn device_id(&self) -> DeviceId {
        self.device_id
    }
    pub const fn op_id(&self) -> OperationId {
        self.op_id
    }
    pub const fn client_time_ms(&self) -> TimestampMs {
        self.client_time_ms
    }
    pub fn operation(&self) -> &Operation {
        &self.operation
    }
    pub fn ensure_identity(
        &self,
        library: LibraryId,
        epoch: SyncEpoch,
        authenticated_device: DeviceId,
    ) -> Result<(), DomainError> {
        if self.library_id != library
            || self.sync_epoch != epoch
            || self.device_id != authenticated_device
        {
            return Err(DomainError::IdentityMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventKind {
    StickerCreated {
        asset: Asset,
        entity: Sticker,
    },
    StickerPatched {
        entity: Sticker,
    },
    StickerDeleted {
        entity: Tombstone,
    },
    StickerRestored {
        asset: Asset,
        entity: Sticker,
    },
    CollectionCreated {
        entity: Collection,
    },
    CollectionPatched {
        entity: Collection,
    },
    CollectionDeleted {
        entity: Tombstone,
    },
    CollectionRestored {
        entity: Collection,
    },
    TagCreated {
        entity: Tag,
    },
    TagPatched {
        entity: Tag,
    },
    TagDeleted {
        entity: Tombstone,
    },
    TagRestored {
        entity: Tag,
    },
    CollectionMembershipSet {
        entity: CollectionItem,
    },
    CollectionItemMoved {
        entity: CollectionItem,
    },
    TagMembershipSet {
        entity: StickerTag,
    },
    CollectionItemsReordered {
        collection_id: CollectionId,
        items: Vec<OrderedSticker>,
    },
    CollectionsReordered {
        items: Vec<OrderedCollection>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderedSticker {
    pub sticker_id: StickerId,
    pub sort_key: SortKey,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderedCollection {
    pub collection_id: CollectionId,
    pub sort_key: SortKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SyncEventWire", into = "SyncEventWire")]
pub struct SyncEvent {
    seq: EventSeq,
    device_id: DeviceId,
    op_id: OperationId,
    created_at: TimestampMs,
    event: EventKind,
}
#[derive(Clone, Serialize, Deserialize)]
struct SyncEventWire {
    seq: EventSeq,
    device_id: DeviceId,
    op_id: OperationId,
    created_at: TimestampMs,
    #[serde(flatten)]
    event: EventKind,
}
impl SyncEvent {
    pub fn new(
        seq: EventSeq,
        device_id: DeviceId,
        op_id: OperationId,
        created_at: TimestampMs,
        event: EventKind,
    ) -> Result<Self, DomainError> {
        let revision = Revision::from(seq);
        match &event {
            EventKind::StickerCreated { asset, entity }
            | EventKind::StickerRestored { asset, entity } => {
                if entity.id() != asset.sticker_id() {
                    return Err(DomainError::IdentityMismatch);
                }
                entity
                    .lifecycle()
                    .ensure_active(entity.lifecycle().generation())?;
                if entity.lifecycle().revision() != revision {
                    return Err(DomainError::StaleRevision);
                }
            }
            EventKind::StickerPatched { entity } => {
                entity
                    .lifecycle()
                    .ensure_active(entity.lifecycle().generation())?;
                if entity.lifecycle().revision() != revision {
                    return Err(DomainError::StaleRevision);
                }
            }
            EventKind::CollectionCreated { entity }
            | EventKind::CollectionPatched { entity }
            | EventKind::CollectionRestored { entity } => {
                entity
                    .lifecycle()
                    .ensure_active(entity.lifecycle().generation())?;
                if entity.lifecycle().revision() != revision {
                    return Err(DomainError::StaleRevision);
                }
            }
            EventKind::TagCreated { entity }
            | EventKind::TagPatched { entity }
            | EventKind::TagRestored { entity } => {
                entity
                    .lifecycle()
                    .ensure_active(entity.lifecycle().generation())?;
                if entity.lifecycle().revision() != revision {
                    return Err(DomainError::StaleRevision);
                }
            }
            EventKind::StickerDeleted { entity } => {
                if !matches!(entity.entity(), EntityId::Sticker(_)) {
                    return Err(DomainError::IdentityMismatch);
                }
                if entity.revision() != revision {
                    return Err(DomainError::StaleRevision);
                }
            }
            EventKind::CollectionDeleted { entity } => {
                if !matches!(entity.entity(), EntityId::Collection(_)) {
                    return Err(DomainError::IdentityMismatch);
                }
                if entity.revision() != revision {
                    return Err(DomainError::StaleRevision);
                }
            }
            EventKind::TagDeleted { entity } => {
                if !matches!(entity.entity(), EntityId::Tag(_)) {
                    return Err(DomainError::IdentityMismatch);
                }
                if entity.revision() != revision {
                    return Err(DomainError::StaleRevision);
                }
            }
            EventKind::CollectionMembershipSet { entity }
            | EventKind::CollectionItemMoved { entity } => {
                if entity.revision() != revision {
                    return Err(DomainError::StaleRevision);
                }
                if matches!(&event, EventKind::CollectionItemMoved { .. }) && !entity.present() {
                    return Err(DomainError::InvalidTransition);
                }
            }
            EventKind::TagMembershipSet { entity } => {
                if entity.revision() != revision {
                    return Err(DomainError::StaleRevision);
                }
            }
            EventKind::CollectionItemsReordered { items, .. } => {
                let mut ids = HashSet::new();
                if items.iter().any(|item| !ids.insert(item.sticker_id))
                    || items.windows(2).any(|w| w[0].sort_key >= w[1].sort_key)
                {
                    return Err(DomainError::InvalidValue("reorder"));
                }
            }
            EventKind::CollectionsReordered { items } => {
                let mut ids = HashSet::new();
                if items.iter().any(|item| !ids.insert(item.collection_id))
                    || items.windows(2).any(|w| w[0].sort_key >= w[1].sort_key)
                {
                    return Err(DomainError::InvalidValue("reorder"));
                }
            }
        }
        Ok(Self {
            seq,
            device_id,
            op_id,
            created_at,
            event,
        })
    }
    pub const fn seq(&self) -> EventSeq {
        self.seq
    }
    pub const fn device_id(&self) -> DeviceId {
        self.device_id
    }
    pub const fn op_id(&self) -> OperationId {
        self.op_id
    }
    pub const fn created_at(&self) -> TimestampMs {
        self.created_at
    }
    pub fn event(&self) -> &EventKind {
        &self.event
    }
}
impl TryFrom<SyncEventWire> for SyncEvent {
    type Error = DomainError;
    fn try_from(w: SyncEventWire) -> Result<Self, Self::Error> {
        Self::new(w.seq, w.device_id, w.op_id, w.created_at, w.event)
    }
}
impl From<SyncEvent> for SyncEventWire {
    fn from(w: SyncEvent) -> Self {
        Self {
            seq: w.seq,
            device_id: w.device_id,
            op_id: w.op_id,
            created_at: w.created_at,
            event: w.event,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionCode {
    EntityDeleted,
    StaleGeneration,
    EntityMissing,
    StaleRevision,
    OpIdReused,
    InvalidOperation,
    LibraryMismatch,
    EpochMismatch,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionNote {
    AnchorMissing,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReceiptOutcome {
    Accepted {
        event_seq: EventSeq,
        note: Option<ResolutionNote>,
    },
    NoOp {
        canonical_revision: Revision,
    },
    Rejected {
        code: RejectionCode,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationReceipt {
    pub device_id: DeviceId,
    pub op_id: OperationId,
    #[serde(flatten)]
    pub outcome: ReceiptOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PullPageWire", into = "PullPageWire")]
pub struct PullPage {
    protocol_version: ProtocolVersion,
    library_id: LibraryId,
    sync_epoch: SyncEpoch,
    next_cursor: Revision,
    has_more: bool,
    events: Vec<SyncEvent>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PullPageWire {
    protocol_version: ProtocolVersion,
    library_id: LibraryId,
    sync_epoch: SyncEpoch,
    next_cursor: Revision,
    has_more: bool,
    events: Vec<SyncEvent>,
}
impl PullPage {
    pub fn new(
        library_id: LibraryId,
        sync_epoch: SyncEpoch,
        after: Revision,
        events: Vec<SyncEvent>,
        has_more: bool,
    ) -> Result<Self, DomainError> {
        let next_cursor = events.last().map_or(after, |event| event.seq.into());
        let page = Self::try_from(PullPageWire {
            protocol_version: ProtocolVersion::V1,
            library_id,
            sync_epoch,
            next_cursor,
            has_more,
            events,
        })?;
        page.validate_after(after)?;
        Ok(page)
    }
    pub const fn library_id(&self) -> LibraryId {
        self.library_id
    }
    pub const fn sync_epoch(&self) -> SyncEpoch {
        self.sync_epoch
    }
    pub const fn next_cursor(&self) -> Revision {
        self.next_cursor
    }
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
    pub fn events(&self) -> &[SyncEvent] {
        &self.events
    }
    /// Must be called on a received page with the actual request cursor/identity.
    pub fn validate_after(&self, after: Revision) -> Result<(), DomainError> {
        if self
            .events
            .first()
            .is_some_and(|event| Revision::from(event.seq) <= after)
            || (self.events.is_empty() && self.next_cursor != after)
        {
            return Err(DomainError::StaleRevision);
        }
        Ok(())
    }
    pub fn ensure_identity(&self, library: LibraryId, epoch: SyncEpoch) -> Result<(), DomainError> {
        if self.library_id != library || self.sync_epoch != epoch {
            return Err(DomainError::IdentityMismatch);
        }
        Ok(())
    }
}
impl TryFrom<PullPageWire> for PullPage {
    type Error = DomainError;
    fn try_from(w: PullPageWire) -> Result<Self, Self::Error> {
        if w.events
            .windows(2)
            .any(|events| events[0].seq >= events[1].seq)
            || w.events
                .last()
                .is_some_and(|e| Revision::from(e.seq) != w.next_cursor)
            || (w.events.is_empty() && w.has_more)
        {
            return Err(DomainError::InvalidValue("event_page"));
        }
        Ok(Self {
            protocol_version: w.protocol_version,
            library_id: w.library_id,
            sync_epoch: w.sync_epoch,
            next_cursor: w.next_cursor,
            has_more: w.has_more,
            events: w.events,
        })
    }
}
impl From<PullPage> for PullPageWire {
    fn from(w: PullPage) -> Self {
        Self {
            protocol_version: w.protocol_version,
            library_id: w.library_id,
            sync_epoch: w.sync_epoch,
            next_cursor: w.next_cursor,
            has_more: w.has_more,
            events: w.events,
        }
    }
}
