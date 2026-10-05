use crate::{
    CoreError, ErrorCode, Library, Result,
    tasks::scheduler::Lane,
    tasks::{Priority, Task},
};
use memedock_domain::{
    asset::Asset,
    identity::{LibraryId, StickerId},
    sticker::Sticker,
    tag::Tag,
};
use memedock_storage::queries::{SpaceStatistics, StickerQuery};
use std::{fmt, str::FromStr};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RequestId(Uuid);
impl RequestId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}
impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}
impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for RequestId {
    type Err = CoreError;
    fn from_str(value: &str) -> Result<Self> {
        let id = Uuid::parse_str(value)
            .map_err(|e| CoreError::caused(ErrorCode::InvalidInput, "invalid request ID", e))?;
        if id.get_version_num() != 7 || id.get_variant() != uuid::Variant::RFC4122 {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "request ID must be UUIDv7",
            ));
        }
        Ok(Self(id))
    }
}
#[derive(Clone, Debug)]
pub struct QueryCursor {
    library_id: LibraryId,
    inner: memedock_storage::queries::PageCursor,
}
#[derive(Clone, Debug)]
pub struct QueryRequest {
    pub request_id: RequestId,
    pub query: StickerQuery,
    pub page_size: u32,
    pub cursor: Option<QueryCursor>,
}
#[derive(Debug)]
pub struct QueryResponse {
    pub request_id: RequestId,
    pub stickers: Vec<Sticker>,
    pub next: Option<QueryCursor>,
    pub resources: Vec<StickerResource>,
}
#[derive(Debug)]
pub struct StickerResource {
    pub asset: Asset,
    pub local: Option<memedock_domain::local::LocalAsset>,
    pub thumbnail_path: Option<std::path::PathBuf>,
}
#[derive(Debug)]
pub struct StickerDetail {
    pub sticker: Sticker,
    pub asset: Asset,
    pub tags: Vec<Tag>,
    pub collections: Vec<memedock_domain::collection::Collection>,
    pub original_path: Option<std::path::PathBuf>,
    pub original_error: Option<ErrorCode>,
}
fn resource_status(
    rows: Vec<(Asset, Option<memedock_domain::local::LocalAsset>)>,
    cache: memedock_storage::files::DerivedStore,
) -> Result<Vec<StickerResource>> {
    rows.into_iter()
        .map(|(asset, mut local)| {
            let ready = match cache.ready(asset.hash()) {
                Ok(ready) => ready,
                Err(error) => {
                    // A broken derived file must not hide the business library.
                    local
                        .get_or_insert_with(|| {
                            memedock_domain::local::LocalAsset::new(asset.hash())
                        })
                        .thumbnail_failed(CoreError::from(error).code().as_str().into());
                    false
                }
            };
            if !ready
                && let Some(state) = &mut local
                && state.thumb_status() == memedock_domain::local::ThumbnailStatus::Ready
            {
                state.evict_thumbnail();
            }
            let thumbnail_path = ready.then(|| cache.thumbnail_path(asset.hash()));
            Ok(StickerResource {
                asset,
                local,
                thumbnail_path,
            })
        })
        .collect()
}
impl Library {
    pub fn sticker_resources(&self, ids: Vec<StickerId>) -> Result<Task<Vec<StickerResource>>> {
        if ids.len() > 200 {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "resource batch exceeds page limit",
            ));
        }
        self.submit(
            Lane::Read,
            Priority::Visible,
            move |services, control| async move {
                control.check()?;
                let hashes: Vec<_> = ids.iter().map(|id| id.content_hash()).collect();
                let rows = services.db.resource_rows(&hashes).await?;
                let cache = services.derived.clone();
                tokio::task::spawn_blocking(move || resource_status(rows, cache)).await?
            },
        )
    }
    pub fn list_stickers(&self, request: QueryRequest) -> Result<Task<QueryResponse>> {
        if request
            .cursor
            .as_ref()
            .is_some_and(|c| c.library_id != self.identity().library_id)
        {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "cursor belongs to another library",
            ));
        }
        self.submit(
            Lane::Read,
            Priority::Interactive,
            move |services, control| async move {
                control.check()?;
                let page = services
                    .db
                    .stickers(
                        &request.query,
                        request.page_size,
                        request.cursor.as_ref().map(|c| &c.inner),
                    )
                    .await?;
                let hashes: Vec<_> = page
                    .stickers
                    .iter()
                    .map(|sticker| sticker.id().content_hash())
                    .collect();
                let rows = services.db.resource_rows(&hashes).await?;
                let cache = services.derived.clone();
                let resources =
                    tokio::task::spawn_blocking(move || resource_status(rows, cache)).await??;
                Ok(QueryResponse {
                    request_id: request.request_id,
                    stickers: page.stickers,
                    next: page.next.map(|inner| QueryCursor {
                        library_id: services.db.identity().library_id,
                        inner,
                    }),
                    resources,
                })
            },
        )
    }
    /// Returns deleted metadata as well; callers can offer explicit recovery.
    pub fn sticker_detail(&self, id: StickerId) -> Result<Task<StickerDetail>> {
        self.submit(
            Lane::Read,
            Priority::Interactive,
            move |services, control| async move {
                control.check()?;
                let sticker = services
                    .db
                    .sticker(id)
                    .await?
                    .ok_or_else(|| CoreError::new(ErrorCode::NotFound, "sticker not found"))?;
                let asset = services.db.asset(id.content_hash()).await?.ok_or_else(|| {
                    CoreError::new(ErrorCode::CorruptData, "sticker asset missing")
                })?;
                let tags = services.db.sticker_tags(id).await?;
                let collections = services.db.sticker_collections(id).await?;
                let blobs = services.blobs.clone();
                let bytes = asset.byte_size().get();
                let original =
                    tokio::task::spawn_blocking(move || -> Result<std::path::PathBuf> {
                        let file = blobs.open_original(id.content_hash())?;
                        if file.metadata()?.len()
                            != u64::try_from(bytes)
                                .map_err(|_| CoreError::internal("asset size overflow"))?
                        {
                            return Err(CoreError::new(
                                ErrorCode::CorruptData,
                                "original size changed",
                            ));
                        }
                        Ok(blobs.original_path(id.content_hash()))
                    })
                    .await?;
                let (original_path, original_error) = match original {
                    Ok(path) => (Some(path), None),
                    Err(error) => (None, Some(error.code())),
                };
                Ok(StickerDetail {
                    sticker,
                    asset,
                    tags,
                    collections,
                    original_path,
                    original_error,
                })
            },
        )
    }
    pub fn collections(
        &self,
        deleted: bool,
    ) -> Result<Task<Vec<memedock_domain::collection::Collection>>> {
        self.submit(
            Lane::Read,
            Priority::Interactive,
            move |services, control| async move {
                control.check()?;
                Ok(services.db.collections(deleted).await?)
            },
        )
    }
    pub fn tags(&self, deleted: bool) -> Result<Task<Vec<Tag>>> {
        self.submit(
            Lane::Read,
            Priority::Interactive,
            move |services, control| async move {
                control.check()?;
                Ok(services.db.tags(deleted).await?)
            },
        )
    }
    pub fn space_statistics(&self) -> Result<Task<SpaceStatistics>> {
        self.submit(
            Lane::Read,
            Priority::Visible,
            move |services, control| async move {
                control.check()?;
                Ok(services.db.space_statistics().await?)
            },
        )
    }
}
