#![cfg(feature = "web")]
use memedock_api_contracts::{PushRequest, PushResponse, SyncInfo};
use memedock_domain::{
    asset::Asset,
    change::{FieldPatch, NamePatch, Operation, OperationKind, StickerPatch},
    identity::{CollectionId, ContentHash, DeviceId, OperationId, TagId},
    sync::{EventKind, OperationEnvelope, PullPage, ReceiptOutcome, RejectionCode},
    version::{Generation, Revision, TimestampMs},
};
use memedock_server::auth;
use memedock_storage::server::ServerDatabase;
use reqwest::{Client, Response, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};
use tokio::io::AsyncWriteExt;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
struct Process {
    child: Child,
    origin: String,
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Process {
    async fn start(dir: &std::path::Path) -> Result<Self> {
        let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
        let address = socket.local_addr()?;
        drop(socket);
        let child = Command::new(env!("CARGO_BIN_EXE_memedock-server"))
            .args(["serve", "--data-dir"])
            .arg(dir)
            .args(["--listen", &address.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let mut process = Self {
            child,
            origin: format!("http://{address}"),
        };
        let client = Client::builder().timeout(Duration::from_secs(2)).build()?;
        for _ in 0..100 {
            if client
                .get(format!("{}/login", process.origin))
                .send()
                .await
                .is_ok()
            {
                return Ok(process);
            }
            if process.child.try_wait()?.is_some() {
                return Err("server exited before becoming ready".into());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err("server readiness timed out".into())
    }
    fn url(&self, path: &str) -> String {
        format!("{}/api/v1{path}", self.origin)
    }
}
async fn payload<T: DeserializeOwned>(response: Response) -> Result<T> {
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value = response.json().await?;
    assert_eq!(value["code"], 0);
    Ok(serde_json::from_value(value["data"].clone())?)
}
async fn post<T: Serialize>(
    client: &Client,
    server: &Process,
    token: &str,
    body: &T,
) -> Result<PushResponse> {
    payload(
        client
            .post(server.url("/device/sync/push"))
            .bearer_auth(token)
            .json(body)
            .send()
            .await?,
    )
    .await
}
fn envelope(info: &SyncInfo, kind: OperationKind, time: i64) -> Result<PushRequest> {
    Ok(PushRequest {
        operation: OperationEnvelope::new(
            info.library_id.parse()?,
            info.sync_epoch.parse()?,
            info.device_id.parse()?,
            OperationId::new(),
            TimestampMs::new(time),
            Operation::new(kind)?,
        ),
    })
}
async fn pull(
    client: &Client,
    server: &Process,
    token: &str,
    info: &SyncInfo,
    after: i64,
    limit: u32,
) -> Result<PullPage> {
    payload(
        client
            .get(server.url(&format!(
                "/device/sync/pull?libraryId={}&syncEpoch={}&after={after}&limit={limit}",
                info.library_id, info.sync_epoch
            )))
            .bearer_auth(token)
            .send()
            .await?,
    )
    .await
}
fn accepted(response: PushResponse) -> Result<i64> {
    match response.receipt.outcome {
        ReceiptOutcome::Accepted { event_seq, .. } => Ok(event_seq.get()),
        other => Err(format!("expected acceptance, received {other:?}").into()),
    }
}

#[tokio::test]
async fn two_real_devices_upload_merge_pull_and_recover_after_process_crash() -> Result {
    let directory = tempfile::tempdir()?;
    let db = ServerDatabase::open(directory.path().join("server.sqlite")).await?;
    let hash = auth::hash_password("sync-e2e-password").map_err(|_| "password setup")?;
    db.initialize_admin("admin", &hash, "test-auth").await?;
    db.close().await?;
    let mut server = Process::start(directory.path()).await?;
    assert!(Process::start(directory.path()).await.is_err());
    let admin = Client::builder().cookie_store(true).build()?;
    let _: serde_json::Value = payload(
        admin
            .post(server.url("/admin/login"))
            .header("Origin", &server.origin)
            .json(&json!({"username":"admin","password":"sync-e2e-password"}))
            .send()
            .await?,
    )
    .await?;
    let mut credentials = Vec::new();
    for name in ["device A", "device B"] {
        let credential: memedock_api_contracts::DeviceCredential = payload(
            admin
                .post(server.url("/admin/devices"))
                .header("Origin", &server.origin)
                .json(&json!({"name":name}))
                .send()
                .await?,
        )
        .await?;
        credentials.push(credential);
    }
    let client = Client::new();
    let a = &credentials[0].token;
    let b = &credentials[1].token;
    let info_a: SyncInfo = payload(
        client
            .get(server.url("/device/sync/info"))
            .bearer_auth(a)
            .send()
            .await?,
    )
    .await?;
    let info_b: SyncInfo = payload(
        client
            .get(server.url("/device/sync/info"))
            .bearer_auth(b)
            .send()
            .await?,
    )
    .await?;
    assert_eq!(info_a.library_id, info_b.library_id);
    assert_ne!(info_a.device_id, info_b.device_id);
    assert_eq!(
        client
            .get(server.url("/device/sync/info"))
            .send()
            .await?
            .status(),
        StatusCode::UNAUTHORIZED
    );
    // A browser cookie does not authorize the device protocol.
    assert_eq!(
        admin
            .get(server.url("/device/sync/info"))
            .send()
            .await?
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgba8(3, 2).write_to(&mut bytes, image::ImageFormat::Png)?;
    let bytes = bytes.into_inner();
    let hash = ContentHash::from_bytes(Sha256::digest(&bytes).into()).to_string();
    let path = format!("/device/assets/{hash}");
    let asset: Asset = payload(
        client
            .put(server.url(&path))
            .bearer_auth(a)
            .body(bytes.clone())
            .send()
            .await?,
    )
    .await?;
    assert_eq!((asset.width(), asset.height()), (3, 2));
    let duplicate: Asset = payload(
        client
            .put(server.url(&path))
            .bearer_auth(b)
            .body(bytes.clone())
            .send()
            .await?,
    )
    .await?;
    assert_eq!(asset, duplicate);
    assert_eq!(
        client
            .put(server.url(&format!("/device/assets/{}", "0".repeat(64))))
            .bearer_auth(a)
            .body(bytes.clone())
            .send()
            .await?
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let invalid = b"not an image";
    assert_eq!(
        client
            .put(server.url(&format!(
                "/device/assets/{}",
                ContentHash::from_bytes(Sha256::digest(invalid).into())
            )))
            .bearer_auth(a)
            .body(invalid.to_vec())
            .send()
            .await?
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let create = envelope(
        &info_a,
        OperationKind::CreateSticker {
            asset: asset.clone(),
            title: "initial".into(),
            original_name: "image.png".into(),
            note: String::new(),
            starred: false,
        },
        100,
    )?;
    let first = post(&client, &server, a, &create).await?;
    let first_seq = accepted(first.clone())?;
    assert_eq!(
        post(&client, &server, a, &create).await?.receipt,
        first.receipt
    );
    let mut reused = serde_json::to_value(&create)?;
    reused["operation"]["operation"]["title"] = json!("different payload");
    let response = post(&client, &server, a, &reused).await?;
    assert!(matches!(
        response.receipt.outcome,
        ReceiptOutcome::Rejected {
            code: RejectionCode::OpIdReused
        }
    ));
    let id = asset.sticker_id();
    let later_edit = envelope(
        &info_b,
        OperationKind::PatchSticker {
            sticker_id: id,
            generation: Generation::INITIAL,
            patch: StickerPatch::new(
                FieldPatch::Set("B submitted first".into()),
                FieldPatch::Missing,
                FieldPatch::Set(true),
            )?,
        },
        9000,
    )?;
    accepted(post(&client, &server, b, &later_edit).await?)?;
    let older_edit = envelope(
        &info_a,
        OperationKind::PatchSticker {
            sticker_id: id,
            generation: Generation::INITIAL,
            patch: StickerPatch::new(
                FieldPatch::Set("A submitted last".into()),
                FieldPatch::Missing,
                FieldPatch::Missing,
            )?,
        },
        1,
    )?;
    let last = accepted(post(&client, &server, a, &older_edit).await?)?;
    let page = pull(&client, &server, b, &info_b, 0, 2).await?;
    assert!(page.has_more());
    assert_eq!(page.events().len(), 2);
    let page = pull(&client, &server, b, &info_b, page.next_cursor().get(), 2).await?;
    assert!(!page.has_more());
    assert_eq!(page.next_cursor().get(), last);
    match page.events().last().ok_or("patch missing")?.event() {
        EventKind::StickerPatched { entity } => {
            assert_eq!(entity.title(), "A submitted last");
            assert!(entity.starred());
        }
        _ => return Err("unexpected event".into()),
    }
    let collection_id = CollectionId::new();
    let tag_id = TagId::new();
    for kind in [
        OperationKind::CreateCollection {
            collection_id,
            name: "Collection".parse()?,
            before_id: None,
        },
        OperationKind::CreateTag {
            tag_id,
            name: "Tag".parse()?,
        },
        OperationKind::SetStickerCollection {
            sticker_id: id,
            sticker_generation: Generation::INITIAL,
            collection: Some((collection_id, Generation::INITIAL)),
        },
        OperationKind::SetTagMembership {
            sticker_id: id,
            tag_id,
            sticker_generation: Generation::INITIAL,
            tag_generation: Generation::INITIAL,
            present: true,
        },
        OperationKind::PatchTag {
            tag_id,
            generation: Generation::INITIAL,
            patch: NamePatch::new("renamed".parse()?),
        },
    ] {
        accepted(post(&client, &server, a, &envelope(&info_a, kind, 1)?).await?)?;
    }
    let delete = envelope(
        &info_b,
        OperationKind::DeleteSticker {
            sticker_id: id,
            generation: Generation::INITIAL,
        },
        4,
    )?;
    let deleted = accepted(post(&client, &server, b, &delete).await?)?;
    let late = envelope(
        &info_a,
        OperationKind::PatchSticker {
            sticker_id: id,
            generation: Generation::INITIAL,
            patch: StickerPatch::new(
                FieldPatch::Set("late".into()),
                FieldPatch::Missing,
                FieldPatch::Missing,
            )?,
        },
        99,
    )?;
    assert!(matches!(
        post(&client, &server, a, &late).await?.receipt.outcome,
        ReceiptOutcome::Rejected {
            code: RejectionCode::EntityDeleted
        }
    ));
    accepted(
        post(
            &client,
            &server,
            b,
            &envelope(
                &info_b,
                OperationKind::RestoreSticker {
                    sticker_id: id,
                    expected_deleted_revision: Revision::new(deleted)?,
                    rebuild: None,
                },
                5,
            )?,
        )
        .await?,
    )?;
    let stale = envelope(
        &info_a,
        OperationKind::DeleteSticker {
            sticker_id: id,
            generation: Generation::INITIAL,
        },
        99,
    )?;
    assert!(matches!(
        post(&client, &server, a, &stale).await?.receipt.outcome,
        ReceiptOutcome::Rejected {
            code: RejectionCode::StaleGeneration
        }
    ));
    assert_eq!(
        client
            .head(server.url(&path))
            .bearer_auth(b)
            .send()
            .await?
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
        Some(bytes.len().to_string())
    );
    assert_eq!(
        client
            .get(server.url(&path))
            .bearer_auth(b)
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        bytes
    );
    // Crash while an upload actually owns a staging file, then recover at startup.
    let staging = directory.path().join("originals/staging");
    let mut partial =
        tokio::net::TcpStream::connect(server.origin.trim_start_matches("http://")).await?;
    partial.write_all(format!("PUT /api/v1{path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {a}\r\nContent-Length: 1000000\r\n\r\nabc", server.origin.trim_start_matches("http://")).as_bytes()).await?;
    for _ in 0..100 {
        if std::fs::read_dir(&staging)?.next().is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(std::fs::read_dir(&staging)?.next().is_some());
    drop(partial);
    for _ in 0..100 {
        if std::fs::read_dir(&staging)?.next().is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(std::fs::read_dir(&staging)?.next().is_none());
    let mut partial =
        tokio::net::TcpStream::connect(server.origin.trim_start_matches("http://")).await?;
    partial.write_all(format!("PUT /api/v1{path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {a}\r\nContent-Length: 1000000\r\n\r\nabc", server.origin.trim_start_matches("http://")).as_bytes()).await?;
    for _ in 0..100 {
        if std::fs::read_dir(&staging)?.next().is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(std::fs::read_dir(&staging)?.next().is_some());
    drop(server);
    drop(partial);
    server = Process::start(directory.path()).await?;
    assert!(std::fs::read_dir(&staging)?.next().is_none());
    let reopened: SyncInfo = payload(
        client
            .get(server.url("/device/sync/info"))
            .bearer_auth(a)
            .send()
            .await?,
    )
    .await?;
    assert_eq!(reopened.library_id, info_a.library_id);
    assert_eq!(reopened.sync_epoch, info_a.sync_epoch);
    assert!(reopened.head > first_seq);
    assert_eq!(
        post(&client, &server, a, &create).await?.receipt,
        first.receipt
    );
    assert_eq!(
        client
            .get(server.url(&path))
            .bearer_auth(b)
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        bytes
    );
    // Revoke through persisted storage; no cached credential may remain usable.
    let db = ServerDatabase::open(directory.path().join("server.sqlite")).await?;
    let device: DeviceId = info_b.device_id.parse()?;
    db.revoke_device(&device.to_string(), 100).await?;
    db.close().await?;
    assert_eq!(
        client
            .get(server.url(&path))
            .bearer_auth(b)
            .send()
            .await?
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .get(server.url("/device/sync/info"))
            .bearer_auth(b)
            .send()
            .await?
            .status(),
        StatusCode::UNAUTHORIZED
    );
    Ok(())
}
