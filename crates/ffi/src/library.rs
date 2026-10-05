use crate::*;
use std::sync::Arc;

#[derive(uniffi::Object)]
pub struct LibraryHandle {
    inner: memedock_core::Library,
}
#[uniffi::export]
pub async fn open_library(configuration: LibraryConfiguration) -> Result<Arc<LibraryHandle>> {
    Ok(Arc::new(LibraryHandle {
        inner: memedock_core::Library::open(configuration.try_into()?).await?,
    }))
}
#[uniffi::export]
pub fn new_request_id() -> String {
    memedock_core::RequestId::new().to_string()
}
#[uniffi::export]
impl LibraryHandle {
    pub fn export_original(&self, id: String) -> Result<Arc<ExportTask>> {
        Ok(ExportTask::new(self.inner.export_original(id.parse()?)?))
    }
    pub fn prepare_handoff(&self, lease: Arc<ArtifactLeaseHandle>) -> Result<Arc<HandoffTask>> {
        Ok(HandoffTask::new(
            self.inner.prepare_handoff(lease.inner.clone())?,
        ))
    }
    pub fn clean_export_artifacts(&self) -> Result<Arc<ArtifactCleanupTask>> {
        Ok(ArtifactCleanupTask::new(
            self.inner.clean_export_artifacts()?,
        ))
    }
    pub fn sticker_resources(&self, ids: Vec<String>) -> Result<Arc<ResourcesTask>> {
        let ids = ids
            .into_iter()
            .map(|id| id.parse())
            .collect::<std::result::Result<_, _>>()?;
        Ok(ResourcesTask::new(self.inner.sticker_resources(ids)?))
    }
    pub fn create_import_input(&self) -> Result<Arc<ImportInputTask>> {
        Ok(ImportInputTask::new(self.inner.create_import_input()?))
    }
    pub fn discard_import_input(
        &self,
        input: Arc<ImportInputHandle>,
    ) -> Result<Arc<DiscardInputTask>> {
        Ok(DiscardInputTask::new(
            self.inner.discard_import_input(input.take()?)?,
        ))
    }
    pub fn import_staged(
        &self,
        input: Arc<ImportInputHandle>,
        options: ImportOptions,
    ) -> Result<Arc<ImportTask>> {
        let options = memedock_core::ImportOptions {
            original_name: options.original_name,
            title: options.title,
            collection: options.collection_id.map(|id| id.parse()).transpose()?,
        };
        Ok(ImportTask::new(
            self.inner.import_staged(input.take()?, options)?,
        ))
    }
    pub fn request_thumbnail(&self, id: String, priority: Priority) -> Result<Arc<ThumbnailTask>> {
        Ok(ThumbnailTask::new(
            self.inner.request_thumbnail(id.parse()?, priority.into())?,
        ))
    }
    pub fn identity(&self) -> LibraryIdentity {
        let v = self.inner.identity();
        LibraryIdentity {
            library_id: v.library_id.to_string(),
            device_id: v.device_id.to_string(),
        }
    }
    pub fn state(&self) -> Result<LibraryState> {
        Ok(match self.inner.state()? {
            memedock_core::LibraryState::Open => LibraryState::Open,
            memedock_core::LibraryState::Closing => LibraryState::Closing,
            memedock_core::LibraryState::Closed => LibraryState::Closed,
        })
    }
    pub async fn shutdown(&self) -> Result<()> {
        Ok(self.inner.close().await?)
    }
    pub fn cancel_task(&self, id: String) -> Result<CancelResult> {
        Ok(self.inner.cancel_task(id.parse()?)?.into())
    }
    pub fn subscribe(&self) -> Result<Arc<SubscriptionHandle>> {
        Ok(SubscriptionHandle::new(self.inner.subscribe()?))
    }
    pub fn list_stickers(&self, query: StickerQuery) -> Result<Arc<StickerPageTask>> {
        let sort = match query.sort {
            StickerSort::Recent => memedock_core::StickerSort::Recent,
            StickerSort::CollectionOrder => memedock_core::StickerSort::CollectionOrder,
            StickerSort::LastUsed => memedock_core::StickerSort::LastUsed,
        };
        let request = memedock_core::QueryRequest {
            request_id: query.request_id.parse()?,
            query: memedock_core::StickerQuery {
                text: query.text,
                collection: query.collection_id.map(|s| s.parse()).transpose()?,
                tags: query
                    .tag_ids
                    .into_iter()
                    .map(|s| s.parse())
                    .collect::<std::result::Result<_, _>>()?,
                starred: query.starred,
                deleted: query.deleted,
                sort,
            },
            page_size: query.page_size,
            cursor: query.cursor.map(|c| c.inner.clone()),
        };
        Ok(StickerPageTask::new(self.inner.list_stickers(request)?))
    }
    pub fn sticker_detail(&self, id: String) -> Result<Arc<StickerDetailTask>> {
        Ok(StickerDetailTask::new(
            self.inner.sticker_detail(id.parse()?)?,
        ))
    }
    pub fn collections(&self, deleted: bool) -> Result<Arc<CollectionsTask>> {
        Ok(CollectionsTask::new(self.inner.collections(deleted)?))
    }
    pub fn tags(&self, deleted: bool) -> Result<Arc<TagsTask>> {
        Ok(TagsTask::new(self.inner.tags(deleted)?))
    }
    pub fn space_statistics(&self) -> Result<Arc<StatisticsTask>> {
        Ok(StatisticsTask::new(self.inner.space_statistics()?))
    }
    pub fn record_use(&self, id: String, action: UsageAction) -> Result<Arc<UsageTask>> {
        Ok(UsageTask::new(
            self.inner.record_use(id.parse()?, action.into())?,
        ))
    }
    pub fn verify_original(
        &self,
        hash: String,
        priority: Priority,
    ) -> Result<Arc<VerificationTask>> {
        Ok(VerificationTask::new(
            self.inner.verify_original(hash.parse()?, priority.into())?,
        ))
    }
    pub fn create_checkpoint(&self) -> Result<Arc<CheckpointTask>> {
        Ok(CheckpointTask::new(self.inner.create_checkpoint()?))
    }
}
