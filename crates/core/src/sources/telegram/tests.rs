use super::*;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration,
};
type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
const TOKEN: &str = "123456:abcdefghijklmnopqrstuvwxyz_123456789";
struct Server {
    endpoint: reqwest::Url,
    stop: Arc<AtomicBool>,
    downloads: Arc<AtomicUsize>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new(
        pack: serde_json::Value,
        files: BTreeMap<String, Vec<u8>>,
    ) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = reqwest::Url::parse(&format!("http://{}/", listener.local_addr()?))?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let downloads = Arc::new(AtomicUsize::new(0));
        let downloaded = downloads.clone();
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::Acquire) {
                let (mut stream, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(_) => break,
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                let mut end = None;
                loop {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => request.extend_from_slice(&buffer[..count]),
                    };
                    if request.len() > 32 * 1024 {
                        break;
                    }
                    if let Some(at) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..at]);
                        let size = headers
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if request.len() >= at + 4 + size {
                            end = Some(at + 4);
                            break;
                        }
                    }
                }
                let Some(body_at) = end else {
                    continue;
                };
                let path = String::from_utf8_lossy(&request[..body_at])
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .to_owned();
                let body = if path.ends_with("/getStickerSet") {
                    serde_json::to_vec(&serde_json::json!({"ok":true,"result":pack}))
                        .unwrap_or_default()
                } else if path.ends_with("/getFile") {
                    let file_id = serde_json::from_slice::<serde_json::Value>(&request[body_at..])
                        .ok()
                        .and_then(|v| v["file_id"].as_str().map(str::to_owned))
                        .unwrap_or_default();
                    serde_json::to_vec(&serde_json::json!({"ok":true,"result":{"file_path":format!("stickers/{file_id}.webp")}})).unwrap_or_default()
                } else {
                    downloaded.fetch_add(1, Ordering::AcqRel);
                    files
                        .get(path.rsplit('/').next().unwrap_or(""))
                        .cloned()
                        .unwrap_or_default()
                };
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream
                    .write_all(header.as_bytes())
                    .and_then(|_| stream.write_all(&body));
            }
        });
        Ok(Self {
            endpoint,
            stop,
            downloads,
            thread: Some(thread),
        })
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn png(red: u8) -> std::result::Result<Vec<u8>, image::ImageError> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        2,
        2,
        image::Rgba([red, 50, 100, 150]),
    ))
    .write_to(&mut bytes, image::ImageFormat::Png)?;
    Ok(bytes.into_inner())
}
fn remote_pack() -> serde_json::Value {
    serde_json::json!({"name":"test_pack","title":"猫猫","stickers":[
        {"file_id":"a","file_unique_id":"unique_a","width":2,"height":2,"is_animated":false,"is_video":false,"emoji":"🐱","thumbnail":{"file_id":"thumb","file_unique_id":"thumb_a"}},
        {"file_id":"b","file_unique_id":"unique_b","width":2,"height":2,"is_animated":false,"is_video":false},
        {"file_id":"bad","file_unique_id":"unique_bad","width":2,"height":2,"is_animated":false,"is_video":false}
    ]})
}
async fn pack(library: &Library, server: &Server) -> Result<Arc<TelegramPack>> {
    library
        .load_telegram_pack(
            TOKEN.into(),
            TelegramPackName::new("test_pack".into())?,
            Some(server.endpoint.clone()),
        )?
        .wait()
        .await
}
fn id(value: &str) -> std::result::Result<SourceItemId, memedock_domain::error::DomainError> {
    SourceItemId::new(value.into())
}

#[tokio::test]
async fn selected_import_is_incremental_survives_restart_and_preserves_user_organization()
-> TestResult {
    let server = Server::new(
        remote_pack(),
        BTreeMap::from([
            ("a.webp".into(), png(10)?),
            ("b.webp".into(), png(20)?),
            ("bad.webp".into(), b"broken".to_vec()),
            ("thumb.webp".into(), png(10)?),
        ]),
    )?;
    let directory = tempfile::tempdir()?;
    let config = crate::LibraryConfig::new(
        directory.path().join("data"),
        directory.path().join("cache"),
        directory.path().join("share"),
    );
    let library = Library::open(config.clone()).await?;
    let first = pack(&library, &server).await?;
    assert!(library.collections(false)?.wait().await?.is_empty());
    let report = library
        .import_telegram(first.clone(), vec![id("unique_a")?, id("unique_bad")?])?
        .wait()
        .await?;
    assert_eq!(report.items[0].outcome, TelegramImportOutcome::Created);
    assert!(matches!(
        report.items[1].outcome,
        TelegramImportOutcome::Failed(ErrorCode::UnsupportedFormat | ErrorCode::InvalidImage)
    ));
    assert_eq!(server.downloads.load(Ordering::Acquire), 2);
    let collections = library.collections(false)?.wait().await?;
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].name().as_str(), "Telegram-猫猫");
    let original_collection = collections[0].id();
    library
        .rename_collection(
            original_collection,
            collections[0].lifecycle().generation(),
            Name::new("我的猫猫".into())?,
        )?
        .wait()
        .await?;
    let preview = library
        .telegram_preview(first.clone(), id("unique_a")?)?
        .wait()
        .await?;
    assert!(preview.path.exists());
    library
        .telegram_preview(first, id("unique_a")?)?
        .wait()
        .await?;
    assert_eq!(server.downloads.load(Ordering::Acquire), 3);
    library.close().await?;
    let library = Library::open(config).await?;
    let next = pack(&library, &server).await?;
    assert_eq!(next.stickers()[0].state, SourceImportState::Imported);
    assert_eq!(next.stickers()[1].state, SourceImportState::Available);
    let report = library
        .import_telegram(next.clone(), vec![id("unique_a")?, id("unique_b")?])?
        .wait()
        .await?;
    assert_eq!(report.items[0].outcome, TelegramImportOutcome::Reused);
    assert_eq!(report.items[1].outcome, TelegramImportOutcome::Created);
    assert_eq!(server.downloads.load(Ordering::Acquire), 4);
    assert_eq!(library.collections(false)?.wait().await?.len(), 1);
    let sticker = report.items[0].sticker.ok_or("sticker missing")?;
    let detail = library.sticker_detail(sticker)?.wait().await?;
    library
        .delete_sticker(sticker, detail.sticker.lifecycle().generation())?
        .wait()
        .await?;
    let report = library
        .import_telegram(next, vec![id("unique_a")?])?
        .wait()
        .await?;
    assert_eq!(
        report.items[0].outcome,
        TelegramImportOutcome::RestoreRequired
    );
    assert_eq!(server.downloads.load(Ordering::Acquire), 4);
    library.close().await?;
    Ok(())
}

#[test]
fn links_reject_other_hosts_paths_and_embedded_credentials() -> TestResult {
    for input in [
        "test_pack",
        "https://t.me/addstickers/test_pack",
        "tg://addstickers?set=test_pack",
    ] {
        assert_eq!(parse_pack(input)?.as_str(), "test_pack");
    }
    for input in [
        "https://example.com/addstickers/test_pack",
        "https://token@t.me/addstickers/test_pack",
        "https://t.me/addstickers/test_pack/other",
        "../test_pack",
    ] {
        assert!(parse_pack(input).is_err());
    }
    Ok(())
}

#[tokio::test]
async fn source_mapping_failure_rolls_back_collection_and_survives_restart() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite");
    let db = memedock_storage::LibraryDatabase::open(&path).await?;
    let mut tx = db.begin_write().await?;
    let source = SourceCommit {
        pack: TelegramPackName::new("test_pack".into())?,
        title: "Test".into(),
        item: id("unique_a")?,
    };
    let control = TaskControl::new(Arc::new(tokio::sync::Notify::new()));
    let collection = source_collection(&mut tx, &source, TimestampMs::new(100), &control).await?;
    let missing = memedock_domain::source::SourceItem {
        id: source.item,
        sticker: memedock_domain::identity::StickerId::new(
            memedock_domain::identity::ContentHash::from_bytes([1; 32]),
        ),
    };
    assert!(tx.save_source_item(&missing).await.is_err());
    assert!(tx.commit().await.is_err());
    db.close().await?;
    let db = memedock_storage::LibraryDatabase::open(path).await?;
    assert!(db.source_pack(&source.pack).await?.is_none());
    assert!(db.collection(collection.id()).await?.is_none());
    assert!(db.source_item(&missing.id).await?.is_none());
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn in_flight_http_request_cancels_without_waiting_for_timeout() -> TestResult {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let endpoint = reqwest::Url::parse(&format!("http://{}/", listener.local_addr()?))?;
    let (accepted, ready) = tokio::sync::oneshot::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            let _ = accepted.send(());
            let _ = wait.recv_timeout(Duration::from_secs(5));
            drop(stream);
        }
    });
    let client = client::Client::with_base(TOKEN.into(), endpoint)?;
    let control = TaskControl::new(Arc::new(tokio::sync::Notify::new()));
    let cancel = control.clone();
    let request = tokio::spawn(async move {
        client
            .call::<RemotePack>(
                "getStickerSet",
                serde_json::json!({"name":"test_pack"}),
                &control,
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), ready).await??;
    cancel.cancel();
    let error = tokio::time::timeout(Duration::from_secs(2), request)
        .await??
        .err()
        .ok_or("request did not cancel")?;
    assert_eq!(error.code(), ErrorCode::Cancelled);
    assert!(!format!("{error:?}").contains(TOKEN));
    release.send(())?;
    server.join().map_err(|_| "test server panicked")?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires bash tools/native-media/fetch-test-fixtures.sh"]
async fn real_tgs_and_transparent_webm_convert_through_the_normal_import_transaction() -> TestResult
{
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/native-media/test-fixtures");
    for (file, animated, video, width, height, frames, duration) in [
        ("telegram.tgs", true, false, 512, 512, 180, 3000),
        ("alpha.webm", false, true, 320, 240, 82, 2733),
    ] {
        let converted = tempfile::NamedTempFile::new()?;
        let control = TaskControl::new(Arc::new(tokio::sync::Notify::new()));
        crate::images::telegram::convert(
            if animated {
                TelegramFormat::Tgs
            } else {
                TelegramFormat::Webm
            },
            &fixtures.join(file),
            converted.path(),
            &control,
            32 * 1024 * 1024,
        )?;
        let remote = serde_json::json!({"name":"test_pack","title":"Animations","stickers":[{"file_id":"media","file_unique_id":"unique_media","width":width,"height":height,"is_animated":animated,"is_video":video}]});
        let server = Server::new(
            remote,
            BTreeMap::from([("media.webp".into(), std::fs::read(fixtures.join(file))?)]),
        )?;
        let directory = tempfile::tempdir()?;
        let library = Library::open(crate::LibraryConfig::new(
            directory.path().join("data"),
            directory.path().join("cache"),
            directory.path().join("share"),
        ))
        .await?;
        let pack = pack(&library, &server).await?;
        let report = library
            .import_telegram(pack, vec![id("unique_media")?])?
            .wait()
            .await?;
        assert_eq!(report.items[0].outcome, TelegramImportOutcome::Created);
        let detail = library
            .sticker_detail(report.items[0].sticker.ok_or("sticker missing")?)?
            .wait()
            .await?;
        assert!(detail.asset.animated());
        assert_eq!(
            detail.asset.format(),
            memedock_domain::asset::ImageFormat::WebP
        );
        let bytes = std::fs::read(detail.original_path.ok_or("original missing")?)?;
        let decoded: Vec<_> = webp_animation::Decoder::new(&bytes)?.into_iter().collect();
        assert_eq!(decoded.len(), frames);
        assert_eq!(decoded.last().ok_or("no frames")?.timestamp(), duration);
        assert!(decoded.iter().any(|frame| {
            frame
                .data()
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0 && pixel[3] < 255)
        }));
        assert_eq!(&bytes[38..44], &[0; 6]);
        library.close().await?;
    }
    Ok(())
}

#[tokio::test]
async fn empty_selection_and_cancellation_leave_no_collection() -> TestResult {
    let server = Server::new(remote_pack(), BTreeMap::from([("a.webp".into(), png(10)?)]))?;
    let directory = tempfile::tempdir()?;
    let library = Library::open(crate::LibraryConfig::new(
        directory.path().join("data"),
        directory.path().join("cache"),
        directory.path().join("share"),
    ))
    .await?;
    let pack = pack(&library, &server).await?;
    assert!(library.import_telegram(pack.clone(), Vec::new()).is_err());
    let task = library.import_telegram(pack, vec![id("unique_a")?])?;
    task.cancel()?;
    let report = task.wait().await?;
    assert!(report.stopped);
    assert_eq!(report.items[0].outcome, TelegramImportOutcome::Pending);
    assert!(library.collections(false)?.wait().await?.is_empty());
    library.close().await?;
    Ok(())
}
