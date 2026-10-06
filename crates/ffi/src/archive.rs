//! Owned, current-format archive tokens; platforms only copy their private files.
use crate::{
    ArchiveInputTask, ArchiveInspectionTask, BackupTask, BridgeError, CancelResult, ErrorCode,
    LibraryHandle, MutationTask, Result, TaskSnapshot, streams::TaskProgressHandle,
    tasks::TaskSlot,
};
use std::sync::{Arc, Mutex};

fn take<T>(slot: &Mutex<Option<T>>) -> Result<T> {
    slot.lock()
        .map_err(|_| BridgeError::new(ErrorCode::Internal, "archive token poisoned"))?
        .take()
        .ok_or_else(|| BridgeError::new(ErrorCode::Conflict, "archive token consumed"))
}
fn path(path: &std::path::Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| BridgeError::new(ErrorCode::InvalidInput, "archive path is not UTF-8"))
}
#[derive(uniffi::Object)]
pub struct ArchiveInputHandle {
    inner: Mutex<Option<memedock_core::ArchiveInput>>,
    path: String,
}
impl ArchiveInputHandle {
    pub(crate) fn new(value: memedock_core::ArchiveInput) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            path: path(value.path())?,
            inner: Mutex::new(Some(value)),
        }))
    }
}
#[uniffi::export]
impl ArchiveInputHandle {
    pub fn path(&self) -> String {
        self.path.clone()
    }
}

#[derive(uniffi::Object)]
pub struct BackupFileHandle {
    inner: Mutex<Option<memedock_core::BackupFile>>,
    path: String,
    file_name: String,
    byte_size: u64,
}
impl BackupFileHandle {
    pub(crate) fn new(value: memedock_core::BackupFile) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            path: path(&value.path)?,
            file_name: value.file_name.clone(),
            byte_size: value.byte_size,
            inner: Mutex::new(Some(value)),
        }))
    }
}
#[uniffi::export]
impl BackupFileHandle {
    pub fn path(&self) -> String {
        self.path.clone()
    }
    pub fn file_name(&self) -> String {
        self.file_name.clone()
    }
    pub fn byte_size(&self) -> u64 {
        self.byte_size
    }
}
#[derive(uniffi::Record)]
pub struct ArchiveSummary {
    pub stickers: u64,
    pub deleted_stickers: u64,
    pub collections: u64,
    pub tags: u64,
    pub original_bytes: u64,
    pub added: u64,
    pub preserved: u64,
    pub skipped_relations: u64,
}
impl From<memedock_core::RestoreSummary> for ArchiveSummary {
    fn from(v: memedock_core::RestoreSummary) -> Self {
        Self {
            stickers: v.stickers,
            deleted_stickers: v.deleted_stickers,
            collections: v.collections,
            tags: v.tags,
            original_bytes: v.original_bytes,
            added: v.added,
            preserved: v.preserved,
            skipped_relations: v.skipped_relations,
        }
    }
}
#[derive(uniffi::Enum, Clone, Copy)]
pub enum ArchiveRestoreMode {
    Merge,
    Replace,
}
#[derive(uniffi::Object)]
pub struct PreparedArchiveHandle {
    inner: Mutex<Option<Arc<memedock_core::PreparedArchive>>>,
    summary: memedock_core::RestoreSummary,
}
impl PreparedArchiveHandle {
    pub(crate) fn new(inner: Arc<memedock_core::PreparedArchive>) -> Arc<Self> {
        Arc::new(Self {
            summary: inner.summary().clone(),
            inner: Mutex::new(Some(inner)),
        })
    }
}
#[uniffi::export]
impl PreparedArchiveHandle {
    pub fn summary(&self) -> ArchiveSummary {
        self.summary.clone().into()
    }
}

#[derive(uniffi::Object)]
pub struct ArchiveRestoreTask {
    inner: TaskSlot<memedock_core::RestoreResult>,
    library: Arc<LibraryHandle>,
    prepared: Arc<memedock_core::PreparedArchive>,
}
#[uniffi::export]
impl ArchiveRestoreTask {
    pub fn snapshot(&self) -> TaskSnapshot {
        self.inner.controller.snapshot().into()
    }
    pub fn id(&self) -> String {
        self.inner.controller.id().to_string()
    }
    pub fn cancel(&self) -> CancelResult {
        self.inner.controller.cancel().into()
    }
    pub fn progress(&self) -> Arc<TaskProgressHandle> {
        TaskProgressHandle::new(self.inner.controller.progress())
    }
    pub async fn await_result(&self) -> Result<ArchiveSummary> {
        let task = self.inner.take()?;
        let outcome = task.wait().await.map_err(BridgeError::from);
        let cleanup = match self
            .library
            .inner
            .discard_prepared_archive(self.prepared.clone())
        {
            Ok(task) => task.wait().await.map_err(BridgeError::from),
            Err(error) => Err(error.into()),
        };
        let prepared = outcome?;
        cleanup?;
        Ok(self
            .library
            .inner
            .complete_restore(prepared)
            .await?
            .summary
            .into())
    }
}
#[uniffi::export]
impl LibraryHandle {
    pub fn create_backup(&self) -> Result<Arc<BackupTask>> {
        Ok(BackupTask::new(self.inner.create_backup()?))
    }
    pub fn discard_backup(&self, backup: Arc<BackupFileHandle>) -> Result<Arc<MutationTask>> {
        Ok(MutationTask::new(
            self.inner.discard_backup(take(&backup.inner)?)?,
        ))
    }
    pub fn create_archive_input(&self) -> Result<Arc<ArchiveInputTask>> {
        Ok(ArchiveInputTask::new(self.inner.create_archive_input()?))
    }
    pub fn discard_archive_input(
        &self,
        input: Arc<ArchiveInputHandle>,
    ) -> Result<Arc<MutationTask>> {
        Ok(MutationTask::new(
            self.inner.discard_archive_input(take(&input.inner)?)?,
        ))
    }
    pub fn inspect_archive(
        &self,
        input: Arc<ArchiveInputHandle>,
    ) -> Result<Arc<ArchiveInspectionTask>> {
        Ok(ArchiveInspectionTask::new(
            self.inner.inspect_archive(take(&input.inner)?)?,
        ))
    }
    pub fn discard_prepared_archive(
        &self,
        prepared: Arc<PreparedArchiveHandle>,
    ) -> Result<Arc<MutationTask>> {
        Ok(MutationTask::new(
            self.inner
                .discard_prepared_archive(take(&prepared.inner)?)?,
        ))
    }
    pub fn restore_archive(
        self: Arc<Self>,
        prepared: Arc<PreparedArchiveHandle>,
        mode: ArchiveRestoreMode,
    ) -> Result<Arc<ArchiveRestoreTask>> {
        let mode = match mode {
            ArchiveRestoreMode::Merge => memedock_core::RestoreMode::Merge,
            ArchiveRestoreMode::Replace => memedock_core::RestoreMode::Replace,
        };
        let prepared = take(&prepared.inner)?;
        let task = self.inner.restore_archive(prepared.clone(), mode)?;
        Ok(Arc::new(ArchiveRestoreTask {
            inner: TaskSlot::new(task),
            library: self,
            prepared,
        }))
    }
}
