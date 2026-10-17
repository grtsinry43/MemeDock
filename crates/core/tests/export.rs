mod common;
use common::*;
use memedock_core::{ErrorCode, Library};
use memedock_domain::export::{AnimationPolicy, ExportOptions, ExportPreset};
#[path = "common/image_fixtures.rs"]
mod image_fixtures;

fn gif_options() -> ExportOptions {
    ExportOptions::for_preset(ExportPreset::CompatibleGif, AnimationPolicy::Preserve)
}

fn timed_gif(
    width: u32,
    height: u32,
    delays: &[u32],
    repeat: Option<image::codecs::gif::Repeat>,
) -> TestResult<Vec<u8>> {
    let mut bytes = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new_with_speed(&mut bytes, 10);
        if let Some(repeat) = repeat {
            encoder.set_repeat(repeat)?;
        }
        for (i, delay) in delays.iter().enumerate() {
            let pixels = image::RgbaImage::from_fn(width, height, |x, _| {
                if (x < width / 2) == i.is_multiple_of(2) {
                    image::Rgba([200, 10, 20, 255])
                } else {
                    image::Rgba([0; 4])
                }
            });
            encoder.encode_frame(image::Frame::from_parts(
                pixels,
                0,
                0,
                image::Delay::from_numer_denom_ms(*delay, 1),
            ))?;
        }
    }
    Ok(bytes)
}

fn gif_repetitions(bytes: &[u8]) -> Option<u16> {
    bytes
        .windows(11)
        .position(|window| window == b"NETSCAPE2.0")
        .map(|at| u16::from_le_bytes([bytes[at + 13], bytes[at + 14]]))
}

#[tokio::test]
async fn gif_resizes_preserves_disposal_timing_repetition_and_original_across_restart() -> TestResult
{
    use image::AnimationDecoder;
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let library = Library::open(settings.clone()).await?;
    let bytes = timed_gif(
        1024,
        512,
        &[40, 60],
        Some(image::codecs::gif::Repeat::Finite(3)),
    )?;
    let id = import_real(&library, &bytes).await?;
    let output = library.export(id, gif_options())?.wait().await?;
    let metadata = output.metadata().clone();
    assert_eq!(metadata.mime, "image/gif");
    assert_eq!(
        metadata.path.extension().and_then(|v| v.to_str()),
        Some("gif")
    );
    assert!(metadata.animated);
    let result = std::fs::read(&metadata.path)?;
    assert_eq!(gif_repetitions(&result), Some(3));
    let frames = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(result))?
        .into_frames()
        .collect_frames()?;
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].buffer().dimensions(), (512, 256));
    assert_eq!(
        frames
            .iter()
            .map(|f| f.delay().numer_denom_ms().0)
            .collect::<Vec<_>>(),
        [40, 60]
    );
    assert_eq!(frames[0].buffer().get_pixel(100, 100).0, [200, 10, 20, 255]);
    assert_eq!(frames[1].buffer().get_pixel(100, 100)[3], 0);
    assert_eq!(frames[1].buffer().get_pixel(400, 100).0, [200, 10, 20, 255]);
    assert_eq!(
        std::fs::read(&library.export_original(id)?.wait().await?.metadata().path)?,
        bytes
    );
    assert_eq!(
        library
            .export(id, gif_options())?
            .wait()
            .await?
            .metadata()
            .id,
        metadata.id
    );
    library.prepare_handoff(output)?.wait().await?;
    library.close().await?;
    let library = Library::open(settings).await?;
    assert_eq!(
        library
            .export(id, gif_options())?
            .wait()
            .await?
            .metadata()
            .id,
        metadata.id
    );
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn gif_converts_composed_webp_and_apng_without_upscaling() -> TestResult {
    use image::AnimationDecoder;
    let dir = tempfile::tempdir()?;
    let library = Library::open(config(dir.path())).await?;
    let mut finite_webp = image_fixtures::animated_webp()?;
    finite_webp[42..44].copy_from_slice(&4u16.to_le_bytes());
    for (bytes, repeats, outside_alpha) in [
        (image_fixtures::animated_webp()?, Some(0), 0),
        (finite_webp, Some(3), 0),
        (image_fixtures::apng()?, Some(0), 255),
        (image_fixtures::apng_with_plays(1)?, None, 255),
        (image_fixtures::apng_with_plays(5)?, Some(4), 255),
        (timed_gif(40, 20, &[40, 60], None)?, None, 0),
    ] {
        let id = import_real(&library, &bytes).await?;
        let output = library.export(id, gif_options())?.wait().await?;
        let encoded = std::fs::read(&output.metadata().path)?;
        assert_eq!(gif_repetitions(&encoded), repeats);
        let frames = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(encoded))?
            .into_frames()
            .collect_frames()?;
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].buffer().dimensions(), (40, 20));
        assert_eq!(frames[0].buffer().get_pixel(10, 10).0, [200, 10, 20, 255]);
        assert_eq!(frames[0].buffer().get_pixel(30, 15)[3], outside_alpha);
    }
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn gif_threshold_and_cache_metadata_are_verified() -> TestResult {
    use image::AnimationDecoder;
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let library = Library::open(settings.clone()).await?;
    let id = import_real(&library, &image_fixtures::animated_webp_with_alpha(127)?).await?;
    let output = library.export(id, gif_options())?.wait().await?;
    let frames = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
        std::fs::File::open(&output.metadata().path)?,
    ))?
    .into_frames()
    .collect_frames()?;
    assert_eq!(frames[0].buffer().get_pixel(10, 10)[3], 0);
    assert_eq!(frames[1].buffer().get_pixel(10, 10).0, [10, 200, 20, 255]);
    let old_id = output.metadata().id;
    let db =
        memedock_storage::LibraryDatabase::open(settings.data_dir.join("library.sqlite")).await?;
    let mut record = db.artifact_by_id(old_id).await?.ok_or("record")?;
    record.animated = false;
    db.save_artifact(&record).await?;
    assert_eq!(
        library
            .export(id, gif_options())?
            .wait()
            .await
            .err()
            .ok_or("metadata corruption")?
            .code(),
        ErrorCode::CorruptData
    );
    drop(output);
    let replacement = library.export(id, gif_options())?.wait().await?;
    assert_ne!(replacement.metadata().id, old_id);
    assert!(replacement.metadata().animated);
    db.close().await?;
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn gif_rejects_overlong_too_many_frames_zero_delays_and_cleans_staging() -> TestResult {
    let dir = tempfile::tempdir()?;
    let settings = config(dir.path());
    let library = Library::open(settings.clone()).await?;
    for (delays, code) in [
        (vec![15_000, 15_010], ErrorCode::ResourceLimit),
        (vec![10; 1801], ErrorCode::ResourceLimit),
        (vec![0, 10], ErrorCode::UnsupportedFormat),
    ] {
        let id = import_real(&library, &timed_gif(2, 1, &delays, None)?).await?;
        let error = library
            .export(id, gif_options())?
            .wait()
            .await
            .err()
            .ok_or("expected GIF limit")?;
        assert_eq!(error.code(), code);
        assert_eq!(std::fs::read_dir(&settings.export_dir)?.count(), 0);
        assert_eq!(
            std::fs::read_dir(settings.data_dir.join("staging"))?.count(),
            0
        );
    }
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn gif_quota_and_cancellation_never_publish_partial_files() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut settings = config(dir.path());
    settings.limits.export_budget_bytes = 32;
    let library = Library::open(settings.clone()).await?;
    let id = import_real(&library, &image_fixtures::animated_webp()?).await?;
    assert_eq!(
        library
            .export(id, gif_options())?
            .wait()
            .await
            .err()
            .ok_or("quota")?
            .code(),
        ErrorCode::ResourceLimit
    );
    let task = library.export(id, gif_options())?;
    task.cancel();
    assert_eq!(
        task.wait().await.err().ok_or("cancel")?.code(),
        ErrorCode::Cancelled
    );
    assert_eq!(std::fs::read_dir(&settings.export_dir)?.count(), 0);
    assert_eq!(
        std::fs::read_dir(settings.data_dir.join("staging"))?.count(),
        0
    );
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn best_compatibility_selects_formats_and_caps_frames_without_changing_duration() -> TestResult
{
    use image::AnimationDecoder;
    let dir = tempfile::tempdir()?;
    let library = Library::open(config(dir.path())).await?;
    let options =
        ExportOptions::for_preset(ExportPreset::BestCompatibility, AnimationPolicy::Preserve);
    let still = import_real(
        &library,
        &image_fixtures::encoded(image::ImageFormat::WebP, 1200, 600, [20, 80, 180, 128])?,
    )
    .await?;
    let output = library.export(still, options)?.wait().await?;
    assert_eq!(output.metadata().mime, "image/png");
    assert_eq!(image::open(&output.metadata().path)?.width(), 1024);
    // WebP retains millisecond timing; GIF fixtures quantize their input to 10 ms.
    let mut source = image_fixtures::animated_webp()?;
    // Two high-rate WebP frames, each 10 ms: output may collapse to one display frame.
    let mut at = 12;
    while at + 8 <= source.len() {
        let len = u32::from_le_bytes(source[at + 4..at + 8].try_into()?) as usize;
        if &source[at..at + 4] == b"ANMF" {
            source[at + 20..at + 23].copy_from_slice(&[10, 0, 0]);
        }
        at += 8 + len + (len % 2);
    }
    let short = import_real(&library, &source).await?;
    let output = library.export(short, options)?.wait().await?;
    assert_eq!(output.metadata().mime, "image/gif");
    assert!(!output.metadata().animated);
    assert_eq!(
        library.export(short, options)?.wait().await?.metadata().id,
        output.metadata().id
    );
    let frames = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
        std::fs::File::open(&output.metadata().path)?,
    ))?
    .into_frames()
    .collect_frames()?;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].delay().numer_denom_ms(), (20, 1));
    let animated = import_real(&library, &timed_gif(40, 20, &[10; 100], None)?).await?;
    let output = library.export(animated, options)?.wait().await?;
    let frames = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
        std::fs::File::open(&output.metadata().path)?,
    ))?
    .into_frames()
    .collect_frames()?;
    assert_eq!(frames.len(), 30);
    assert_eq!(
        frames
            .iter()
            .map(|f| f.delay().numer_denom_ms().0)
            .sum::<u32>(),
        1000
    );
    assert_eq!(frames[0].buffer().get_pixel(10, 10).0, [200, 10, 20, 255]);
    let first = library
        .export(
            animated,
            ExportOptions::for_preset(ExportPreset::BestCompatibility, AnimationPolicy::FirstFrame),
        )?
        .wait()
        .await?;
    assert_eq!(first.metadata().mime, "image/png");
    assert!(!first.metadata().animated);
    library.close().await?;
    Ok(())
}

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
