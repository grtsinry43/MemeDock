use crate::*;
use memedock_domain::version::{Generation, Revision};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, uniffi::Record)]
pub struct BatchTarget {
    pub target: EntityReference,
    pub deleted_revision: Option<i64>,
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum BatchAction {
    Assign { collection: EntityReference },
    ClearCollection { expected: Option<EntityReference> },
    AddTags { tags: Vec<EntityReference> },
    RemoveTags { tags: Vec<EntityReference> },
    Star { value: bool },
    Delete,
    Restore,
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum BatchOutcome {
    Pending,
    Applied,
    Unchanged,
    Failed,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct BatchItemResult {
    pub id: String,
    pub outcome: BatchOutcome,
    pub error: Option<ErrorCode>,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct BatchReport {
    pub items: Vec<BatchItemResult>,
    pub stopped: bool,
}
impl From<memedock_core::batch::BatchReport> for BatchReport {
    fn from(value: memedock_core::batch::BatchReport) -> Self {
        Self {
            stopped: value.stopped,
            items: value
                .items
                .into_iter()
                .map(|item| {
                    use memedock_core::batch::BatchOutcome as C;
                    let (outcome, error) = match item.outcome {
                        C::Pending => (BatchOutcome::Pending, None),
                        C::Applied => (BatchOutcome::Applied, None),
                        C::Unchanged => (BatchOutcome::Unchanged, None),
                        C::Failed(error) => (BatchOutcome::Failed, Some(error.into())),
                    };
                    BatchItemResult {
                        id: item.id.to_string(),
                        outcome,
                        error,
                    }
                })
                .collect(),
        }
    }
}

#[derive(uniffi::Object)]
pub struct BatchTask {
    inner: Mutex<Option<memedock_core::batch::BatchTask>>,
    controller: memedock_core::batch::BatchController,
}
#[uniffi::export]
impl BatchTask {
    pub fn cancel(&self) -> Result<()> {
        Ok(self.controller.cancel()?)
    }
    pub fn snapshot(&self) -> BatchReport {
        self.controller.snapshot().into()
    }
    pub async fn await_result(&self) -> Result<BatchReport> {
        let task = self
            .inner
            .lock()
            .map_err(|_| BridgeError::new(ErrorCode::Internal, "batch slot poisoned"))?
            .take()
            .ok_or_else(|| BridgeError::new(ErrorCode::Conflict, "batch already awaited"))?;
        Ok(task.wait().await?.into())
    }
}
#[uniffi::export]
impl LibraryHandle {
    pub fn batch(&self, targets: Vec<BatchTarget>, action: BatchAction) -> Result<Arc<BatchTask>> {
        use memedock_core::batch as c;
        let targets = targets
            .into_iter()
            .map(|value| {
                Ok(c::BatchTarget {
                    id: value.target.id.parse()?,
                    generation: Generation::new(value.target.generation)?,
                    deleted_revision: value.deleted_revision.map(Revision::new).transpose()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let action = match action {
            BatchAction::Assign { collection } => c::BatchAction::Assign(
                collection.id.parse()?,
                Generation::new(collection.generation)?,
            ),
            BatchAction::ClearCollection { expected } => c::BatchAction::ClearCollection(
                expected
                    .map(|v| Ok::<_, BridgeError>((v.id.parse()?, Generation::new(v.generation)?)))
                    .transpose()?,
            ),
            BatchAction::AddTags { tags } => c::BatchAction::AddTags(
                tags.into_iter()
                    .map(|v| Ok((v.id.parse()?, Generation::new(v.generation)?)))
                    .collect::<Result<Vec<_>>>()?,
            ),
            BatchAction::RemoveTags { tags } => c::BatchAction::RemoveTags(
                tags.into_iter()
                    .map(|v| Ok((v.id.parse()?, Generation::new(v.generation)?)))
                    .collect::<Result<Vec<_>>>()?,
            ),
            BatchAction::Star { value } => c::BatchAction::Star(value),
            BatchAction::Delete => c::BatchAction::Delete,
            BatchAction::Restore => c::BatchAction::Restore,
        };
        let inner = self.inner.batch(targets, action)?;
        Ok(Arc::new(BatchTask {
            controller: inner.controller(),
            inner: Mutex::new(Some(inner)),
        }))
    }
    pub fn collection_summaries(&self) -> Result<Arc<CollectionSummariesTask>> {
        Ok(CollectionSummariesTask::new(
            self.inner.collection_summaries()?,
        ))
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct CollectionSummary {
    pub collection: CollectionMetadata,
    pub count: u64,
    pub cover: Option<String>,
    pub thumbnail_path: Option<String>,
}
impl TryFrom<memedock_core::CollectionSummary> for CollectionSummary {
    type Error = BridgeError;
    fn try_from(value: memedock_core::CollectionSummary) -> Result<Self> {
        Ok(Self {
            collection: value.collection.into(),
            count: value.count,
            cover: value.cover.map(|v| v.to_string()),
            thumbnail_path: value
                .thumbnail_path
                .map(|v| {
                    v.into_os_string().into_string().map_err(|_| {
                        BridgeError::new(ErrorCode::InvalidInput, "thumbnail path is not UTF-8")
                    })
                })
                .transpose()?,
        })
    }
}
