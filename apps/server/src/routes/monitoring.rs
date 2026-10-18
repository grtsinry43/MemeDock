use crate::{
    auth,
    error::{ApiError, RequestContext, timestamp},
    state::AppState,
};
use axum::{
    Extension,
    extract::{Query, State, rejection::QueryRejection},
    response::Response,
};
use memedock_api_contracts::{
    ActivityEntry, ActivityPage, ActivityQuery, ActivityResult, ApiErrorKind, DeviceSyncProgress,
    SyncOverview,
};
use memedock_storage::server::ActivityFilter;
use tower_sessions::Session;

pub async fn overview(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
) -> Response {
    ctx.respond(
        async {
            auth::require_admin(&state, &session).await?;
            let view = state.db.sync_overview().await?;
            let devices = view
                .devices
                .into_iter()
                .map(|d| {
                    Ok(DeviceSyncProgress {
                        id: d.id,
                        name: d.name,
                        revoked: d.revoked,
                        last_seen_at: d.last_seen_at.map(timestamp).transpose()?,
                        applied_cursor: d.applied_cursor,
                        observed_at: d.observed_at,
                    })
                })
                .collect::<Result<Vec<_>, ApiError>>()?;
            Ok(SyncOverview {
                head: view.head,
                stickers: view.stickers,
                deleted: view.deleted,
                collections: view.collections,
                tags: view.tags,
                original_bytes: view.original_bytes,
                devices,
            })
        }
        .await,
    )
}
pub async fn activity(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
    query: Result<Query<ActivityQuery>, QueryRejection>,
) -> Response {
    ctx.respond(
        async {
            auth::require_admin(&state, &session).await?;
            let Query(q) = query.map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            let filter = ActivityFilter {
                device: q
                    .device_id
                    .map(|s| s.parse())
                    .transpose()
                    .map_err(|_| ApiError(ApiErrorKind::ParamsError))?,
                result: q.result.map(|r| {
                    match r {
                        ActivityResult::Accepted => "accepted",
                        ActivityResult::NoOp => "no_op",
                        ActivityResult::Rejected => "rejected",
                    }
                    .to_owned()
                }),
                page: q.page,
                limit: q.limit,
            };
            let page = state.db.sync_activity(&filter).await?;
            let items = page
                .items
                .into_iter()
                .map(|r| {
                    Ok(ActivityEntry {
                        op_id: r.op_id,
                        device_id: r.device_id,
                        device_name: r.device_name,
                        kind: r.kind,
                        target: r.target,
                        result: match r.result.as_str() {
                            "accepted" => ActivityResult::Accepted,
                            "no_op" => ActivityResult::NoOp,
                            "rejected" => ActivityResult::Rejected,
                            _ => return Err(ApiError(ApiErrorKind::ServerError)),
                        },
                        error_code: r.error_code,
                        received_at: r.received_at,
                        event_seq: r.event_seq,
                    })
                })
                .collect::<Result<Vec<_>, ApiError>>()?;
            Ok(ActivityPage {
                items,
                total: page.total,
                page: q.page,
                limit: q.limit,
            })
        }
        .await,
    )
}
