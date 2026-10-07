use crate::{ErrorCode, QueryCursorHandle};
use memedock_domain::{
    asset::Asset, collection::Collection, lifecycle::Lifecycle, local::LocalUsage,
    sticker::Sticker, tag::Tag,
};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct EntityLifecycle {
    pub generation: i64,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}
impl From<&Lifecycle> for EntityLifecycle {
    fn from(v: &Lifecycle) -> Self {
        Self {
            generation: v.generation().get(),
            revision: v.revision().get(),
            created_at: v.created_at().get(),
            updated_at: v.updated_at().get(),
            deleted_at: v.deleted_at().map(|t| t.get()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct StickerMetadata {
    pub id: String,
    pub title: String,
    pub original_name: String,
    pub note: String,
    pub starred: bool,
    pub lifecycle: EntityLifecycle,
}
impl From<Sticker> for StickerMetadata {
    fn from(v: Sticker) -> Self {
        Self {
            id: v.id().to_string(),
            title: v.title().into(),
            original_name: v.original_name().into(),
            note: v.note().into(),
            starred: v.starred(),
            lifecycle: v.lifecycle().into(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct AssetMetadata {
    pub hash: String,
    pub byte_size: i64,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub animated: bool,
    pub created_at: i64,
}
impl From<Asset> for AssetMetadata {
    fn from(v: Asset) -> Self {
        Self {
            hash: v.hash().to_string(),
            byte_size: v.byte_size().get(),
            mime: v.format().mime().into(),
            width: v.width(),
            height: v.height(),
            animated: v.animated(),
            created_at: v.created_at().get(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CollectionMetadata {
    pub id: String,
    pub name: String,
    pub sort_key: String,
    pub lifecycle: EntityLifecycle,
}
impl From<Collection> for CollectionMetadata {
    fn from(v: Collection) -> Self {
        Self {
            id: v.id().to_string(),
            name: v.name().as_str().into(),
            sort_key: v.sort_key().to_string(),
            lifecycle: v.lifecycle().into(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct TagMetadata {
    pub id: String,
    pub name: String,
    pub normalized_name: String,
    pub lifecycle: EntityLifecycle,
}
impl From<Tag> for TagMetadata {
    fn from(v: Tag) -> Self {
        Self {
            id: v.id().to_string(),
            name: v.name().as_str().into(),
            normalized_name: v.normalized_name(),
            lifecycle: v.lifecycle().into(),
        }
    }
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct StickerDetail {
    pub sticker: StickerMetadata,
    pub asset: AssetMetadata,
    pub tags: Vec<TagMetadata>,
    pub collection: Option<CollectionMetadata>,
    pub original_path: Option<String>,
    pub original_error: Option<ErrorCode>,
}
impl TryFrom<memedock_core::StickerDetail> for StickerDetail {
    type Error = crate::BridgeError;
    fn try_from(v: memedock_core::StickerDetail) -> crate::Result<Self> {
        Ok(Self {
            sticker: v.sticker.into(),
            asset: v.asset.into(),
            tags: v.tags.into_iter().map(Into::into).collect(),
            collection: v.collection.map(Into::into),
            original_path: v
                .original_path
                .map(|p| {
                    p.into_os_string().into_string().map_err(|_| {
                        crate::BridgeError::new(
                            ErrorCode::InvalidInput,
                            "original path is not UTF-8",
                        )
                    })
                })
                .transpose()?,
            original_error: v.original_error.map(Into::into),
        })
    }
}
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum StickerSort {
    Recent,
    CollectionOrder,
    LastUsed,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct StickerQuery {
    pub request_id: String,
    pub text: String,
    pub collection_id: Option<String>,
    pub tag_ids: Vec<String>,
    pub starred: Option<bool>,
    pub deleted: bool,
    pub sort: StickerSort,
    pub page_size: u32,
    pub cursor: Option<Arc<QueryCursorHandle>>,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct StickerPage {
    pub request_id: String,
    pub stickers: Vec<StickerMetadata>,
    pub next: Option<Arc<QueryCursorHandle>>,
    pub resources: Vec<StickerResource>,
}
#[derive(Clone, Debug, uniffi::Enum)]
pub enum ThumbnailStatus {
    Missing,
    Generating,
    Ready,
    Failed,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct StickerResource {
    pub asset: AssetMetadata,
    pub thumbnail_status: ThumbnailStatus,
    pub thumbnail_path: Option<String>,
    pub last_error: Option<String>,
}
impl From<memedock_core::StickerResource> for StickerResource {
    fn from(value: memedock_core::StickerResource) -> Self {
        use memedock_domain::local::ThumbnailStatus as C;
        let status = value
            .local
            .as_ref()
            .map_or(C::Missing, |local| local.thumb_status());
        Self {
            asset: value.asset.into(),
            thumbnail_status: match status {
                C::Missing => ThumbnailStatus::Missing,
                C::Generating => ThumbnailStatus::Generating,
                C::Ready => ThumbnailStatus::Ready,
                C::Failed => ThumbnailStatus::Failed,
            },
            thumbnail_path: value
                .thumbnail_path
                .and_then(|path| path.into_os_string().into_string().ok()),
            last_error: value
                .local
                .and_then(|local| local.last_error().map(ToOwned::to_owned)),
        }
    }
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct ImportOptions {
    pub original_name: String,
    pub title: Option<String>,
    pub collection_id: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ImportStatus {
    Created,
    Reused,
    RestoreRequired,
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct ImportOutcome {
    pub sticker: StickerMetadata,
    pub status: ImportStatus,
}
impl From<memedock_core::ImportOutcome> for ImportOutcome {
    fn from(value: memedock_core::ImportOutcome) -> Self {
        use memedock_core::ImportStatus as C;
        Self {
            sticker: value.sticker.into(),
            status: match value.status {
                C::Created => ImportStatus::Created,
                C::Reused => ImportStatus::Reused,
                C::RestoreRequired => ImportStatus::RestoreRequired,
            },
        }
    }
}
#[derive(Clone, Debug, uniffi::Record)]
pub struct Thumbnail {
    pub path: String,
}
impl From<memedock_core::QueryResponse> for StickerPage {
    fn from(v: memedock_core::QueryResponse) -> Self {
        Self {
            request_id: v.request_id.to_string(),
            stickers: v.stickers.into_iter().map(Into::into).collect(),
            next: v.next.map(|inner| Arc::new(QueryCursorHandle { inner })),
            resources: v.resources.into_iter().map(Into::into).collect(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct LibraryIdentity {
    pub library_id: String,
    pub device_id: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum LibraryState {
    Open,
    Closing,
    Closed,
}
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum UsageAction {
    CopyImage,
    CopyFile,
    ShareLaunched,
    ExportSaved,
}
impl From<UsageAction> for memedock_domain::local::UsageAction {
    fn from(v: UsageAction) -> Self {
        match v {
            UsageAction::CopyImage => Self::CopyImage,
            UsageAction::CopyFile => Self::CopyFile,
            UsageAction::ShareLaunched => Self::ShareLaunched,
            UsageAction::ExportSaved => Self::ExportSaved,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UsageMetadata {
    pub sticker_id: String,
    pub last_used_at: i64,
    pub use_count: i64,
}
impl From<LocalUsage> for UsageMetadata {
    fn from(v: LocalUsage) -> Self {
        Self {
            sticker_id: v.sticker_id().to_string(),
            last_used_at: v.last_used_at().get(),
            use_count: v.use_count(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct SpaceStatistics {
    pub known_assets: i64,
    pub known_original_bytes: i64,
    pub ready_original_bytes: i64,
    pub thumbnail_bytes: i64,
    pub temporary_share_bytes: i64,
}
impl From<memedock_core::SpaceStatistics> for SpaceStatistics {
    fn from(v: memedock_core::SpaceStatistics) -> Self {
        Self {
            known_assets: v.known_assets,
            known_original_bytes: v.known_original_bytes,
            ready_original_bytes: v.ready_original_bytes,
            thumbnail_bytes: v.thumbnail_bytes,
            temporary_share_bytes: v.temporary_share_bytes,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct VerifiedOriginal {
    pub hash: String,
    pub byte_size: u64,
}
impl From<memedock_core::VerifiedOriginal> for VerifiedOriginal {
    fn from(v: memedock_core::VerifiedOriginal) -> Self {
        Self {
            hash: v.hash.to_string(),
            byte_size: v.byte_size,
        }
    }
}
#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum Priority {
    Interactive,
    Visible,
    Background,
}
impl From<Priority> for memedock_core::tasks::Priority {
    fn from(v: Priority) -> Self {
        match v {
            Priority::Interactive => Self::Interactive,
            Priority::Visible => Self::Visible,
            Priority::Background => Self::Background,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum CancelResult {
    Requested,
    CommitInProgress,
    Finished,
    NotFound,
}
impl From<memedock_core::tasks::CancelResult> for CancelResult {
    fn from(v: memedock_core::tasks::CancelResult) -> Self {
        use memedock_core::tasks::CancelResult as C;
        match v {
            C::Requested => Self::Requested,
            C::CommitInProgress => Self::CommitInProgress,
            C::Finished => Self::Finished,
            C::NotFound => Self::NotFound,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum TaskStatus {
    Queued,
    Running,
    Committing,
    Succeeded,
    Failed { code: ErrorCode },
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct TaskSnapshot {
    pub id: String,
    pub status: TaskStatus,
    pub cancellation_requested: bool,
    pub stage: String,
}
impl From<memedock_core::tasks::TaskSnapshot> for TaskSnapshot {
    fn from(v: memedock_core::tasks::TaskSnapshot) -> Self {
        use memedock_core::tasks::TaskStatus as C;
        Self {
            id: v.id.to_string(),
            status: match v.status {
                C::Queued => TaskStatus::Queued,
                C::Running => TaskStatus::Running,
                C::Committing => TaskStatus::Committing,
                C::Succeeded => TaskStatus::Succeeded,
                C::Failed(c) => TaskStatus::Failed { code: c.into() },
                C::Cancelled => TaskStatus::Cancelled,
            },
            cancellation_requested: v.cancellation_requested,
            stage: v.stage.into(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Notification {
    CollectionsChanged { sequence: u64 },
    TagsChanged { sequence: u64 },
    UsageChanged { sequence: u64, sticker_id: String },
    StickerChanged { sequence: u64, sticker_id: String },
    ThumbnailChanged { sequence: u64, sticker_id: String },
    ReloadRequired { through_sequence: u64 },
    Closed,
}
impl From<memedock_core::events::Notification> for Notification {
    fn from(v: memedock_core::events::Notification) -> Self {
        use memedock_core::events::{ChangeKind, Notification as C};
        match v {
            C::Changed(e) => match e.kind {
                ChangeKind::CollectionsChanged => Self::CollectionsChanged {
                    sequence: e.sequence,
                },
                ChangeKind::TagsChanged => Self::TagsChanged {
                    sequence: e.sequence,
                },
                ChangeKind::UsageChanged(id) => Self::UsageChanged {
                    sequence: e.sequence,
                    sticker_id: id.to_string(),
                },
                ChangeKind::StickerChanged(id) => Self::StickerChanged {
                    sequence: e.sequence,
                    sticker_id: id.to_string(),
                },
                ChangeKind::ThumbnailChanged(id) => Self::ThumbnailChanged {
                    sequence: e.sequence,
                    sticker_id: id.to_string(),
                },
            },
            C::ReloadRequired { through_sequence } => Self::ReloadRequired { through_sequence },
            C::Closed => Self::Closed,
        }
    }
}
