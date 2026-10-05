type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
#[path = "common/image_fixtures.rs"]
mod image_fixtures;
fn config(root: &std::path::Path) -> memedock_core::LibraryConfig {
    memedock_core::LibraryConfig::new(root.join("data"), root.join("cache"), root.join("export"))
}
use memedock_core::{
    ErrorCode, ImportOptions, ImportStatus, Library, QueryRequest, RequestId, StickerQuery,
    tasks::Priority,
};
use memedock_domain::{
    change::{FieldPatch, Operation, OperationKind, StickerPatch},
    identity::OperationId,
    version::{Revision, TimestampMs},
};
use std::{io::Cursor, path::Path, time::Duration};

fn png() -> TestResult<Vec<u8>> {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        400,
        200,
        image::Rgba([20, 80, 180, 128]),
    ));
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, image::ImageFormat::Png)?;
    Ok(output.into_inner())
}
async fn import(
    library: &Library,
    bytes: &[u8],
    name: &str,
) -> TestResult<memedock_core::ImportOutcome> {
    let input = library.create_import_input()?.wait().await?;
    std::fs::write(input.path(), bytes)?;
    Ok(library
        .import_staged(
            input,
            ImportOptions {
                original_name: name.into(),
                ..Default::default()
            },
        )?
        .wait()
        .await?)
}
async fn query(library: &Library, text: &str) -> TestResult<memedock_core::QueryResponse> {
    Ok(library
        .list_stickers(QueryRequest {
            request_id: RequestId::new(),
            query: StickerQuery {
                text: text.into(),
                ..Default::default()
            },
            page_size: 30,
            cursor: None,
        })?
        .wait()
        .await?)
}
async fn db(root: &Path) -> TestResult<memedock_storage::LibraryDatabase> {
    Ok(memedock_storage::LibraryDatabase::open(root.join("library.sqlite")).await?)
}

#[tokio::test]
async fn real_import_dedup_thumbnail_and_restart_preserve_original_and_metadata() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let bytes = png()?;
    let library = Library::open(settings.clone()).await?;
    let created = import(&library, &bytes, "猫猫.png").await?;
    assert_eq!(created.status, ImportStatus::Created);
    let reused = import(&library, &bytes, "changed.jpg").await?;
    assert_eq!(reused.status, ImportStatus::Reused);
    assert_eq!(reused.sticker.title(), "猫猫");
    let thumb = tokio::time::timeout(
        Duration::from_secs(10),
        library
            .request_thumbnail(created.sticker.id(), Priority::Visible)?
            .wait(),
    )
    .await??;
    let decoded = image::open(&thumb.path)?;
    assert_eq!((decoded.width(), decoded.height()), (256, 128));
    assert_eq!(decoded.to_rgba8().get_pixel(0, 0)[3], 128);
    let original = memedock_storage::files::FsBlobStore::open(&settings.data_dir)?
        .original_path(created.sticker.id().content_hash());
    assert_eq!(std::fs::read(original)?, bytes);
    assert_eq!(query(&library, "猫猫").await?.stickers.len(), 1);
    assert!(query(&library, "不存在").await?.stickers.is_empty());
    library.close().await?;
    let database = db(&settings.data_dir).await?;
    assert_eq!(database.changes_after(None, 30).await?.len(), 1);
    database.close().await?;
    let reopened = Library::open(settings).await?;
    let page = query(&reopened, "猫猫").await?;
    assert_eq!(page.stickers.len(), 1);
    assert!(page.resources[0].thumbnail_path.is_some());
    reopened.close().await?;
    Ok(())
}

#[tokio::test]
async fn concurrent_identical_imports_create_one_original_and_one_log() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let library = Library::open(settings.clone()).await?;
    let bytes = png()?;
    let mut tasks = Vec::new();
    for _ in 0..5 {
        let input = library.create_import_input()?.wait().await?;
        std::fs::write(input.path(), &bytes)?;
        tasks.push(library.import_staged(input, ImportOptions::default())?);
    }
    let mut created = 0;
    for task in tasks {
        if task.wait().await?.status == ImportStatus::Created {
            created += 1;
        }
    }
    assert_eq!(created, 1);
    library.close().await?;
    let database = db(&settings.data_dir).await?;
    assert_eq!(database.space_statistics().await?.known_assets, 1);
    assert_eq!(database.changes_after(None, 20).await?.len(), 1);
    database.close().await?;
    Ok(())
}

#[tokio::test]
async fn duplicate_keeps_edits_and_deleted_sticker_requires_explicit_recovery() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let library = Library::open(settings.clone()).await?;
    let bytes = png()?;
    let mut sticker = import(&library, &bytes, "initial.png").await?.sticker;
    library.close().await?;
    let database = db(&settings.data_dir).await?;
    let patch = StickerPatch::new(
        FieldPatch::Set("编辑后标题".into()),
        FieldPatch::Set("备注".into()),
        FieldPatch::Set(true),
    )?;
    sticker.patch(
        sticker.lifecycle().generation(),
        &patch,
        TimestampMs::new(200),
        Revision::LOCAL,
    )?;
    let mut tx = database.begin_write().await?;
    tx.save_sticker(&sticker).await?;
    tx.append_change(
        OperationId::new(),
        Operation::new(OperationKind::PatchSticker {
            sticker_id: sticker.id(),
            generation: sticker.lifecycle().generation(),
            patch,
        })?,
        TimestampMs::new(200),
    )
    .await?;
    tx.commit().await?;
    database.close().await?;
    let library = Library::open(settings.clone()).await?;
    let reused = import(&library, &bytes, "other.webp").await?;
    assert_eq!(reused.sticker, sticker);
    library.close().await?;
    let database = db(&settings.data_dir).await?;
    sticker.delete(
        sticker.lifecycle().generation(),
        TimestampMs::new(300),
        Revision::LOCAL,
    )?;
    let mut tx = database.begin_write().await?;
    tx.save_sticker(&sticker).await?;
    tx.append_change(
        OperationId::new(),
        Operation::new(OperationKind::DeleteSticker {
            sticker_id: sticker.id(),
            generation: sticker.lifecycle().generation(),
        })?,
        TimestampMs::new(300),
    )
    .await?;
    tx.commit().await?;
    database.close().await?;
    let library = Library::open(settings).await?;
    let result = import(&library, &bytes, "again.png").await?;
    assert_eq!(result.status, ImportStatus::RestoreRequired);
    assert_eq!(result.sticker, sticker);
    assert!(query(&library, "").await?.stickers.is_empty());
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_and_oversized_inputs_leave_no_records_or_live_staging() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut settings = config(dir.path());
    settings.limits.max_frame_pixels = 100;
    let library = Library::open(settings.clone()).await?;
    for (bytes, code) in [
        (b"not a picture".to_vec(), ErrorCode::UnsupportedFormat),
        (png()?, ErrorCode::ResourceLimit),
        (
            b"\x89PNG\r\n\x1a\ninvalid".to_vec(),
            ErrorCode::InvalidImage,
        ),
    ] {
        let error = import(&library, &bytes, "untrusted.png")
            .await
            .err()
            .ok_or("expected invalid image")?;
        let core = error
            .downcast_ref::<memedock_core::CoreError>()
            .ok_or("core error")?;
        assert_eq!(core.code(), code);
    }
    assert!(query(&library, "").await?.stickers.is_empty());
    assert_eq!(
        std::fs::read_dir(settings.data_dir.join("staging"))?.count(),
        0
    );
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn thumbnail_publication_failure_keeps_import_and_can_be_retried() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let library = Library::open(settings.clone()).await?;
    let hash =
        memedock_domain::identity::ContentHash::from_bytes(sha2::Sha256::digest(png()?).into());
    let target = settings
        .cache_dir
        .join("thumbnails")
        .join(format!("thumb-v1-256-{hash}.png"));
    std::fs::create_dir(&target)?;
    let imported = import(&library, &png()?, "cat.png").await?;
    assert_eq!(imported.status, ImportStatus::Created);
    assert!(
        library
            .request_thumbnail(imported.sticker.id(), Priority::Visible)?
            .wait()
            .await
            .is_err()
    );
    assert_eq!(library.space_statistics()?.wait().await?.known_assets, 1);
    let page = query(&library, "").await?;
    assert_eq!(page.stickers.len(), 1);
    assert_eq!(
        page.resources[0]
            .local
            .as_ref()
            .ok_or("local state")?
            .thumb_status(),
        memedock_domain::local::ThumbnailStatus::Failed
    );
    std::fs::remove_dir(&target)?;
    assert!(
        library
            .request_thumbnail(imported.sticker.id(), Priority::Visible)?
            .wait()
            .await?
            .path
            .is_file()
    );
    library.close().await?;
    Ok(())
}

use sha2::Digest;

#[tokio::test]
async fn partial_gif_updates_are_composed_onto_the_full_canvas() -> TestResult {
    let directory = tempfile::tempdir()?;
    let library = Library::open(config(directory.path())).await?;
    let mut gif = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut gif);
        for color in [[200, 10, 20, 255], [10, 200, 20, 255]] {
            encoder.encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
                40,
                20,
                image::Rgba(color),
            )))?;
        }
    }
    gif[6..8].copy_from_slice(&80u16.to_le_bytes());
    gif[8..10].copy_from_slice(&40u16.to_le_bytes());
    let palette = if gif[10] & 0x80 != 0 {
        3 * (1usize << ((gif[10] & 7) + 1))
    } else {
        0
    };
    let mut at = 13 + palette;
    while gif[at] == 0x21 {
        at += 2;
        while gif[at] != 0 {
            at += usize::from(gif[at]) + 1;
        }
        at += 1;
    }
    assert_eq!(gif[at], 0x2c);
    gif[at + 1..at + 3].copy_from_slice(&10u16.to_le_bytes());
    gif[at + 3..at + 5].copy_from_slice(&8u16.to_le_bytes());
    let imported = import(&library, &gif, "partial.gif").await?;
    let thumbnail = library
        .request_thumbnail(imported.sticker.id(), Priority::Visible)?
        .wait()
        .await?;
    let display = image::open(thumbnail.path)?.to_rgba8();
    assert_eq!(display.dimensions(), (80, 40));
    assert_eq!(display.get_pixel(10, 8).0, [200, 10, 20, 255]);
    assert_eq!(display.get_pixel(0, 0)[3], 0);
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn corrupt_later_animation_frames_are_rejected_before_business_commit() -> TestResult {
    let directory = tempfile::tempdir()?;
    let library = Library::open(config(directory.path())).await?;
    let mut webp = image_fixtures::animated_webp()?;
    webp.truncate(webp.len() - 10);
    let mut apng = image_fixtures::apng()?;
    let offset = apng
        .windows(4)
        .position(|bytes| bytes == b"fdAT")
        .ok_or("frame chunk")?;
    apng[offset + 8] ^= 0x80;
    for bytes in [webp, apng] {
        let error = import(&library, &bytes, "broken-animation")
            .await
            .err()
            .ok_or("expected invalid frame")?;
        assert_eq!(
            error
                .downcast_ref::<memedock_core::CoreError>()
                .ok_or("core error")?
                .code(),
            ErrorCode::InvalidImage
        );
    }
    assert!(query(&library, "").await?.stickers.is_empty());
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn animated_webp_and_apng_use_composed_first_frames() -> TestResult {
    let directory = tempfile::tempdir()?;
    let library = Library::open(config(directory.path())).await?;
    for (bytes, name, pixel) in [
        (image_fixtures::animated_webp()?, "animated.webp", (8, 6)),
        (image_fixtures::apng()?, "animated.png", (0, 0)),
    ] {
        let imported = import(&library, &bytes, name).await?;
        assert!(
            library
                .sticker_detail(imported.sticker.id())?
                .wait()
                .await?
                .asset
                .animated()
        );
        let thumbnail = library
            .request_thumbnail(imported.sticker.id(), Priority::Visible)?
            .wait()
            .await?;
        let display = image::open(thumbnail.path)?.to_rgba8();
        assert_eq!(display.dimensions(), (40, 20));
        assert_eq!(display.get_pixel(pixel.0, pixel.1).0, [200, 10, 20, 128]);
        if name.ends_with("webp") {
            assert_eq!(display.get_pixel(0, 0)[3], 0);
        }
    }
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn jpeg_exif_orientation_changes_only_the_thumbnail() -> TestResult {
    let directory = tempfile::tempdir()?;
    let settings = config(directory.path());
    let library = Library::open(settings.clone()).await?;
    let mut jpeg = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        40,
        20,
        image::Rgb([20, 80, 180]),
    ))
    .write_to(&mut jpeg, image::ImageFormat::Jpeg)?;
    let jpeg = jpeg.into_inner();
    // Exif TIFF little-endian IFD0: Orientation SHORT, one value, Rotate90.
    let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut bytes = jpeg[..2].to_vec();
    bytes.extend([0xff, 0xe1]);
    bytes.extend(u16::try_from(exif.len() + 2)?.to_be_bytes());
    bytes.extend(exif);
    bytes.extend(&jpeg[2..]);
    let imported = import(&library, &bytes, "oriented.jpeg").await?;
    let thumbnail = library
        .request_thumbnail(imported.sticker.id(), Priority::Visible)?
        .wait()
        .await?;
    let display = image::open(thumbnail.path)?;
    assert_eq!((display.width(), display.height()), (20, 40));
    let original = memedock_storage::files::FsBlobStore::open(&settings.data_dir)?
        .original_path(imported.sticker.id().content_hash());
    assert_eq!(std::fs::read(original)?, bytes);
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn jpeg_webp_and_multiframe_gif_are_validated_and_get_static_thumbnails() -> TestResult {
    let directory = tempfile::tempdir()?;
    let library = Library::open(config(directory.path())).await?;
    let pixels = image::RgbImage::from_pixel(40, 20, image::Rgb([30, 90, 170]));
    for (format, name) in [
        (image::ImageFormat::Jpeg, "photo.jpeg"),
        (image::ImageFormat::WebP, "photo.webp"),
    ] {
        let mut buffer = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(pixels.clone()).write_to(&mut buffer, format)?;
        let imported = import(&library, &buffer.into_inner(), name).await?;
        let detail = library
            .sticker_detail(imported.sticker.id())?
            .wait()
            .await?;
        assert!(!detail.asset.animated());
        let thumbnail = library
            .request_thumbnail(imported.sticker.id(), Priority::Visible)?
            .wait()
            .await?;
        assert_eq!(image::open(thumbnail.path)?.width(), 40);
    }
    let mut gif = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut gif);
        for color in [[200, 10, 20, 255], [10, 200, 20, 255]] {
            encoder.encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
                40,
                20,
                image::Rgba(color),
            )))?;
        }
    }
    let imported = import(&library, &gif, "animated.gif").await?;
    assert!(
        library
            .sticker_detail(imported.sticker.id())?
            .wait()
            .await?
            .asset
            .animated()
    );
    let thumbnail = library
        .request_thumbnail(imported.sticker.id(), Priority::Visible)?
        .wait()
        .await?;
    assert_eq!(
        image::open(thumbnail.path)?.to_rgba8().get_pixel(0, 0).0,
        [200, 10, 20, 255]
    );
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn restart_cleans_abandoned_staging_and_resets_interrupted_thumbnail() -> TestResult {
    use memedock_domain::local::{LocalAsset, ThumbnailStatus};
    let directory = tempfile::tempdir()?;
    let settings = config(directory.path());
    let library = Library::open(settings.clone()).await?;
    let imported = import(&library, &png()?, "cat.png").await?;
    let abandoned = library.create_import_input()?.wait().await?;
    let abandoned_path = abandoned.path().to_owned();
    std::fs::write(&abandoned_path, b"partial")?;
    drop(abandoned);
    library.close().await?;
    let unknown = settings.data_dir.join("staging/keep.txt");
    std::fs::write(&unknown, b"unknown")?;
    let database = db(&settings.data_dir).await?;
    let mut local = LocalAsset::new(imported.sticker.id().content_hash());
    local.verified(TimestampMs::new(100));
    local.begin_thumbnail()?;
    let mut tx = database.begin_write().await?;
    tx.save_local_asset(&local).await?;
    tx.commit().await?;
    database.close().await?;
    let library = Library::open(settings.clone()).await?;
    assert!(!abandoned_path.exists());
    assert!(unknown.exists());
    library.close().await?;
    let database = db(&settings.data_dir).await?;
    assert_eq!(
        database
            .local_asset(imported.sticker.id().content_hash())
            .await?
            .ok_or("local asset")?
            .thumb_status(),
        ThumbnailStatus::Missing
    );
    database.close().await?;
    Ok(())
}

#[tokio::test]
async fn corrupted_cached_thumbnail_is_regenerated_without_mutating_original() -> TestResult {
    let directory = tempfile::tempdir()?;
    let library = Library::open(config(directory.path())).await?;
    let imported = import(&library, &png()?, "cat.png").await?;
    let thumbnail = library
        .request_thumbnail(imported.sticker.id(), Priority::Visible)?
        .wait()
        .await?;
    std::fs::write(&thumbnail.path, b"broken cache")?;
    let regenerated = library
        .request_thumbnail(imported.sticker.id(), Priority::Visible)?
        .wait()
        .await?;
    assert_eq!(image::open(regenerated.path)?.width(), 256);
    assert_eq!(library.space_statistics()?.wait().await?.known_assets, 1);
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_collection_rolls_back_business_records_after_file_publication() -> TestResult {
    let directory = tempfile::tempdir()?;
    let settings = config(directory.path());
    let library = Library::open(settings.clone()).await?;
    let input = library.create_import_input()?.wait().await?;
    std::fs::write(input.path(), png()?)?;
    let error = library
        .import_staged(
            input,
            ImportOptions {
                collection: Some(memedock_domain::identity::CollectionId::new()),
                ..Default::default()
            },
        )?
        .wait()
        .await
        .err()
        .ok_or("expected missing collection")?;
    assert_eq!(error.code(), ErrorCode::NotFound);
    assert!(query(&library, "").await?.stickers.is_empty());
    library.close().await?;
    let database = db(&settings.data_dir).await?;
    assert!(database.changes_after(None, 20).await?.is_empty());
    assert_eq!(database.space_statistics().await?.known_assets, 0);
    database.close().await?;
    Ok(())
}
