use axum::response::IntoResponse;
use memedock_core::{ErrorCode, ImportOptions, Library, LibraryConfig, RestoreMode};
use memedock_domain::{
    change::{FieldPatch, StickerPatch},
    identity::DeviceId,
    tag::Name,
    version::Generation,
};
use memedock_server::{auth, config::ServerConfig, state::AppState};
use memedock_storage::server::ServerDatabase;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn config(path: &std::path::Path) -> LibraryConfig {
    LibraryConfig::new(
        path.join("library"),
        path.join("cache"),
        path.join("exports"),
    )
}
async fn sync(library: &Library, token: &str) -> Result {
    for _ in 0..16 {
        let state = library.synchronize(token.to_owned())?.wait().await?;
        if state.pending == 0 {
            assert_eq!(state.blocked, 0);
            return Ok(());
        }
    }
    Err("sync did not drain".into())
}

#[tokio::test]
async fn real_libraries_merge_preserve_offline_edits_and_resume() -> Result {
    let root = tempfile::tempdir()?;
    let server_dir = root.path().join("server");
    let db = ServerDatabase::open(server_dir.join("server.sqlite")).await?;
    let mut tokens = Vec::new();
    for name in ["a", "b"] {
        let id = DeviceId::new();
        let token = format!(
            "mdk_{id}_{}",
            auth::random_secret().map_err(|_| "random token")?
        );
        db.create_device(&id.to_string(), name, &auth::digest(&token), 0)
            .await?;
        tokens.push(token);
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let listen = listener.local_addr()?;
    let origin = format!("http://{listen}");
    let lose_reply = Arc::new(AtomicBool::new(true));
    let app = memedock_server::router(AppState::new(
        db.clone(),
        ServerConfig {
            data_dir: server_dir,
            listen,
            public_origin: origin.clone(),
        },
    ))?
    .layer(axum::middleware::from_fn(
        move |request: axum::extract::Request, next: axum::middleware::Next| {
            let lose_reply = lose_reply.clone();
            async move {
                let push = request.uri().path() == "/api/v1/device/sync/push";
                let response = next.run(request).await;
                if push
                    && response.status().is_success()
                    && lose_reply.swap(false, Ordering::SeqCst)
                {
                    // Commit succeeded, but the caller never receives its receipt.
                    axum::http::StatusCode::BAD_GATEWAY.into_response()
                } else {
                    response
                }
            }
        },
    ));
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let ca = config(&root.path().join("a"));
    let cb = config(&root.path().join("b"));
    let a = Library::open(ca.clone()).await?;
    let b = Library::open(cb.clone()).await?;
    let png = root.path().join("sticker.png");
    image::DynamicImage::new_rgba8(4, 3).save(&png)?;
    let imported = a
        .import_file(
            png.clone(),
            ImportOptions {
                title: Some("cloud title".into()),
                original_name: "cloud.png".into(),
                ..Default::default()
            },
        )?
        .wait()
        .await?;
    let id = imported.sticker.id();
    b.import_file(
        png,
        ImportOptions {
            title: Some("local title".into()),
            original_name: "local.png".into(),
            ..Default::default()
        },
    )?
    .wait()
    .await?;
    a.connect_sync(origin.clone(), tokens[0].clone())?
        .wait()
        .await?;
    let interrupted = a
        .synchronize(tokens[0].clone())?
        .wait()
        .await
        .err()
        .ok_or("lost receipt did not interrupt sync")?;
    assert_eq!(interrupted.code(), ErrorCode::Network);
    a.close().await?;
    let a = Library::open(ca.clone()).await?;
    sync(&a, &tokens[0]).await?;
    b.connect_sync(origin.clone(), tokens[1].clone())?
        .wait()
        .await?;
    sync(&b, &tokens[1]).await?;
    assert_eq!(
        b.sticker_detail(id)?.wait().await?.sticker.title(),
        "cloud title"
    );
    a.patch_sticker(
        id,
        Generation::INITIAL,
        StickerPatch::new(
            FieldPatch::Set("edited offline".into()),
            FieldPatch::Missing,
            FieldPatch::Missing,
        )?,
    )?
    .wait()
    .await?;
    b.patch_sticker(
        id,
        Generation::INITIAL,
        StickerPatch::new(
            FieldPatch::Set("edited later but submitted first".into()),
            FieldPatch::Set("remote note".into()),
            FieldPatch::Missing,
        )?,
    )?
    .wait()
    .await?;
    a.set_sync_paused(true)?.wait().await?;
    let paused = a.synchronize(tokens[0].clone())?.wait().await?;
    assert!(paused.paused && paused.pending > 0);
    sync(&b, &tokens[1]).await?;
    a.set_sync_paused(false)?.wait().await?;
    sync(&a, &tokens[0]).await?;
    let detail = a.sticker_detail(id)?.wait().await?;
    assert_eq!(detail.sticker.title(), "edited offline");
    assert_eq!(detail.sticker.note(), "remote note");
    sync(&b, &tokens[1]).await?;
    assert_eq!(
        b.sticker_detail(id)?.wait().await?.sticker.title(),
        "edited offline"
    );
    a.close().await?;
    let a = Library::open(ca).await?;
    assert!(a.sync_status()?.wait().await?.connected);
    sync(&a, &tokens[0]).await?;
    // Restoring a portable backup expands into new commands, including a new
    // tag attached to a sticker that already exists remotely.
    let backup_source = Library::open(config(&root.path().join("backup"))).await?;
    let archive = a.create_backup()?.wait().await?;
    let input = backup_source.create_archive_input()?.wait().await?;
    std::fs::copy(&archive.path, input.path())?;
    a.discard_backup(archive)?.wait().await?;
    let prepared = backup_source.inspect_archive(input)?.wait().await?;
    backup_source
        .restore_archive(prepared.clone(), RestoreMode::Merge)?
        .wait()
        .await?;
    backup_source
        .discard_prepared_archive(prepared)?
        .wait()
        .await?;
    let tag = backup_source
        .create_tag(Name::new("from backup".into())?)?
        .wait()
        .await?;
    backup_source
        .set_sticker_organization(
            id,
            Generation::INITIAL,
            None,
            Some(vec![(tag.id(), Generation::INITIAL)]),
        )?
        .wait()
        .await?;
    let archive = backup_source.create_backup()?.wait().await?;
    let input = a.create_archive_input()?.wait().await?;
    std::fs::copy(&archive.path, input.path())?;
    backup_source.discard_backup(archive)?.wait().await?;
    let prepared = a.inspect_archive(input)?.wait().await?;
    a.restore_archive(prepared.clone(), RestoreMode::Merge)?
        .wait()
        .await?;
    a.discard_prepared_archive(prepared)?.wait().await?;
    backup_source.close().await?;
    sync(&a, &tokens[0]).await?;
    sync(&b, &tokens[1]).await?;
    assert!(
        b.sticker_detail(id)?
            .wait()
            .await?
            .tags
            .iter()
            .any(|t| t.id() == tag.id())
    );
    a.patch_sticker(
        id,
        Generation::INITIAL,
        StickerPatch::new(
            FieldPatch::Set("stale offline edit".into()),
            FieldPatch::Missing,
            FieldPatch::Missing,
        )?,
    )?
    .wait()
    .await?;
    b.delete_sticker(id, Generation::INITIAL)?.wait().await?;
    sync(&b, &tokens[1]).await?;
    let rejected = a.synchronize(tokens[0].clone())?.wait().await?;
    assert_eq!(rejected.pending, 0);
    assert_eq!(rejected.blocked, 1);
    assert!(
        !a.sticker_detail(id)?
            .wait()
            .await?
            .sticker
            .lifecycle()
            .is_active()
    );
    a.create_tag(Name::new("after rejected edit".into())?)?
        .wait()
        .await?;
    let continued = a.synchronize(tokens[0].clone())?.wait().await?;
    assert_eq!(continued.pending, 0);
    assert_eq!(continued.blocked, 1);
    let device = a
        .sync_status()?
        .wait()
        .await?
        .device_id
        .ok_or("device missing")?;
    db.revoke_device(&device, 1).await?;
    let error = a
        .synchronize(tokens[0].clone())?
        .wait()
        .await
        .err()
        .ok_or("revoked token succeeded")?;
    assert_eq!(error.code(), ErrorCode::Unauthorized);
    assert_eq!(
        a.sync_status()?.wait().await?.error.as_deref(),
        Some("unauthorized")
    );
    a.close().await?;
    b.close().await?;
    server.abort();
    let _ = server.await;
    db.close().await?;
    Ok(())
}
