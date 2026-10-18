use crate::{
    auth::{self, AUTH_VERSION},
    error::{ApiError, RequestContext},
    state::AppState,
};
use axum::{
    Extension, Json,
    extract::{State, rejection::JsonRejection},
    response::Response,
};
use memedock_api_contracts::{AdminInfo, ApiErrorKind, ChangePasswordRequest, LoginRequest};
use tower_sessions::Session;

pub async fn login(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
    body: Result<Json<LoginRequest>, JsonRejection>,
) -> Response {
    ctx.respond(
        async {
            state.check_login_rate()?;
            let Json(body) = body.map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            if body.password.len() > 1024 || body.username.len() > 80 {
                return Err(ApiError(ApiErrorKind::ParamsError));
            }
            let account = state
                .db
                .admin()
                .await?
                .ok_or(ApiError(ApiErrorKind::InvalidCredential))?;
            let valid = auth::verify_password(&state, body.password, account.password_hash).await?;
            if !valid || body.username != account.username {
                return Err(ApiError(ApiErrorKind::InvalidCredential));
            }
            state
                .db
                .delete_expired_sessions(time::OffsetDateTime::now_utc().unix_timestamp())
                .await?;
            session.flush().await?;
            session.insert(AUTH_VERSION, account.auth_version).await?;
            // Surface persistence failure before emitting a successful login response.
            session.save().await?;
            Ok(AdminInfo {
                username: account.username,
            })
        }
        .await,
    )
}
pub async fn me(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
) -> Response {
    ctx.respond(
        async {
            let account = auth::require_admin(&state, &session).await?;
            Ok(AdminInfo {
                username: account.username,
            })
        }
        .await,
    )
}
pub async fn logout(Extension(ctx): Extension<RequestContext>, session: Session) -> Response {
    ctx.respond(
        async {
            session.flush().await?;
            Ok(())
        }
        .await,
    )
}
pub async fn change_password(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: Session,
    body: Result<Json<ChangePasswordRequest>, JsonRejection>,
) -> Response {
    ctx.respond(
        async {
            let account = auth::require_admin(&state, &session).await?;
            state.check_login_rate()?;
            let Json(body) = body.map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            if body.old_password.len() > 1024 || !auth::valid_password(&body.new_password) {
                return Err(ApiError(ApiErrorKind::ParamsError));
            }
            if !auth::verify_password(&state, body.old_password, account.password_hash).await? {
                return Err(ApiError(ApiErrorKind::InvalidCredential));
            }
            let permit = state
                .password_budget
                .clone()
                .try_acquire_owned()
                .map_err(|_| ApiError(ApiErrorKind::TooManyRequests))?;
            let hash = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                auth::hash_password(&body.new_password)
            })
            .await
            .map_err(|_| ApiError(ApiErrorKind::ServerError))??;
            state
                .db
                .replace_password(
                    &account.auth_version,
                    &hash,
                    &uuid::Uuid::now_v7().to_string(),
                )
                .await?;
            session.flush().await?;
            Ok(())
        }
        .await,
    )
}
