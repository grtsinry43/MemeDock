pub mod auth;
pub mod config;
pub mod error;
mod routes;
mod session_store;
pub mod state;
mod static_assets;

use axum::{
    Router,
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderValue, Method, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post, put},
};
use error::{ApiError, RequestContext};
use memedock_api_contracts::ApiErrorKind;
use state::AppState;
use tower_sessions::{Expiry, SessionManagerLayer, cookie::SameSite};

pub fn router(state: AppState) -> Result<Router, String> {
    let secure = state.config.validate()?;
    let sessions = SessionManagerLayer::new(session_store::SqliteSessions(state.db.clone()))
        .with_name("memedock_session")
        .with_path("/api/v1")
        .with_http_only(true)
        .with_secure(secure)
        .with_same_site(SameSite::Strict)
        .with_expiry(Expiry::OnInactivity(time::Duration::days(7)));
    Ok(Router::new()
        .route("/api/v1/admin/login", post(routes::auth::login))
        .route("/api/v1/admin/me", get(routes::auth::me))
        .route("/api/v1/admin/logout", post(routes::auth::logout))
        .route("/api/v1/admin/password", put(routes::auth::change_password))
        .route(
            "/api/v1/admin/assets/{hash}/original",
            get(routes::assets::admin_original),
        )
        .route(
            "/api/v1/admin/assets/{hash}/thumbnail",
            get(routes::assets::admin_thumbnail),
        )
        .route("/api/v1/admin/library/stickers", get(routes::library::list))
        .route(
            "/api/v1/admin/library/stickers/{id}",
            get(routes::library::detail),
        )
        .route(
            "/api/v1/admin/library/collections",
            get(routes::library::collections),
        )
        .route("/api/v1/admin/library/tags", get(routes::library::tags))
        .route(
            "/api/v1/admin/sync/overview",
            get(routes::monitoring::overview),
        )
        .route(
            "/api/v1/admin/sync/operations",
            get(routes::monitoring::activity),
        )
        .route(
            "/api/v1/admin/devices",
            get(routes::devices::list).post(routes::devices::create),
        )
        .route(
            "/api/v1/admin/devices/{id}",
            axum::routing::delete(routes::devices::revoke),
        )
        .route("/api/v1/device/verify", get(routes::devices::verify))
        .route("/api/v1/device/sync/info", get(routes::sync::info))
        .route(
            "/api/v1/device/sync/push",
            post(routes::sync::push).layer(DefaultBodyLimit::max(routes::sync::MAX_PUSH_BYTES)),
        )
        .route("/api/v1/device/sync/pull", get(routes::sync::pull))
        .route(
            "/api/v1/device/assets/{hash}",
            get(routes::assets::download)
                .put(routes::assets::upload)
                .layer(DefaultBodyLimit::disable()),
        )
        .fallback(static_assets::serve)
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(sessions)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            request_boundary,
        ))
        .with_state(state))
}

async fn request_boundary(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let ctx = RequestContext::default();
    let api = request.uri().path().starts_with("/api/");
    let asset_download = (request.uri().path().starts_with("/api/v1/device/assets/")
        || request.uri().path().starts_with("/api/v1/admin/assets/"))
        && matches!(*request.method(), Method::GET | Method::HEAD);
    let admin_write = request.uri().path().starts_with("/api/v1/admin/")
        && !matches!(
            *request.method(),
            Method::GET | Method::HEAD | Method::OPTIONS
        );
    let valid_origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        == Some(state.config.public_origin.as_str());
    request.extensions_mut().insert(ctx.clone());
    let mut response = if admin_write && !valid_origin {
        ctx.respond::<()>(Err(ApiError(ApiErrorKind::Forbidden)))
    } else {
        next.run(request).await
    };
    if api
        && !(asset_download && response.status().is_success())
        && !response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|v| v.as_bytes().starts_with(b"application/json"))
    {
        let kind = match response.status().as_u16() {
            404 => ApiErrorKind::NotFound,
            405 => ApiErrorKind::MethodNotAllowed,
            413 => ApiErrorKind::PayloadTooLarge,
            400..=499 => ApiErrorKind::ParamsError,
            _ => ApiErrorKind::ServerError,
        };
        response = ctx.respond::<()>(Err(ApiError(kind)));
    }
    let headers = response.headers_mut();
    if let Ok(id) = HeaderValue::from_str(&ctx.id) {
        headers.insert("x-request-id", id);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response
}
