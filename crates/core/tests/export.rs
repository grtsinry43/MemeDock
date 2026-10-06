mod common;
use common::*;
use memedock_core::{ErrorCode, Library};
use memedock_domain::export::{AnimationPolicy, ExportOptions, ExportPreset};
#[path = "common/image_fixtures.rs"]
mod image_fixtures;

async fn import_real(
    library: &Library,
    bytes: &[u8],
) -> TestResult<memedock_domain::identity::StickerId> {
    let input = library.create_import_input()?.wait().await?;
    std::fs::write(input.path(), bytes)?;
    Ok(library
        .import_staged(input, memedock_core::ImportOptions::default())?
        .wait()
        .await?
        .sticker
        .id())
}

#[tokio::test]
async fn derived_presets_keep_alpha_compose_white_resize_and_isolate_cache() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let library = Library::open(settings.clone()).await?;
    let bytes = image_fixtures::encoded(image::ImageFormat::Png, 1200, 600, [200, 10, 20, 128])?;
    let id = import_real(&library, &bytes).await?;
    let mut artifact_ids = Vec::new();
    for (preset, size, pixel, mime) in [
        (
            ExportPreset::CompatiblePng,
            (1024, 512),
            [200, 10, 20, 128],
            "image/png",
        ),
        (
            ExportPreset::WhiteBackground,
            (1200, 600),
            [227, 132, 137, 255],
            "image/png",
        ),
        (
            ExportPreset::SmallJpeg,
            (512, 256),
            [227, 132, 137, 255],
            "image/jpeg",
        ),
    ] {
        let options = ExportOptions::for_preset(preset, AnimationPolicy::Preserve);
        let output = library.export(id, options)?.wait().await?;
        let display = image::open(&output.metadata().path)?.into_rgba8();
        assert_eq!(display.dimensions(), size);
        for (actual, expected) in display.get_pixel(100, 100).0.into_iter().zip(pixel) {
            assert!(
                actual.abs_diff(expected) <= 2,
                "{preset:?}: {actual} != {expected}"
            );
        }
        assert_eq!(output.metadata().mime, mime);
        assert!(!artifact_ids.contains(&output.metadata().id));
        artifact_ids.push(output.metadata().id);
        assert_eq!(
            library.export(id, options)?.wait().await?.metadata().id,
            output.metadata().id
        );
        library.prepare_handoff(output)?.wait().await?;
    }
    assert_eq!(
        std::fs::read(
            library
                .export_original(id)?
                .wait()
                .await?
                .metadata()
                .path
                .clone()
        )?,
        bytes
    );
    library.close().await?;
    let library = Library::open(settings).await?;
    assert_eq!(
        library
            .export(
                id,
                ExportOptions::for_preset(ExportPreset::CompatiblePng, AnimationPolicy::Preserve)
            )?
            .wait()
            .await?
            .metadata()
            .id,
        artifact_ids[0]
    );
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn animated_outputs_require_consent_and_use_complete_first_canvas() -> TestResult {
    let dir = tempfile::tempdir()?;
    let library = Library::open(config(dir.path())).await?;
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
    for bytes in [
        gif,
        image_fixtures::animated_webp()?,
        image_fixtures::apng()?,
    ] {
        let id = import_real(&library, &bytes).await?;
        assert_eq!(
            library
                .export(
                    id,
                    ExportOptions::for_preset(
                        ExportPreset::CompatiblePng,
                        AnimationPolicy::Preserve
                    )
                )?
                .wait()
                .await
                .err()
                .ok_or("expected consent rejection")?
                .code(),
            ErrorCode::InvalidInput
        );
        let output = library
            .export(
                id,
                ExportOptions::for_preset(ExportPreset::CompatiblePng, AnimationPolicy::FirstFrame),
            )?
            .wait()
            .await?;
        let display = image::open(&output.metadata().path)?.into_rgba8();
        assert_eq!(display.dimensions(), (40, 20));
        assert_eq!(display.get_pixel(10, 10)[0], 200);
        assert!(!output.metadata().animated);
        let original = library.export_original(id)?.wait().await?;
        assert!(original.metadata().animated);
        assert_eq!(std::fs::read(&original.metadata().path)?, bytes);
    }
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn tiny_inputs_are_not_upscaled_and_quota_failure_leaves_no_partial_output() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut settings = config(dir.path());
    settings.limits.export_budget_bytes = 64;
    let library = Library::open(settings.clone()).await?;
    let bytes = image_fixtures::encoded(image::ImageFormat::WebP, 8, 4, [20, 80, 180, 128])?;
    let id = import_real(&library, &bytes).await?;
    assert_eq!(
        library
            .export(
                id,
                ExportOptions::for_preset(ExportPreset::CompatiblePng, AnimationPolicy::Preserve)
            )?
            .wait()
            .await
            .err()
            .ok_or("expected quota rejection")?
            .code(),
        ErrorCode::ResourceLimit
    );
    assert_eq!(std::fs::read_dir(&settings.export_dir)?.count(), 0);
    assert_eq!(
        std::fs::read_dir(settings.data_dir.join("staging"))?.count(),
        0
    );
    library.close().await?;
    settings.limits.export_budget_bytes = 1024 * 1024;
    let library = Library::open(settings).await?;
    let output = library
        .export(
            id,
            ExportOptions::for_preset(ExportPreset::CompatiblePng, AnimationPolicy::Preserve),
        )?
        .wait()
        .await?;
    assert_eq!(
        image::open(&output.metadata().path)?
            .into_rgba8()
            .dimensions(),
        (8, 4)
    );
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn all_exif_orientations_are_applied_before_metadata_is_stripped() -> TestResult {
    use image::ImageEncoder;
    let dir = tempfile::tempdir()?;
    let library = Library::open(config(dir.path())).await?;
    for orientation in 1u8..=8 {
        let pixels = image::RgbImage::from_fn(40, 20, |x, y| {
            image::Rgb([
                if x < 20 { 200 } else { 20 },
                if y < 10 { 200 } else { 20 },
                50,
            ])
        });
        let mut jpeg = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 95);
        let mut exif =
            b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x01\0\0\0\0\0\0\0".to_vec();
        exif[18] = orientation;
        encoder.set_exif_metadata(exif)?;
        encoder.encode_image(&pixels)?;
        let mut expected = image::load_from_memory(&jpeg)?;
        expected.apply_orientation(
            image::metadata::Orientation::from_exif(orientation).ok_or("orientation")?,
        );
        let id = import_real(&library, &jpeg).await?;
        let output = library
            .export(
                id,
                ExportOptions::for_preset(ExportPreset::CompatiblePng, AnimationPolicy::Preserve),
            )?
            .wait()
            .await?;
        let displayed = image::open(&output.metadata().path)?;
        assert_eq!(displayed.into_rgba8(), expected.into_rgba8());
        assert!(
            !std::fs::read(&output.metadata().path)?
                .windows(4)
                .any(|v| v == b"eXIf")
        );
    }
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn embedded_icc_is_rejected_for_conversion_but_original_is_preserved() -> TestResult {
    use image::ImageEncoder;
    let dir = tempfile::tempdir()?;
    let library = Library::open(config(dir.path())).await?;
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85);
    encoder.set_icc_profile(vec![1, 2, 3, 4])?;
    encoder.encode_image(&image::RgbImage::from_pixel(
        40,
        20,
        image::Rgb([20, 80, 180]),
    ))?;
    let id = import_real(&library, &bytes).await?;
    assert_eq!(
        library
            .export(
                id,
                ExportOptions::for_preset(ExportPreset::CompatiblePng, AnimationPolicy::Preserve)
            )?
            .wait()
            .await
            .err()
            .ok_or("expected profile rejection")?
            .code(),
        ErrorCode::UnsupportedColorProfile
    );
    assert_eq!(
        std::fs::read(&library.export_original(id)?.wait().await?.metadata().path)?,
        bytes
    );
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn a_byte_valid_cache_with_wrong_preset_metadata_is_not_reused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let settings = config(directory.path());
    let library = Library::open(settings.clone()).await?;
    let id = import_real(
        &library,
        &image_fixtures::encoded(image::ImageFormat::Png, 40, 20, [20, 80, 180, 128])?,
    )
    .await?;
    let options = ExportOptions::for_preset(ExportPreset::CompatiblePng, AnimationPolicy::Preserve);
    let output = library.export(id, options)?.wait().await?;
    let old_id = output.metadata().id;
    let db =
        memedock_storage::LibraryDatabase::open(settings.data_dir.join("library.sqlite")).await?;
    let mut record = db.artifact_by_id(old_id).await?.ok_or("record")?;
    let store = memedock_storage::files::ExportStore::open(&settings.export_dir)?;
    let original_path = store.path(&record);
    record.format = memedock_domain::asset::ImageFormat::Jpeg;
    std::fs::rename(original_path, store.path(&record))?;
    db.save_artifact(&record).await?;
    store.verify(&record, || false)?;
    drop(output);
    let replacement = library.export(id, options)?.wait().await?;
    assert_ne!(replacement.metadata().id, old_id);
    assert_eq!(replacement.metadata().mime, "image/png");
    assert_eq!(image::open(&replacement.metadata().path)?.width(), 40);
    db.close().await?;
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn original_export_reuses_verified_bytes_and_survives_restart() -> TestResult {
    let dir = tempfile::tempdir()?;
    let configuration = config(dir.path());
    let bytes = b"original exact bytes";
    let (_, sticker) = seed(&configuration, bytes).await?;
    let library = Library::open(configuration.clone()).await?;
    let a = library.export_original(sticker.id())?.wait().await?;
    let b = library.export_original(sticker.id())?.wait().await?;
    assert_eq!(a.metadata().id, b.metadata().id);
    assert_eq!(std::fs::read(&a.metadata().path)?, bytes);
    let prepared = library.prepare_handoff(a.clone())?.wait().await?;
    assert!(prepared.retained_until >= a.metadata().retained_until);
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 0);
    let old = a.metadata().id;
    drop(a);
    drop(b);
    library.close().await?;
    let library = Library::open(configuration).await?;
    let a = library.export_original(sticker.id())?.wait().await?;
    assert_eq!(a.metadata().id, old);
    assert_eq!(std::fs::read(&a.metadata().path)?, bytes);
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn active_lease_prevents_expired_cleanup_and_corrupt_original_cannot_publish() -> TestResult {
    let dir = tempfile::tempdir()?;
    let configuration = config(dir.path());
    let (_, sticker) = seed(&configuration, b"protected").await?;
    let library = Library::open(configuration.clone()).await?;
    let lease = library.export_original(sticker.id())?.wait().await?;
    let path = lease.metadata().path.clone();
    let db = memedock_storage::LibraryDatabase::open(configuration.data_dir.join("library.sqlite"))
        .await?;
    let mut record = db
        .artifact_for(sticker.id().content_hash())
        .await?
        .ok_or("artifact")?;
    record.retained_until = 0;
    db.save_artifact(&record).await?;
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 0);
    assert!(path.is_file());
    drop(lease);
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 1);
    assert!(!path.exists());
    let blobs = memedock_storage::files::FsBlobStore::open(&configuration.data_dir)?;
    std::fs::write(
        blobs.original_path(sticker.id().content_hash()),
        b"corrupted",
    )?;
    assert_eq!(
        library
            .export_original(sticker.id())?
            .wait()
            .await
            .err()
            .ok_or("expected corruption")?
            .code(),
        ErrorCode::CorruptData
    );
    db.close().await?;
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn concurrent_exports_share_one_output_and_quota_never_evicts_retained_files() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut configuration = config(dir.path());
    configuration.limits.export_budget_bytes = 16;
    let (_, first) = seed(&configuration, b"first original").await?;
    let (_, second) = seed(&configuration, b"second original").await?;
    let library = Library::open(configuration).await?;
    let a = library.export_original(first.id())?;
    let b = library.export_original(first.id())?;
    let (a, b) = tokio::join!(a.wait(), b.wait());
    let a = a?;
    let b = b?;
    assert_eq!(a.metadata().id, b.metadata().id);
    assert_eq!(
        library
            .export_original(second.id())?
            .wait()
            .await
            .err()
            .ok_or("expected quota")?
            .code(),
        ErrorCode::ResourceLimit
    );
    assert!(a.metadata().path.is_file());
    drop(a);
    drop(b);
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn corrupt_output_is_replaced_with_new_uri_and_retired_file_keeps_retention() -> TestResult {
    let dir = tempfile::tempdir()?;
    let configuration = config(dir.path());
    let bytes = b"original exact bytes";
    let (_, sticker) = seed(&configuration, bytes).await?;
    let library = Library::open(configuration).await?;
    let a = library.export_original(sticker.id())?.wait().await?;
    let path = a.metadata().path.clone();
    let id = a.metadata().id;
    std::fs::write(&path, b"broken")?;
    assert_eq!(
        library
            .export_original(sticker.id())?
            .wait()
            .await
            .err()
            .ok_or("expected active corruption")?
            .code(),
        ErrorCode::CorruptData
    );
    drop(a);
    let b = library.export_original(sticker.id())?.wait().await?;
    assert_ne!(b.metadata().id, id);
    assert_eq!(std::fs::read(&b.metadata().path)?, bytes);
    assert_eq!(library.clean_export_artifacts()?.wait().await?, 0);
    assert!(path.is_file());
    library.close().await?;
    Ok(())
}
