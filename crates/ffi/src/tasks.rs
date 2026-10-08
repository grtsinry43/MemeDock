use crate::{
    BridgeError, CancelResult, ErrorCode, Result, TaskSnapshot, streams::TaskProgressHandle,
};
use memedock_core::tasks::{Task, TaskController};
use std::sync::{Arc, Mutex};

pub(crate) struct TaskSlot<T> {
    task: Mutex<Option<Task<T>>>,
    pub(crate) controller: TaskController,
}
impl<T> TaskSlot<T> {
    pub(crate) fn new(task: Task<T>) -> Self {
        let controller = task.controller();
        Self {
            task: Mutex::new(Some(task)),
            controller,
        }
    }
    pub(crate) async fn wait(&self) -> Result<T> {
        Ok(self.take()?.wait().await?)
    }
    pub(crate) fn take(&self) -> Result<Task<T>> {
        self.task
            .lock()
            .map_err(|_| BridgeError::new(ErrorCode::Internal, "task slot poisoned"))?
            .take()
            .ok_or_else(|| BridgeError::new(ErrorCode::Conflict, "task result already consumed"))
    }
}
macro_rules! task_handle {
    ($name:ident, $core:ty, $output:ty, $convert:expr) => {
        #[derive(uniffi::Object)]
        pub struct $name {
            inner: TaskSlot<$core>,
        }
        impl $name {
            pub(crate) fn new(task: Task<$core>) -> Arc<Self> {
                Arc::new(Self {
                    inner: TaskSlot::new(task),
                })
            }
        }
        #[uniffi::export]
        impl $name {
            pub fn id(&self) -> String {
                self.inner.controller.id().to_string()
            }
            pub fn snapshot(&self) -> TaskSnapshot {
                self.inner.controller.snapshot().into()
            }
            pub fn cancel(&self) -> CancelResult {
                self.inner.controller.cancel().into()
            }
            pub fn progress(&self) -> Arc<TaskProgressHandle> {
                TaskProgressHandle::new(self.inner.controller.progress())
            }
            pub async fn await_result(&self) -> Result<$output> {
                let convert: fn($core) -> Result<$output> = $convert;
                convert(self.inner.wait().await?)
            }
        }
    };
}
task_handle!(
    TelegramPackTask,
    Arc<memedock_core::sources::telegram::TelegramPack>,
    Arc<crate::TelegramPackHandle>,
    |value| Ok(crate::TelegramPackHandle::new(value))
);
task_handle!(
    TelegramPreviewTask,
    memedock_core::Preview,
    String,
    |value: memedock_core::Preview| value
        .path
        .into_os_string()
        .into_string()
        .map_err(|_| BridgeError::new(ErrorCode::InvalidInput, "preview path is not UTF-8"))
);
task_handle!(
    StickerPageTask,
    memedock_core::QueryResponse,
    crate::StickerPage,
    |v| Ok(v.into())
);
task_handle!(
    ResourcesTask,
    Vec<memedock_core::StickerResource>,
    Vec<crate::StickerResource>,
    |values: Vec<memedock_core::StickerResource>| Ok(values.into_iter().map(Into::into).collect())
);
task_handle!(
    ImportInputTask,
    memedock_core::ImportInput,
    Arc<crate::ImportInputHandle>,
    crate::ImportInputHandle::new
);
task_handle!(
    ImportTask,
    memedock_core::ImportOutcome,
    crate::ImportOutcome,
    |value| Ok(value.into())
);
task_handle!(
    ThumbnailTask,
    memedock_core::Thumbnail,
    crate::Thumbnail,
    |value: memedock_core::Thumbnail| Ok(crate::Thumbnail {
        path: value
            .path
            .into_os_string()
            .into_string()
            .map_err(|_| BridgeError::new(
                ErrorCode::InvalidInput,
                "thumbnail path is not UTF-8"
            ))?,
    })
);
task_handle!(DiscardInputTask, (), (), |value| Ok(value));
task_handle!(
    BackupTask,
    memedock_core::BackupFile,
    Arc<crate::BackupFileHandle>,
    crate::BackupFileHandle::new
);
task_handle!(
    ArchiveInputTask,
    memedock_core::ArchiveInput,
    Arc<crate::ArchiveInputHandle>,
    crate::ArchiveInputHandle::new
);
task_handle!(
    ArchiveInspectionTask,
    Arc<memedock_core::PreparedArchive>,
    Arc<crate::PreparedArchiveHandle>,
    |value| Ok(crate::PreparedArchiveHandle::new(value))
);
task_handle!(MutationTask, (), (), |value| Ok(value));
task_handle!(
    StickerMutationTask,
    memedock_domain::sticker::Sticker,
    crate::StickerMetadata,
    |value| Ok(value.into())
);
task_handle!(
    CollectionMutationTask,
    memedock_domain::collection::Collection,
    crate::CollectionMetadata,
    |value| Ok(value.into())
);
task_handle!(
    TagMutationTask,
    memedock_domain::tag::Tag,
    crate::TagMetadata,
    |value| Ok(value.into())
);
task_handle!(
    RestoreSuggestionsTask,
    memedock_core::RestoreSuggestions,
    crate::RestoreSuggestions,
    |value: memedock_core::RestoreSuggestions| Ok(crate::RestoreSuggestions {
        collection: value.collection.map(Into::into),
        tags: value.tags.into_iter().map(Into::into).collect(),
    })
);
task_handle!(
    ExportTask,
    Arc<memedock_core::ArtifactLease>,
    Arc<crate::ArtifactLeaseHandle>,
    |value| Ok(crate::ArtifactLeaseHandle::new(value))
);
task_handle!(
    HandoffTask,
    memedock_core::ExportArtifact,
    crate::ExportArtifact,
    TryInto::try_into
);
task_handle!(ArtifactCleanupTask, u64, u64, |value| Ok(value));
task_handle!(
    ClipboardProtectionTask,
    memedock_domain::identity::OperationId,
    String,
    |value: memedock_domain::identity::OperationId| Ok(value.to_string())
);
task_handle!(ClipboardUpdateTask, (), (), |value| Ok(value));
task_handle!(
    StickerDetailTask,
    memedock_core::StickerDetail,
    crate::StickerDetail,
    TryInto::try_into
);
task_handle!(
    CollectionsTask,
    Vec<memedock_domain::collection::Collection>,
    Vec<crate::CollectionMetadata>,
    |v: Vec<memedock_domain::collection::Collection>| Ok(v.into_iter().map(Into::into).collect())
);
task_handle!(
    TagsTask,
    Vec<memedock_domain::tag::Tag>,
    Vec<crate::TagMetadata>,
    |v: Vec<memedock_domain::tag::Tag>| Ok(v.into_iter().map(Into::into).collect())
);
task_handle!(
    StatisticsTask,
    memedock_core::SpaceStatistics,
    crate::SpaceStatistics,
    |v| Ok(v.into())
);
task_handle!(
    UsageTask,
    memedock_domain::local::LocalUsage,
    crate::UsageMetadata,
    |v| Ok(v.into())
);
task_handle!(
    VerificationTask,
    memedock_core::VerifiedOriginal,
    crate::VerifiedOriginal,
    |v| Ok(v.into())
);
task_handle!(
    CheckpointTask,
    std::path::PathBuf,
    String,
    |v: std::path::PathBuf| v
        .into_os_string()
        .into_string()
        .map_err(|_| BridgeError::new(ErrorCode::InvalidInput, "checkpoint path is not UTF-8"))
);
task_handle!(OrganizationMutationTask, bool, bool, |value| Ok(value));
task_handle!(
    CollectionSummariesTask,
    Vec<memedock_core::CollectionSummary>,
    Vec<crate::CollectionSummary>,
    |values: Vec<memedock_core::CollectionSummary>| values
        .into_iter()
        .map(TryInto::try_into)
        .collect()
);
