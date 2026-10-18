use crate::{
    auth,
    error::{ApiError, RequestContext},
    state::AppState,
};
use axum::{
    Extension,
    extract::{Path, Query, State, rejection::QueryRejection},
    response::Response,
};
use memedock_api_contracts::{
    ApiErrorKind, FacetQuery, LibraryAsset, LibraryDetail, LibraryFacet, LibraryFacets,
    LibraryPage, LibraryQuery, LibrarySticker,
};
use memedock_storage::server::{BrowseFilter, BrowseSticker};
use tower_sessions::Session;

fn sticker(item: BrowseSticker) -> LibrarySticker {
    let s = item.sticker;
    let a = item.asset;
    LibrarySticker {
        id: s.id().to_string(),
        title: s.title().into(),
        original_name: s.original_name().into(),
        starred: s.starred(),
        deleted: !s.lifecycle().is_active(),
        created_at: s.lifecycle().created_at().get(),
        updated_at: s.lifecycle().updated_at().get(),
        asset: LibraryAsset {
            mime: a.format().mime().into(),
            width: a.width(),
            height: a.height(),
            animated: a.animated(),
            byte_size: a.byte_size().get(),
        },
    }
}
pub async fn list(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
    query: Result<Query<LibraryQuery>, QueryRejection>,
) -> Response {
    ctx.respond(
        async {
            auth::require_admin(&state, &session).await?;
            let Query(q) = query.map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            let filter = BrowseFilter {
                query: q.q,
                collection: q
                    .collection_id
                    .map(|s| s.parse())
                    .transpose()
                    .map_err(|_| ApiError(ApiErrorKind::ParamsError))?,
                tag: q
                    .tag_id
                    .map(|s| s.parse())
                    .transpose()
                    .map_err(|_| ApiError(ApiErrorKind::ParamsError))?,
                starred: q.starred,
                deleted: q.deleted,
                page: q.page,
                limit: q.limit,
            };
            let page = state.db.browse_stickers(&filter).await?;
            Ok(LibraryPage {
                items: page.items.into_iter().map(sticker).collect(),
                total: page.total,
                page: q.page,
                limit: q.limit,
            })
        }
        .await,
    )
}
pub async fn detail(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
    Path(id): Path<String>,
) -> Response {
    ctx.respond(
        async {
            auth::require_admin(&state, &session).await?;
            let id = id
                .parse()
                .map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            let detail = state
                .db
                .browse_detail(id)
                .await?
                .ok_or(ApiError(ApiErrorKind::NotFound))?;
            let note = detail.item.sticker.note().to_owned();
            Ok(LibraryDetail {
                sticker: sticker(detail.item),
                note,
                collection: detail.collection.map(|c| LibraryFacet {
                    id: c.id().to_string(),
                    name: c.name().as_str().into(),
                }),
                tags: detail
                    .tags
                    .into_iter()
                    .map(|t| LibraryFacet {
                        id: t.id().to_string(),
                        name: t.name().as_str().into(),
                    })
                    .collect(),
            })
        }
        .await,
    )
}
async fn facets(
    state: &AppState,
    session: &Session,
    query: Result<Query<FacetQuery>, QueryRejection>,
    collections: bool,
) -> Result<LibraryFacets, ApiError> {
    auth::require_admin(state, session).await?;
    let Query(q) = query.map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
    let page = state.db.browse_facets(collections, &q.q).await?;
    Ok(LibraryFacets {
        items: page
            .items
            .into_iter()
            .map(|v| LibraryFacet {
                id: v.id,
                name: v.name,
            })
            .collect(),
        has_more: page.has_more,
    })
}
pub async fn collections(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
    query: Result<Query<FacetQuery>, QueryRejection>,
) -> Response {
    ctx.respond(facets(&state, &session, query, true).await)
}
pub async fn tags(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
    query: Result<Query<FacetQuery>, QueryRejection>,
) -> Response {
    ctx.respond(facets(&state, &session, query, false).await)
}
