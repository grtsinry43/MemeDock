use crate::{
    BridgeError, CancelResult, ErrorCode, Result, TaskSnapshot, streams::TaskProgressHandle,
};
use memedock_core::tasks::{Task, TaskController};
use std::sync::{Arc, Mutex};

struct TaskSlot<T> {
    task: Mutex<Option<Task<T>>>,
    controller: TaskController,
}
impl<T> TaskSlot<T> {
    fn new(task: Task<T>) -> Self {
        let controller = task.controller();
        Self {
            task: Mutex::new(Some(task)),
            controller,
        }
    }
    async fn wait(&self) -> Result<T> {
        let task = self
            .task
            .lock()
            .map_err(|_| BridgeError::new(ErrorCode::Internal, "task slot poisoned"))?
            .take()
            .ok_or_else(|| BridgeError::new(ErrorCode::Conflict, "task result already consumed"))?;
        Ok(task.wait().await?)
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
        collections: value.collections.into_iter().map(Into::into).collect(),
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
