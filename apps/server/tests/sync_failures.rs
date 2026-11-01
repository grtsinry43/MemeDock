use axum::{
    body::Body,
    http::{Method, StatusCode},
    response::IntoResponse,
};
use memedock_core::{ErrorCode, ImportOptions, Library, LibraryConfig};
use memedock_domain::identity::{DeviceId, StickerId};
use memedock_server::{auth, config::ServerConfig, state::AppState};
use memedock_storage::server::ServerDatabase;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering},
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
struct Server {
    root: tempfile::TempDir,
    origin: String,
    tokens: [String; 2],
    fault: Arc<AtomicU16>,
    hash: Arc<Mutex<String>>,
    requests: Arc<AtomicUsize>,
    corrupt_length: Arc<AtomicUsize>,
    fail_push: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn start() -> Result<Self> {
        let root = tempfile::tempdir()?;
        let data_dir = root.path().join("server");
        let db = ServerDatabase::open(data_dir.join("server.sqlite")).await?;
        let mut tokens = Vec::new();
        for name in ["source", "receiver"] {
            let id = DeviceId::new();
            let token = format!("mdk_{id}_{}", auth::random_secret().map_err(|_| "token")?);
            db.create_device(&id.to_string(), name, &auth::digest(&token), 0)
                .await?;
            tokens.push(token);
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let listen = listener.local_addr()?;
        let origin = format!("http://{listen}");
        let fault = Arc::new(AtomicU16::new(0));
        let hash = Arc::new(Mutex::new(String::new()));
        let requests = Arc::new(AtomicUsize::new(0));
        let corrupt_length = Arc::new(AtomicUsize::new(0));
        let c = corrupt_length.clone();
        let fail_push = Arc::new(AtomicBool::new(false));
        let p = fail_push.clone();
        let (f, h, r) = (fault.clone(), hash.clone(), requests.clone());
        let app = memedock_server::router(AppState::new(db, ServerConfig { data_dir, listen, public_origin: origin.clone() }))?
            .layer(axum::middleware::from_fn(move |request: axum::extract::Request, next: axum::middleware::Next| {
                let (fault, hash, requests) = (f.clone(), h.clone(), r.clone());
                let corrupt_length = c.clone();
                let fail_push = p.clone();
                async move {
                    if request.uri().path().ends_with("/device/sync/push") && fail_push.load(Ordering::SeqCst) {
                        return StatusCode::BAD_GATEWAY.into_response();
                    }
                    if request.method() == Method::GET && request.uri().path().contains("/device/assets/") {
                        requests.fetch_add(1, Ordering::SeqCst);
                        let target = hash.lock().map(|v| v.clone()).unwrap_or_default();
                        let code = fault.load(Ordering::SeqCst);
                        if code > 0 && request.uri().path().ends_with(&target) {
                            let mut response = if code == 200 {
                                (StatusCode::OK, Body::from("truncated")).into_response()
                            } else if code == 1001 {
                                (StatusCode::OK, Body::from(vec![0; corrupt_length.load(Ordering::SeqCst)])).into_response()
                            } else {
                                (StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), axum::Json(serde_json::json!({
                                    "code": code, "bizErr": "INJECTED_FAILURE", "msg": "injected download failure",
                                    "data": null, "meta": {"requestId": "fault-request", "timestamp": "2026-11-01T00:00:00Z"}
                                }))).into_response()
                            };
                            response.headers_mut().insert("x-request-id", "fault-request".parse().unwrap_or_else(|_| axum::http::HeaderValue::from_static("fault")));
                            if code == 429 { response.headers_mut().insert("retry-after", axum::http::HeaderValue::from_static("120")); }
                            return response;
                        }
                    }
                    next.run(request).await
                }
            }));
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let tokens = tokens.try_into().map_err(|_| "two tokens")?;
        Ok(Self {
            root,
            origin,
            tokens,
            fault,
            hash,
            requests,
            corrupt_length,
            fail_push,
            task,
        })
    }
    fn config(&self, name: &str) -> LibraryConfig {
        let path = self.root.path().join(name);
        LibraryConfig::new(
            path.join("library"),
            path.join("cache"),
            path.join("exports"),
        )
    }
    async fn import(&self, source: &Library, width: u32) -> Result<StickerId> {
        let path = self.root.path().join(format!("{width}.png"));
        image::DynamicImage::new_rgba8(width, 3).save(&path)?;
        Ok(source
            .import_file(path, ImportOptions::default())?
            .wait()
            .await?
            .sticker
            .id())
    }
    fn inject(&self, id: StickerId, code: u16) -> Result {
        if code == 1001 {
            let store = memedock_storage::files::FsBlobStore::open(
                self.root.path().join("server/originals"),
            )?;
            self.corrupt_length.store(
                store.open_original(id.content_hash())?.metadata()?.len() as usize,
                Ordering::SeqCst,
            );
        }
        *self.hash.lock().map_err(|_| "fault lock")? = id.to_string();
        self.fault.store(code, Ordering::SeqCst);
        Ok(())
    }
    async fn upload(&self, source: &Library) -> Result {
        source
            .connect_sync(self.origin.clone(), self.tokens[0].clone())?
            .wait()
            .await?;
        for _ in 0..16 {
            if source
                .synchronize(self.tokens[0].clone())?
                .wait()
                .await?
                .pending
                == 0
            {
                return Ok(());
            }
        }
        Err("upload did not drain".into())
    }
}

#[tokio::test]
async fn missing_download_stays_out_of_library_and_backup_while_other_files_continue() -> Result {
    let server = Server::start().await?;
    let source = Library::open(server.config("source")).await?;
    let bad = server.import(&source, 4).await?;
    let good = server.import(&source, 5).await?;
    let collection = source
        .create_collection("pending collection".parse()?, None)?
        .wait()
        .await?;
    let tag = source.create_tag("pending tag".parse()?)?.wait().await?;
    source
        .set_sticker_organization(
            bad,
            memedock_domain::version::Generation::INITIAL,
            Some(Some((
                collection.id(),
                memedock_domain::version::Generation::INITIAL,
            ))),
            Some(vec![(
                tag.id(),
                memedock_domain::version::Generation::INITIAL,
            )]),
        )?
        .wait()
        .await?;
    server.upload(&source).await?;
    server.inject(bad, 404)?;
    let config = server.config("receiver");
    let receiver = Library::open(config.clone()).await?;
    receiver
        .connect_sync(server.origin.clone(), server.tokens[1].clone())?
        .wait()
        .await?;
    let state = receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await?;
    assert_eq!(state.downloading, 1);
    assert!(
        state
            .diagnostic
            .as_deref()
            .is_some_and(|v| v.contains("404") && v.contains("fault-request"))
    );
    assert!(receiver.sticker_detail(bad)?.wait().await.is_err());
    assert!(
        receiver
            .sticker_detail(good)?
            .wait()
            .await?
            .original_path
            .is_some()
    );
    let backup = receiver.create_backup()?.wait().await?;
    assert_eq!(backup.pending_sync_items, 1);
    let input = receiver.create_archive_input()?.wait().await?;
    std::fs::copy(&backup.path, input.path())?;
    let archive = receiver.inspect_archive(input)?.wait().await?;
    assert_eq!(archive.summary().stickers, 1);
    receiver.discard_prepared_archive(archive)?.wait().await?;
    receiver.discard_backup(backup)?.wait().await?;
    receiver.close().await?;
    server.fault.store(0, Ordering::SeqCst);
    let receiver = Library::open(config).await?;
    let state = receiver.sync_status()?.wait().await?;
    tokio::time::sleep(std::time::Duration::from_millis(state.retry_after_ms)).await;
    let state = receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await?;
    assert_eq!(state.pending, 0);
    assert!(state.error.is_none());
    assert!(state.last_success_ms.is_some());
    assert!(
        receiver
            .sticker_detail(bad)?
            .wait()
            .await?
            .original_path
            .is_some()
    );
    assert!(!state.recent_errors.is_empty());
    let detail = receiver.sticker_detail(bad)?.wait().await?;
    assert_eq!(detail.collection.map(|c| c.id()), Some(collection.id()));
    assert_eq!(
        detail.tags.iter().map(|t| t.id()).collect::<Vec<_>>(),
        vec![tag.id()]
    );
    receiver.close().await?;
    source.close().await?;
    Ok(())
}

#[tokio::test]
async fn download_429_retains_http_details_and_persists_cooldown_across_restart() -> Result {
    let server = Server::start().await?;
    let source = Library::open(server.config("source")).await?;
    let id = server.import(&source, 4).await?;
    server.upload(&source).await?;
    server.inject(id, 429)?;
    let config = server.config("receiver");
    let receiver = Library::open(config.clone()).await?;
    receiver
        .connect_sync(server.origin.clone(), server.tokens[1].clone())?
        .wait()
        .await?;
    let error = receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await
        .err()
        .ok_or("expected 429")?;
    assert_eq!(error.code(), ErrorCode::RateLimited);
    let status = receiver.sync_status()?.wait().await?;
    assert!(status.retry_after_ms > 110_000);
    assert!(
        status
            .diagnostic
            .as_deref()
            .is_some_and(|v| v.contains("429") && v.contains("fault-request"))
    );
    assert!(status.last_success_ms.is_none());
    receiver.close().await?;
    let receiver = Library::open(config).await?;
    let requests = server.requests.load(Ordering::SeqCst);
    receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await?;
    assert_eq!(server.requests.load(Ordering::SeqCst), requests);
    let backup = receiver.create_backup()?.wait().await?;
    assert_eq!(backup.pending_sync_items, 1);
    receiver.discard_backup(backup)?.wait().await?;
    receiver.close().await?;
    source.close().await?;
    Ok(())
}

#[tokio::test]
async fn truncated_original_never_becomes_published() -> Result {
    let server = Server::start().await?;
    let source = Library::open(server.config("source")).await?;
    let id = server.import(&source, 4).await?;
    server.upload(&source).await?;
    server.inject(id, 200)?;
    let receiver = Library::open(server.config("receiver")).await?;
    receiver
        .connect_sync(server.origin.clone(), server.tokens[1].clone())?
        .wait()
        .await?;
    let status = receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await?;
    assert_eq!(status.downloading, 1);
    assert!(status.last_success_ms.is_none());
    assert!(receiver.sticker_detail(id)?.wait().await.is_err());
    let backup = receiver.create_backup()?.wait().await?;
    receiver.discard_backup(backup)?.wait().await?;
    receiver.close().await?;
    source.close().await?;
    Ok(())
}

#[tokio::test]
async fn same_length_corrupt_original_fails_hash_verification_without_publication() -> Result {
    let server = Server::start().await?;
    let source = Library::open(server.config("source")).await?;
    let id = server.import(&source, 4).await?;
    server.upload(&source).await?;
    server.inject(id, 1001)?;
    let receiver = Library::open(server.config("receiver")).await?;
    receiver
        .connect_sync(server.origin.clone(), server.tokens[1].clone())?
        .wait()
        .await?;
    let status = receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await?;
    assert_eq!(status.downloading, 1);
    assert!(
        status
            .diagnostic
            .as_deref()
            .is_some_and(|v| v.contains("hash mismatch"))
    );
    assert!(receiver.sticker_detail(id)?.wait().await.is_err());
    receiver.close().await?;
    source.close().await?;
    Ok(())
}

#[tokio::test]
async fn published_file_without_database_commit_is_recovered_without_redownload() -> Result {
    let server = Server::start().await?;
    let source = Library::open(server.config("source")).await?;
    let id = server.import(&source, 4).await?;
    server.upload(&source).await?;
    server.inject(id, 404)?;
    let config = server.config("receiver");
    let receiver = Library::open(config.clone()).await?;
    receiver
        .connect_sync(server.origin.clone(), server.tokens[1].clone())?
        .wait()
        .await?;
    receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await?;
    receiver.close().await?;
    // The exact durable state of a crash after publishing bytes but before
    // committing the local ready flag or the business projection.
    let original = source
        .sticker_detail(id)?
        .wait()
        .await?
        .original_path
        .ok_or("source original")?;
    let store = memedock_storage::files::FsBlobStore::open(config.data_dir.clone())?;
    let pending = store.create_staging()?;
    std::fs::copy(original, pending.path())?;
    let staged = pending.finish(1024 * 1024, || false)?;
    store.publish(staged, || false)?;
    let requests = server.requests.load(Ordering::SeqCst);
    let receiver = Library::open(config).await?;
    assert!(
        receiver
            .sticker_detail(id)?
            .wait()
            .await?
            .original_path
            .is_some()
    );
    assert_eq!(server.requests.load(Ordering::SeqCst), requests);
    let backup = receiver.create_backup()?.wait().await?;
    assert_eq!(backup.pending_sync_items, 0);
    receiver.discard_backup(backup)?.wait().await?;
    receiver.close().await?;
    source.close().await?;
    Ok(())
}

#[tokio::test]
async fn damaged_local_original_is_quarantined_and_repaired_without_losing_metadata() -> Result {
    let server = Server::start().await?;
    let source = Library::open(server.config("source")).await?;
    let id = server.import(&source, 4).await?;
    server.upload(&source).await?;
    let config = server.config("receiver");
    let receiver = Library::open(config.clone()).await?;
    receiver
        .connect_sync(server.origin.clone(), server.tokens[1].clone())?
        .wait()
        .await?;
    receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await?;
    let detail = receiver.sticker_detail(id)?.wait().await?;
    let sticker = detail.sticker.clone();
    let path = detail.original_path.ok_or("original path")?;
    receiver.close().await?;
    std::fs::write(&path, b"damaged")?;
    let receiver = Library::open(config.clone()).await?;
    assert!(receiver.sticker_detail(id)?.wait().await.is_err());
    let backup = receiver.create_backup()?.wait().await?;
    assert_eq!(backup.pending_sync_items, 1);
    receiver.discard_backup(backup)?.wait().await?;
    receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await?;
    assert_eq!(receiver.sticker_detail(id)?.wait().await?.sticker, sticker);
    let entries: Vec<_> = std::fs::read_dir(config.data_dir.join("damaged"))?
        .collect::<std::result::Result<_, _>>()?;
    assert_eq!(entries.len(), 1);
    assert_eq!(std::fs::read(entries[0].path())?, b"damaged");
    receiver.close().await?;
    source.close().await?;
    Ok(())
}

#[tokio::test]
async fn old_client_is_rejected_before_any_sync_write() -> Result {
    let server = Server::start().await?;
    let response = reqwest::Client::new()
        .post(format!("{}/api/v1/device/sync/push", server.origin))
        .bearer_auth(&server.tokens[0])
        .json(&serde_json::json!({}))
        .send()
        .await?;
    assert_eq!(response.status().as_u16(), 426);
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body["bizErr"], "UNSUPPORTED_PROTOCOL");
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn inaccessible_original_keeps_library_accessible_and_recovers_after_permission_repair()
-> Result {
    use std::os::unix::fs::PermissionsExt;
    let server = Server::start().await?;
    let source = Library::open(server.config("source")).await?;
    let id = server.import(&source, 4).await?;
    server.upload(&source).await?;
    let config = server.config("receiver");
    let receiver = Library::open(config.clone()).await?;
    receiver
        .connect_sync(server.origin.clone(), server.tokens[1].clone())?
        .wait()
        .await?;
    receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await?;
    let path = receiver
        .sticker_detail(id)?
        .wait()
        .await?
        .original_path
        .ok_or("original")?;
    receiver.close().await?;
    let permissions = std::fs::metadata(&path)?.permissions();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o0))?;
    let receiver = Library::open(config).await?;
    let state = receiver.sync_status()?.wait().await?;
    assert_eq!(state.error.as_deref(), Some("permission_denied"));
    assert_eq!(state.downloading, 1);
    let backup = receiver.create_backup()?.wait().await?;
    receiver.discard_backup(backup)?.wait().await?;
    std::fs::set_permissions(path, permissions)?;
    let state = receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await?;
    assert_eq!(state.pending, 0);
    assert!(state.error.is_none());
    assert!(
        receiver
            .sticker_detail(id)?
            .wait()
            .await?
            .original_path
            .is_some()
    );
    receiver.close().await?;
    source.close().await?;
    Ok(())
}

#[tokio::test]
async fn offline_edit_conflicting_with_pulled_deletion_does_not_prevent_reopening_after_network_failure()
-> Result {
    use memedock_domain::{
        change::{FieldPatch, StickerPatch},
        version::Generation,
    };
    let server = Server::start().await?;
    let source = Library::open(server.config("source")).await?;
    let id = server.import(&source, 4).await?;
    server.upload(&source).await?;
    let config = server.config("receiver");
    let receiver = Library::open(config.clone()).await?;
    receiver
        .connect_sync(server.origin.clone(), server.tokens[1].clone())?
        .wait()
        .await?;
    receiver
        .synchronize(server.tokens[1].clone())?
        .wait()
        .await?;
    receiver
        .patch_sticker(
            id,
            Generation::INITIAL,
            StickerPatch::new(
                FieldPatch::Set("offline edit".into()),
                FieldPatch::Missing,
                FieldPatch::Missing,
            )?,
        )?
        .wait()
        .await?;
    source
        .delete_sticker(id, Generation::INITIAL)?
        .wait()
        .await?;
    source.synchronize(server.tokens[0].clone())?.wait().await?;
    server.fail_push.store(true, Ordering::SeqCst);
    assert_eq!(
        receiver
            .synchronize(server.tokens[1].clone())?
            .wait()
            .await
            .err()
            .ok_or("expected failed push")?
            .code(),
        ErrorCode::Network
    );
    receiver.close().await?;
    let receiver = Library::open(config).await?;
    assert!(
        !receiver
            .sticker_detail(id)?
            .wait()
            .await?
            .sticker
            .lifecycle()
            .is_active()
    );
    let status = receiver.sync_status()?.wait().await?;
    assert!(status.pending > 0);
    assert!(status.cooldown_until_ms.is_some());
    let backup = receiver.create_backup()?.wait().await?;
    receiver.discard_backup(backup)?.wait().await?;
    receiver.close().await?;
    source.close().await?;
    Ok(())
}
