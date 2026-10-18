use crate::{
    auth,
    error::{ApiError, RequestContext},
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{
        Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::HeaderMap,
    response::Response,
};
use memedock_api_contracts::{ApiErrorKind, PullRequest, PushRequest, PushResponse, SyncInfo};
use memedock_domain::version::{ProtocolVersion, Revision, TimestampMs};
pub const MAX_PUSH_BYTES: usize = 64 * 1024;
pub const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;
pub fn now() -> TimestampMs {
    TimestampMs::new((time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64)
}

pub async fn info(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
) -> Response {
    ctx.respond(
        async {
            let device = auth::require_device(&state, &headers).await?;
            let identity = state.db.sync_identity().await?;
            Ok(SyncInfo {
                protocol_version: ProtocolVersion::V1.get(),
                library_id: identity.library_id.to_string(),
                sync_epoch: identity.epoch.to_string(),
                device_id: device.to_string(),
                head: identity.head.get(),
                max_push_bytes: MAX_PUSH_BYTES as u32,
                max_pull_events: 256,
                max_asset_bytes: MAX_ASSET_BYTES,
            })
        }
        .await,
    )
}
pub async fn push(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    body: Result<Json<PushRequest>, JsonRejection>,
) -> Response {
    ctx.respond(
        async {
            let device = auth::require_device(&state, &headers).await?;
            let Json(body) = body.map_err(|e| {
                ApiError(if e.status() == axum::http::StatusCode::PAYLOAD_TOO_LARGE {
                    ApiErrorKind::PayloadTooLarge
                } else {
                    ApiErrorKind::ParamsError
                })
            })?;
            let assets = state
                .asset_write_gate
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| ApiError(ApiErrorKind::ServerError))?;
            // Once admitted, retain the gate through blocking cleanup even if
            // the HTTP client disconnects. At most one such worker is admitted.
            let receipt = tokio::spawn(async move {
                let _assets = assets;
                let receipt =
                    memedock_core::server_sync::push(&state.db, device, body.operation, now())
                        .await?;
                // The receipt is durable; preserve it if filesystem cleanup fails.
                if let Err(error) = memedock_core::server_sync::cleanup_purged(
                    &state.db,
                    &state.config.data_dir.join("originals"),
                    &state.config.data_dir.join("cache"),
                )
                .await
                {
                    eprintln!("permanent-delete cleanup failed: {error}");
                }
                Ok::<_, memedock_storage::StorageError>(receipt)
            })
            .await
            .map_err(|_| ApiError(ApiErrorKind::ServerError))??;
            Ok(PushResponse {
                protocol_version: ProtocolVersion::V1,
                receipt,
            })
        }
        .await,
    )
}
pub async fn pull(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    query: Result<Query<PullRequest>, QueryRejection>,
) -> Response {
    ctx.respond(
        async {
            let device = auth::require_device(&state, &headers).await?;
            let Query(query) = query.map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            let library = query
                .library_id
                .parse()
                .map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            let epoch = query
                .sync_epoch
                .parse()
                .map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            let after =
                Revision::new(query.after).map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            Ok(state
                .db
                .pull_events_observed(device, (library, epoch), after, query.limit, Some(now()))
                .await?)
        }
        .await,
    )
}
