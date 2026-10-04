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
}
#[derive(Debug)]
pub struct StickerDetail {
    pub sticker: Sticker,
    pub asset: Asset,
    pub tags: Vec<Tag>,
}
impl Library {
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
                Ok(QueryResponse {
                    request_id: request.request_id,
                    stickers: page.stickers,
                    next: page.next.map(|inner| QueryCursor {
                        library_id: services.db.identity().library_id,
                        inner,
                    }),
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
                Ok(StickerDetail {
                    sticker,
                    asset,
                    tags,
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
