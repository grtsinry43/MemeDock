use crate::{
    auth,
    error::{ApiError, RequestContext, timestamp},
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    response::Response,
};
use memedock_api_contracts::{
    ApiErrorKind, CreateDeviceRequest, DeviceCredential, DeviceInfo, DeviceList,
};
use memedock_domain::identity::DeviceId;
use memedock_storage::server::StoredDevice;
use tower_sessions::Session;

fn info(row: StoredDevice) -> Result<DeviceInfo, ApiError> {
    Ok(DeviceInfo {
        id: row.id,
        name: row.name,
        created_at: timestamp(row.created_at)?,
        revoked_at: row.revoked_at.map(timestamp).transpose()?,
        last_seen_at: row.last_seen_at.map(timestamp).transpose()?,
    })
}
pub async fn list(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
) -> Response {
    ctx.respond(
        async {
            auth::require_admin(&state, &session).await?;
            Ok(DeviceList {
                items: state
                    .db
                    .devices()
                    .await?
                    .into_iter()
                    .map(info)
                    .collect::<Result<_, _>>()?,
            })
        }
        .await,
    )
}
pub async fn create(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
    body: Result<Json<CreateDeviceRequest>, JsonRejection>,
) -> Response {
    ctx.respond(
        async {
            auth::require_admin(&state, &session).await?;
            let Json(body) = body.map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            let name = body.name.trim();
            if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
                return Err(ApiError(ApiErrorKind::ParamsError));
            }
            let id = DeviceId::new().to_string();
            let token = format!("mdk_{id}_{}", auth::random_secret()?);
            let row = state
                .db
                .create_device(
                    &id,
                    name,
                    &auth::digest(&token),
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                )
                .await?;
            Ok(DeviceCredential {
                device: info(row)?,
                token,
            })
        }
        .await,
    )
}
pub async fn revoke(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
    Path(id): Path<String>,
) -> Response {
    ctx.respond(
        async {
            auth::require_admin(&state, &session).await?;
            id.parse::<DeviceId>()
                .map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            if !state
                .db
                .revoke_device(&id, time::OffsetDateTime::now_utc().unix_timestamp())
                .await?
            {
                return Err(ApiError(ApiErrorKind::NotFound));
            }
            Ok(())
        }
        .await,
    )
}
/// Real credential check for client setup; never accepts an administrator cookie.
pub async fn verify(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
) -> Response {
    ctx.respond(
        async {
            let id = auth::require_device(&state, &headers).await?;
            let row = state
                .db
                .device(&id.to_string())
                .await?
                .ok_or(ApiError(ApiErrorKind::InvalidCredential))?;
            info(row)
        }
        .await,
    )
}
