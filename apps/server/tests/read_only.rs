use memedock_core::{ImportOptions, Library, LibraryConfig};
use memedock_domain::{
    change::{Operation, OperationKind},
    identity::{DeviceId, OperationId},
    sync::OperationEnvelope,
    tag::Name,
    version::{Generation, TimestampMs},
};
use memedock_server::{auth, config::ServerConfig, state::AppState};
use memedock_storage::server::ServerDatabase;
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Fixture {
    _root: tempfile::TempDir,
    db: ServerDatabase,
    task: tokio::task::JoinHandle<()>,
    origin: String,
    token: String,
    sticker: String,
    collection: String,
    tag: String,
    client: Client,
}
impl Fixture {
    async fn start(port: u16) -> Result<Self> {
        let root = tempfile::tempdir()?;
        let server_dir = root.path().join("server");
        let db = ServerDatabase::open(server_dir.join("server.sqlite")).await?;
        let password = tokio::task::spawn_blocking(|| {
            auth::hash_password("memedock-test-password").map_err(|_| "password hashing")
        })
        .await??;
        db.initialize_admin("admin", &password, "read-only-test")
            .await?;
        let device = DeviceId::new();
        let token = format!(
            "mdk_{device}_{}",
            auth::random_secret().map_err(|_| "token")?
        );
        db.create_device(
            &device.to_string(),
            "macOS 测试机",
            &auth::digest(&token),
            0,
        )
        .await?;
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
        let listen = listener.local_addr()?;
        let origin = format!("http://{listen}");
        let router = memedock_server::router(AppState::new(
            db.clone(),
            ServerConfig {
                data_dir: server_dir,
                listen,
                public_origin: origin.clone(),
            },
        ))?;
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        let library = Library::open(LibraryConfig::new(
            root.path().join("data"),
            root.path().join("cache"),
            root.path().join("exports"),
        ))
        .await?;
        let png = root.path().join("cat.png");
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            512,
            256,
            image::Rgba([68, 132, 210, 160]),
        ))
        .save(&png)?;
        let sticker = library
            .import_file(
                png,
                ImportOptions {
                    title: Some("云端猫猫".into()),
                    original_name: "cat.png".into(),
                    ..Default::default()
                },
            )?
            .wait()
            .await?
            .sticker;
        let collection = library
            .create_collection(Name::new("聊天常用".into())?, None)?
            .wait()
            .await?;
        let tag = library
            .create_tag(Name::new("开心".into())?)?
            .wait()
            .await?;
        library
            .set_sticker_organization(
                sticker.id(),
                Generation::INITIAL,
                Some(Some((collection.id(), Generation::INITIAL))),
                Some(vec![(tag.id(), Generation::INITIAL)]),
            )?
            .wait()
            .await?;
        library
            .connect_sync(origin.clone(), token.clone())?
            .wait()
            .await?;
        for _ in 0..16 {
            if library.synchronize(token.clone())?.wait().await?.pending == 0 {
                break;
            }
        }
        let status = library.sync_status()?.wait().await?;
        assert_eq!(status.pending, 0);
        assert_eq!(status.blocked, 0);
        // A subsequent pull acknowledges the page applied by the preceding cycle.
        library.synchronize(token.clone())?.wait().await?;
        library.close().await?;
        let info = db.sync_identity().await?;
        let client = Client::builder().cookie_store(true).build()?;
        for kind in [
            OperationKind::CreateTag {
                tag_id: tag.id(),
                name: tag.name().clone(),
            },
            OperationKind::DeleteSticker {
                sticker_id: sticker.id(),
                generation: Generation::new(99)?,
            },
        ] {
            let operation = OperationEnvelope::new(
                info.library_id,
                info.epoch,
                device,
                OperationId::new(),
                TimestampMs::new(0),
                Operation::new(kind)?,
            );
            assert_eq!(
                client
                    .post(format!("{origin}/api/v1/device/sync/push"))
                    .bearer_auth(&token)
                    .json(&json!({"operation":operation}))
                    .send()
                    .await?
                    .status(),
                StatusCode::OK
            );
        }
        Ok(Self {
            _root: root,
            db,
            task,
            origin,
            token,
            sticker: sticker.id().to_string(),
            collection: collection.id().to_string(),
            tag: tag.id().to_string(),
            client,
        })
    }
    async fn login(&self) -> Result {
        assert_eq!(
            self.client
                .post(format!("{}/api/v1/admin/login", self.origin))
                .header("Origin", &self.origin)
                .json(&json!({"username":"admin","password":"memedock-test-password"}))
                .send()
                .await?
                .status(),
            StatusCode::OK
        );
        Ok(())
    }
    async fn get(&self, path: &str) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/api/v1/admin{path}", self.origin))
            .send()
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let json: Value = response.json().await?;
        assert_eq!(json["code"], 0);
        Ok(json["data"].clone())
    }
}

#[tokio::test]
async fn real_catalog_assets_receipts_and_acknowledged_progress() -> Result {
    let f = Fixture::start(0).await?;
    for path in [
        "/library/stickers".to_owned(),
        format!("/library/stickers/{}", f.sticker),
        "/sync/overview".into(),
        "/sync/operations".into(),
        format!("/assets/{}/thumbnail", f.sticker),
        format!("/assets/{}/original", f.sticker),
    ] {
        assert_eq!(
            f.client
                .get(format!("{}/api/v1/admin{path}", f.origin))
                .send()
                .await?
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    f.login().await?;
    let list = f.get("/library/stickers").await?;
    assert_eq!(list["total"], 1);
    let detail = f.get(&format!("/library/stickers/{}", f.sticker)).await?;
    assert_eq!(detail["sticker"]["title"], "云端猫猫");
    assert_eq!(detail["collection"]["name"], "聊天常用");
    assert_eq!(detail["tags"][0]["name"], "开心");
    for path in [
        format!("/library/stickers?collectionId={}", f.collection),
        format!("/library/stickers?tagId={}", f.tag),
    ] {
        assert_eq!(f.get(&path).await?["total"], 1);
    }
    assert_eq!(f.get("/library/stickers?starred=true").await?["total"], 0);
    assert_eq!(f.get("/library/stickers?q=cat").await?["total"], 1);
    assert_eq!(f.get("/library/stickers?q=absent").await?["total"], 0);
    assert_eq!(
        f.get("/library/stickers?page=2&limit=1").await?["items"],
        json!([])
    );
    for path in [
        "/library/stickers?page=0",
        "/library/stickers?limit=99999",
        "/library/stickers?tagId=invalid",
        "/sync/operations?result=invalid",
    ] {
        assert_eq!(
            f.client
                .get(format!("{}/api/v1/admin{path}", f.origin))
                .send()
                .await?
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let thumbnail = f
        .client
        .get(format!(
            "{}/api/v1/admin/assets/{}/thumbnail",
            f.origin, f.sticker
        ))
        .send()
        .await?;
    assert_eq!(thumbnail.status(), StatusCode::OK);
    assert_eq!(thumbnail.headers()["content-type"], "image/png");
    assert_eq!(thumbnail.headers()["cache-control"], "no-store");
    let image = image::load_from_memory(&thumbnail.bytes().await?)?;
    assert_eq!((image.width(), image.height()), (256, 128));
    let original = f
        .client
        .get(format!(
            "{}/api/v1/admin/assets/{}/original",
            f.origin, f.sticker
        ))
        .send()
        .await?;
    assert_eq!(original.status(), StatusCode::OK);
    let image = image::load_from_memory(&original.bytes().await?)?;
    assert_eq!((image.width(), image.height()), (512, 256));
    for result in ["accepted", "no_op", "rejected"] {
        let logs = f.get(&format!("/sync/operations?result={result}")).await?;
        assert!(logs["total"].as_u64().is_some_and(|n| n > 0));
        assert_eq!(logs["items"][0]["result"], result);
        assert!(logs["items"][0]["receivedAt"].is_i64());
        let body = logs.to_string();
        assert!(
            !body.contains(&f.token) && !body.contains("token_hash") && !body.contains("request\"")
        );
    }
    let overview = f.get("/sync/overview").await?;
    assert_eq!(overview["devices"][0]["appliedCursor"], overview["head"]);
    // The server observes the request's applied cursor, not the events it sends.
    let identity = f.db.sync_identity().await?;
    let response = f
        .client
        .get(format!(
            "{}/api/v1/device/sync/pull?libraryId={}&syncEpoch={}&after=0&limit=1",
            f.origin, identity.library_id, identity.epoch
        ))
        .bearer_auth(&f.token)
        .send()
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let overview = f.get("/sync/overview").await?;
    assert_eq!(overview["devices"][0]["appliedCursor"], 0);
    assert!(overview["head"].as_i64().is_some_and(|n| n > 0));
    f.task.abort();
    let _ = f.task.await;
    f.db.close().await?;
    Ok(())
}

#[tokio::test]
async fn permanent_delete_propagates_offline_edits_cannot_revive_and_explicit_reimport_syncs()
-> Result {
    use memedock_domain::{
        change::{FieldPatch, StickerPatch},
        lifecycle::EntityId,
    };
    let f = Fixture::start(0).await?;
    let a = Library::open(LibraryConfig::new(
        f._root.path().join("data"),
        f._root.path().join("cache"),
        f._root.path().join("exports"),
    ))
    .await?;
    let device = DeviceId::new();
    let token = format!(
        "mdk_{device}_{}",
        auth::random_secret().map_err(|_| "token")?
    );
    f.db.create_device(&device.to_string(), "second", &auth::digest(&token), 0)
        .await?;
    let second = tempfile::tempdir()?;
    let b = Library::open(LibraryConfig::new(
        second.path().join("data"),
        second.path().join("cache"),
        second.path().join("exports"),
    ))
    .await?;
    b.connect_sync(f.origin.clone(), token.clone())?
        .wait()
        .await?;
    for _ in 0..16 {
        b.synchronize(token.clone())?.wait().await?;
    }
    let id = f.sticker.parse()?;
    let old = a.sticker_detail(id)?.wait().await?.sticker;
    b.patch_sticker(
        id,
        old.lifecycle().generation(),
        StickerPatch::new(
            FieldPatch::Set("offline stale edit".into()),
            FieldPatch::Missing,
            FieldPatch::Missing,
        )?,
    )?
    .wait()
    .await?;
    let deleted = a
        .delete_sticker(id, old.lifecycle().generation())?
        .wait()
        .await?;
    a.purge_entity(
        EntityId::Sticker(id),
        deleted.lifecycle().generation(),
        deleted.lifecycle().revision(),
    )?
    .wait()
    .await?;
    for _ in 0..16 {
        a.synchronize(f.token.clone())?.wait().await?;
    }
    assert_eq!(a.sync_status()?.wait().await?.pending, 0);
    assert!(
        f.db.server_sticker(id.content_hash())
            .await?
            .ok_or("server sticker")?
            .lifecycle()
            .is_purged()
    );
    let server_blobs =
        memedock_storage::files::FsBlobStore::open(f._root.path().join("server/originals"))?;
    assert!(!server_blobs.original_path(id.content_hash()).exists());
    for _ in 0..16 {
        b.synchronize(token.clone())?.wait().await?;
    }
    assert!(b.trash_snapshot()?.wait().await?.is_empty());
    assert!(
        b.sticker_detail(id)?
            .wait()
            .await?
            .sticker
            .lifecycle()
            .is_purged()
    );
    let imported = b
        .import_file(
            f._root.path().join("cat.png"),
            ImportOptions {
                original_name: "fresh.png".into(),
                ..Default::default()
            },
        )?
        .wait()
        .await?
        .sticker;
    assert_eq!(
        imported.lifecycle().generation(),
        old.lifecycle().generation().checked_next()?
    );
    for _ in 0..16 {
        b.synchronize(token.clone())?.wait().await?;
    }
    assert_eq!(b.sync_status()?.wait().await?.pending, 0);
    for _ in 0..16 {
        a.synchronize(f.token.clone())?.wait().await?;
    }
    let fresh = a.sticker_detail(id)?.wait().await?;
    assert_eq!(fresh.sticker.title(), "fresh");
    assert_eq!(
        fresh.sticker.lifecycle().generation(),
        imported.lifecycle().generation()
    );
    assert!(fresh.collection.is_none());
    assert!(fresh.tags.is_empty());
    assert!(fresh.original_path.ok_or("new original")?.exists());
    a.close().await?;
    b.close().await?;
    f.task.abort();
    f.db.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "isolated real-data fixture for read-only Web E2E"]
async fn browser_fixture() -> Result {
    let fixture = Fixture::start(18788).await?;
    std::future::pending::<()>().await;
    fixture.db.close().await?;
    Ok(())
}
