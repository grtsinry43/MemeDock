use crate::{
    change::Operation,
    error::DomainError,
    identity::{ContentHash, OperationId, StickerId},
    version::{ChangeSchemaVersion, EventSeq, LocalOrder, Revision, TimestampMs},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlobStatus {
    Missing,
    Downloading,
    Ready,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThumbnailStatus {
    Missing,
    Generating,
    Ready,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalAsset {
    hash: ContentHash,
    blob_status: BlobStatus,
    thumb_status: ThumbnailStatus,
    last_error: Option<String>,
    verified_at: Option<TimestampMs>,
}
impl LocalAsset {
    pub fn new(hash: ContentHash) -> Self {
        Self {
            hash,
            blob_status: BlobStatus::Missing,
            thumb_status: ThumbnailStatus::Missing,
            last_error: None,
            verified_at: None,
        }
    }
    pub const fn hash(&self) -> ContentHash {
        self.hash
    }
    pub const fn blob_status(&self) -> BlobStatus {
        self.blob_status
    }
    pub const fn thumb_status(&self) -> ThumbnailStatus {
        self.thumb_status
    }
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
    pub const fn verified_at(&self) -> Option<TimestampMs> {
        self.verified_at
    }
    pub fn begin_download(&mut self) {
        self.blob_status = BlobStatus::Downloading;
        self.last_error = None;
    }
    /// Call only after whole-file verification and atomic publication succeed.
    pub fn verified(&mut self, at: TimestampMs) {
        self.blob_status = BlobStatus::Ready;
        self.verified_at = Some(at);
        self.last_error = None;
    }
    pub fn blob_failed(&mut self, error_code: String) {
        self.blob_status = BlobStatus::Failed;
        self.last_error = Some(error_code);
    }
    pub fn mark_missing(&mut self) {
        self.blob_status = BlobStatus::Missing;
    }
    pub fn begin_thumbnail(&mut self) -> Result<(), DomainError> {
        if self.blob_status != BlobStatus::Ready {
            return Err(DomainError::InvalidTransition);
        }
        self.thumb_status = ThumbnailStatus::Generating;
        Ok(())
    }
    pub fn thumbnail_ready(&mut self) -> Result<(), DomainError> {
        if self.thumb_status != ThumbnailStatus::Generating {
            return Err(DomainError::InvalidTransition);
        }
        self.thumb_status = ThumbnailStatus::Ready;
        self.last_error = None;
        Ok(())
    }
    pub fn thumbnail_failed(&mut self, error_code: String) {
        self.thumb_status = ThumbnailStatus::Failed;
        self.last_error = Some(error_code);
    }
    pub fn evict_thumbnail(&mut self) {
        self.thumb_status = ThumbnailStatus::Missing;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageAction {
    CopyImage,
    CopyFile,
    ShareLaunched,
    ExportSaved,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalUsage {
    sticker_id: StickerId,
    last_used_at: TimestampMs,
    use_count: crate::version::UseCount,
}
impl LocalUsage {
    pub fn first_use(sticker_id: StickerId, at: TimestampMs) -> Self {
        Self {
            sticker_id,
            last_used_at: at,
            use_count: crate::version::UseCount::FIRST,
        }
    }
    pub fn record(&mut self, at: TimestampMs, _action: UsageAction) -> Result<(), DomainError> {
        let count = self.use_count.checked_next()?;
        self.last_used_at = at;
        self.use_count = count;
        Ok(())
    }
    pub const fn sticker_id(&self) -> StickerId {
        self.sticker_id
    }
    pub const fn last_used_at(&self) -> TimestampMs {
        self.last_used_at
    }
    pub const fn use_count(&self) -> i64 {
        self.use_count.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeStatus {
    Pending,
    Sending,
    AcceptedWaitingPull,
    Confirmed,
    Blocked,
    Sealed,
}

/// Durable operation state. Once sending begins, ID and payload are immutable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "LocalChangeWire", into = "LocalChangeWire")]
pub struct LocalChange {
    local_order: LocalOrder,
    op_id: OperationId,
    schema_version: ChangeSchemaVersion,
    operation: Operation,
    status: ChangeStatus,
    attempts: u32,
    retry_after: Option<TimestampMs>,
    accepted_revision: Option<Revision>,
    last_error: Option<String>,
    created_at: TimestampMs,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalChangeWire {
    local_order: LocalOrder,
    op_id: OperationId,
    schema_version: ChangeSchemaVersion,
    operation: Operation,
    status: ChangeStatus,
    attempts: u32,
    retry_after: Option<TimestampMs>,
    accepted_revision: Option<Revision>,
    last_error: Option<String>,
    created_at: TimestampMs,
}
impl LocalChange {
    pub fn new(
        local_order: LocalOrder,
        op_id: OperationId,
        operation: Operation,
        at: TimestampMs,
    ) -> Self {
        Self {
            local_order,
            op_id,
            schema_version: ChangeSchemaVersion::V1,
            operation,
            status: ChangeStatus::Pending,
            attempts: 0,
            retry_after: None,
            accepted_revision: None,
            last_error: None,
            created_at: at,
        }
    }
    pub const fn local_order(&self) -> LocalOrder {
        self.local_order
    }
    pub const fn op_id(&self) -> OperationId {
        self.op_id
    }
    pub fn operation(&self) -> &Operation {
        &self.operation
    }
    pub const fn status(&self) -> ChangeStatus {
        self.status
    }
    pub const fn attempts(&self) -> u32 {
        self.attempts
    }
    pub const fn retry_after(&self) -> Option<TimestampMs> {
        self.retry_after
    }
    pub const fn accepted_revision(&self) -> Option<Revision> {
        self.accepted_revision
    }
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
    pub const fn created_at(&self) -> TimestampMs {
        self.created_at
    }
    pub fn contributes_overlay(&self) -> bool {
        matches!(
            self.status,
            ChangeStatus::Pending | ChangeStatus::Sending | ChangeStatus::AcceptedWaitingPull
        )
    }
    pub fn rebase_unsent(&mut self, operation: Operation) -> Result<(), DomainError> {
        if self.attempts != 0 {
            return Err(DomainError::OperationFrozen);
        }
        if self.status != ChangeStatus::Pending {
            return Err(DomainError::InvalidTransition);
        }
        self.operation = operation;
        Ok(())
    }
    /// Persist this transition before issuing any request, including on retries.
    pub fn begin_send(&mut self) -> Result<(), DomainError> {
        if self.status != ChangeStatus::Pending {
            return Err(DomainError::InvalidTransition);
        }
        let attempts = self
            .attempts
            .checked_add(1)
            .ok_or(DomainError::VersionOverflow)?;
        self.attempts = attempts;
        self.status = ChangeStatus::Sending;
        self.retry_after = None;
        self.last_error = None;
        Ok(())
    }
    /// Also recovers a persisted Sending state after a process/network interruption.
    pub fn retry(&mut self, at: TimestampMs, error_code: String) -> Result<(), DomainError> {
        if self.status != ChangeStatus::Sending {
            return Err(DomainError::InvalidTransition);
        }
        self.status = ChangeStatus::Pending;
        self.retry_after = Some(at);
        self.last_error = Some(error_code);
        Ok(())
    }
    pub fn accepted(&mut self, revision: Revision) -> Result<(), DomainError> {
        if self.status != ChangeStatus::Sending {
            return Err(DomainError::InvalidTransition);
        }
        self.status = ChangeStatus::AcceptedWaitingPull;
        self.accepted_revision = Some(revision);
        Ok(())
    }
    /// Events may arrive before the push response. Caller applies canonical state
    /// and this confirmation in the same database transaction before advancing cursor.
    pub fn confirm_event(&mut self, op_id: OperationId, seq: EventSeq) -> Result<(), DomainError> {
        if op_id != self.op_id {
            return Err(DomainError::IdentityMismatch);
        }
        if self.attempts == 0 || !self.contributes_overlay() {
            return Err(DomainError::InvalidTransition);
        }
        if self
            .accepted_revision
            .is_some_and(|r| Revision::from(seq) < r)
        {
            return Err(DomainError::StaleRevision);
        }
        self.status = ChangeStatus::Confirmed;
        self.retry_after = None;
        self.last_error = None;
        self.accepted_revision = Some(seq.into());
        Ok(())
    }
    /// No-op receipts need canonical state pulled through the referenced revision.
    pub fn confirm_canonical(&mut self, applied: Revision) -> Result<(), DomainError> {
        if self.status != ChangeStatus::AcceptedWaitingPull {
            return Err(DomainError::InvalidTransition);
        }
        let required = self
            .accepted_revision
            .ok_or(DomainError::InvalidTransition)?;
        if applied < required {
            return Err(DomainError::StaleRevision);
        }
        self.status = ChangeStatus::Confirmed;
        self.last_error = None;
        Ok(())
    }
    pub fn block(&mut self, error_code: String) -> Result<(), DomainError> {
        if !self.contributes_overlay() {
            return Err(DomainError::InvalidTransition);
        }
        self.status = ChangeStatus::Blocked;
        self.retry_after = None;
        self.last_error = Some(error_code);
        Ok(())
    }
    /// Bootstrap seals unsent local history only after its cutoff snapshot commits.
    pub fn seal_local(&mut self) -> Result<(), DomainError> {
        if self.attempts != 0 || self.status != ChangeStatus::Pending {
            return Err(DomainError::InvalidTransition);
        }
        self.status = ChangeStatus::Sealed;
        Ok(())
    }
}
impl TryFrom<LocalChangeWire> for LocalChange {
    type Error = DomainError;
    fn try_from(w: LocalChangeWire) -> Result<Self, Self::Error> {
        if (matches!(
            w.status,
            ChangeStatus::Sending | ChangeStatus::AcceptedWaitingPull | ChangeStatus::Confirmed
        ) && w.attempts == 0)
            || (matches!(
                w.status,
                ChangeStatus::AcceptedWaitingPull | ChangeStatus::Confirmed
            ) && w.accepted_revision.is_none())
            || (matches!(
                w.status,
                ChangeStatus::Pending | ChangeStatus::Sending | ChangeStatus::Sealed
            ) && w.accepted_revision.is_some())
            || (w.status == ChangeStatus::Sealed && w.attempts != 0)
            || (w.retry_after.is_some() && w.status != ChangeStatus::Pending)
        {
            return Err(DomainError::InvalidTransition);
        }
        Ok(Self {
            local_order: w.local_order,
            op_id: w.op_id,
            schema_version: w.schema_version,
            operation: w.operation,
            status: w.status,
            attempts: w.attempts,
            retry_after: w.retry_after,
            accepted_revision: w.accepted_revision,
            last_error: w.last_error,
            created_at: w.created_at,
        })
    }
}
impl From<LocalChange> for LocalChangeWire {
    fn from(w: LocalChange) -> Self {
        Self {
            local_order: w.local_order,
            op_id: w.op_id,
            schema_version: w.schema_version,
            operation: w.operation,
            status: w.status,
            attempts: w.attempts,
            retry_after: w.retry_after,
            accepted_revision: w.accepted_revision,
            last_error: w.last_error,
            created_at: w.created_at,
        }
    }
}
