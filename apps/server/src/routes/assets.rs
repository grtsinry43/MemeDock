use super::sync::{MAX_ASSET_BYTES, now};

/// Last owner cleans up on request cancellation as well as ordinary errors.
/// A decoder retains its own Arc, so cleanup cannot race a blocking validation.
struct UploadStaging(memedock_storage::files::PendingStaging);
impl Drop for UploadStaging {
    fn drop(&mut self) {
        let path = self.0.path().to_owned();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn_blocking(move || match std::fs::remove_file(&path) {
                Ok(()) => {
                    if let Some(parent) = path.parent()
                        && let Err(error) = memedock_storage::files::sync_directory(parent)
                    {
                        eprintln!("upload staging sync failed: {error}");
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => eprintln!("upload staging cleanup failed: {error}"),
            });
        }
        // Process termination leaves a .part file for locked startup recovery.
    }
}
use crate::{
    auth,
    error::{ApiError, RequestContext},
    state::AppState,
};
use axum::{
    Extension,
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, Method, header},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use memedock_api_contracts::ApiErrorKind;
use memedock_domain::identity::ContentHash;
use memedock_storage::files::FsBlobStore;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

fn io_error(error: std::io::Error) -> ApiError {
    memedock_storage::StorageError::from(error).into()
}
fn hash(value: &str) -> Result<ContentHash, ApiError> {
    value
        .parse()
        .map_err(|_| ApiError(ApiErrorKind::ParamsError))
}
fn budget(state: &AppState) -> Result<tokio::sync::OwnedSemaphorePermit, ApiError> {
    state
        .transfer_budget
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError(ApiErrorKind::TooManyRequests))
}
async fn store(state: &AppState) -> Result<FsBlobStore, ApiError> {
    let root = state.config.data_dir.join("originals");
    tokio::task::spawn_blocking(move || FsBlobStore::open(root))
        .await
        .map_err(|_| ApiError(ApiErrorKind::ServerError))?
        .map_err(Into::into)
}
pub async fn upload(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(path): Path<String>,
    request: Request,
) -> Response {
    ctx.respond(upload_inner(&state, &path, request).await)
}
async fn upload_inner(
    state: &AppState,
    path: &str,
    request: Request,
) -> Result<memedock_api_contracts::AssetInfo, ApiError> {
    auth::require_device(state, request.headers()).await?;
    let expected = hash(path)?;
    let permit = budget(state)?;
    if request.headers().get(header::CONTENT_ENCODING).is_some() {
        return Err(ApiError(ApiErrorKind::ParamsError));
    }
    if let Some(length) = request.headers().get(header::CONTENT_LENGTH)
        && length
            .to_str()
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .is_none_or(|n| n > MAX_ASSET_BYTES)
    {
        return Err(ApiError(ApiErrorKind::PayloadTooLarge));
    }
    let store = store(state).await?;
    let pending = std::sync::Arc::new(UploadStaging(
        tokio::task::spawn_blocking({
            let store = store.clone();
            move || store.create_staging()
        })
        .await
        .map_err(|_| ApiError(ApiErrorKind::ServerError))??,
    ));
    async {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(pending.0.path())
            .await
            .map_err(io_error)?;
        let mut stream = request.into_body().into_data_stream();
        let mut received = 0u64;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
        while let Some(chunk) = tokio::time::timeout_at(deadline, stream.next())
            .await
            .map_err(|_| ApiError(ApiErrorKind::ParamsError))?
        {
            let chunk = chunk.map_err(|_| ApiError(ApiErrorKind::ParamsError))?;
            received = received
                .checked_add(chunk.len() as u64)
                .ok_or(ApiError(ApiErrorKind::PayloadTooLarge))?;
            if received > MAX_ASSET_BYTES {
                return Err(ApiError(ApiErrorKind::PayloadTooLarge));
            }
            file.write_all(&chunk).await.map_err(io_error)?;
        }
        file.sync_all().await.map_err(io_error)?;
        drop(file);
        let _assets = state
            .asset_write_gate
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ApiError(ApiErrorKind::ServerError))?;
        let (result, _assets) = tokio::task::spawn_blocking({
            let pending = pending.clone();
            move || {
                let _permit = permit;
                let staged = pending.0.finish(MAX_ASSET_BYTES, || false)?;
                if staged.hash() != expected {
                    return Err(ApiError(ApiErrorKind::InvalidAsset));
                }
                let asset = memedock_core::server_sync::validate_original(&staged, now()).map_err(
                    |error| {
                        eprintln!("original validation: {}", error.code().as_str());
                        ApiError(ApiErrorKind::InvalidAsset)
                    },
                )?;
                store.publish(staged, || false)?;
                Ok::<_, ApiError>((asset, _assets))
            }
        })
        .await
        .map_err(|_| ApiError(ApiErrorKind::ServerError))??;
        state
            .db
            .register_server_asset(&result)
            .await
            .map_err(Into::into)
    }
    .await
}
pub async fn download(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(path): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let result = async {
        auth::require_device(&state, &headers).await?;
        original_response(&state, hash(&path)?, method).await
    }
    .await;
    match result {
        Ok(response) => response,
        Err(error) => ctx.respond::<()>(Err(error)),
    }
}

pub async fn admin_original(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: tower_sessions::Session,
    Path(path): Path<String>,
    method: Method,
) -> Response {
    let result = async {
        auth::require_admin(&state, &session).await?;
        original_response(&state, hash(&path)?, method).await
    }
    .await;
    match result {
        Ok(response) => response,
        Err(error) => ctx.respond::<()>(Err(error)),
    }
}

async fn original_response(
    state: &AppState,
    hash: ContentHash,
    method: Method,
) -> Result<Response, ApiError> {
    let permit = budget(state)?;
    if state
        .db
        .server_sticker(hash)
        .await?
        .is_some_and(|v| v.lifecycle().is_purged())
    {
        return Err(ApiError(ApiErrorKind::NotFound));
    }
    let asset = state
        .db
        .server_asset(hash)
        .await?
        .ok_or(ApiError(ApiErrorKind::NotFound))?;
    let store = store(state).await?;
    let size = asset.byte_size().get() as u64;
    let file = tokio::task::spawn_blocking(move || {
        let file = store.open_original(hash)?;
        if file.metadata()?.len() != size {
            return Err(memedock_storage::StorageError::Integrity(
                "original length changed",
            ));
        }
        Ok::<_, memedock_storage::StorageError>(file)
    })
    .await
    .map_err(|_| ApiError(ApiErrorKind::ServerError))??;
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        // Stream owns the permit until completion or client disconnection.
        let stream = ReaderStream::new(tokio::fs::File::from_std(file)).map(move |chunk| {
            let _ = &permit;
            chunk
        });
        Body::from_stream(stream)
    };
    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        asset
            .format()
            .mime()
            .parse()
            .map_err(|_| ApiError(ApiErrorKind::ServerError))?,
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        asset
            .byte_size()
            .get()
            .to_string()
            .parse()
            .map_err(|_| ApiError(ApiErrorKind::ServerError))?,
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "private, no-store"
            .parse()
            .map_err(|_| ApiError(ApiErrorKind::ServerError))?,
    );
    response.headers_mut().insert(
        header::ETAG,
        format!("\"{hash}\"")
            .parse()
            .map_err(|_| ApiError(ApiErrorKind::ServerError))?,
    );
    Ok::<_, ApiError>(response)
}

pub async fn admin_thumbnail(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    session: tower_sessions::Session,
    Path(path): Path<String>,
    method: Method,
) -> Response {
    let result = async {
        auth::require_admin(&state, &session).await?;
        let hash = hash(&path)?;
        let asset_gate = state
            .asset_write_gate
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ApiError(ApiErrorKind::ServerError))?;
        if state
            .db
            .server_sticker(hash)
            .await?
            .is_some_and(|v| v.lifecycle().is_purged())
        {
            return Err(ApiError(ApiErrorKind::NotFound));
        }
        state
            .db
            .server_asset(hash)
            .await?
            .ok_or(ApiError(ApiErrorKind::NotFound))?;
        let request = state
            .thumbnail_requests
            .clone()
            .try_acquire_owned()
            .map_err(|_| ApiError(ApiErrorKind::TooManyRequests))?;
        let permit = state
            .thumbnail_budget
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ApiError(ApiErrorKind::ServerError))?;
        let root = state.config.data_dir.clone();
        let file = tokio::task::spawn_blocking(move || {
            let (_request, _permit, _assets) = (request, permit, asset_gate);
            let blobs = FsBlobStore::open(root.join("originals"))?;
            std::fs::create_dir_all(root.join("cache")).map_err(io_error)?;
            let cache = memedock_storage::files::DerivedStore::open(&root.join("cache"))?;
            let path = memedock_core::server_sync::prepare_thumbnail(&blobs, &cache, hash)
                .map_err(|_| ApiError(ApiErrorKind::InvalidAsset))?;
            std::fs::File::open(path).map_err(io_error)
        })
        .await
        .map_err(|_| ApiError(ApiErrorKind::ServerError))??;
        let length = file.metadata().map_err(io_error)?.len();
        let body = if method == Method::HEAD {
            Body::empty()
        } else {
            Body::from_stream(ReaderStream::new(tokio::fs::File::from_std(file)))
        };
        let mut response = body.into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            "image/png"
                .parse()
                .map_err(|_| ApiError(ApiErrorKind::ServerError))?,
        );
        response.headers_mut().insert(
            header::CONTENT_LENGTH,
            length
                .to_string()
                .parse()
                .map_err(|_| ApiError(ApiErrorKind::ServerError))?,
        );
        Ok::<_, ApiError>(response)
    }
    .await;
    match result {
        Ok(response) => response,
        Err(error) => ctx.respond::<()>(Err(error)),
    }
}
