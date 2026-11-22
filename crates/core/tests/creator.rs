#[allow(dead_code)]
mod common;
use common::{TestResult, config};
use image::{AnimationDecoder, ImageDecoder};
use memedock_core::{
    ErrorCode, ImportOptions, Library,
    creator::{VideoCrop, VideoEdit},
};
use memedock_domain::export::{AnimationPolicy, ExportOptions, ExportPreset, GifSizeLimit};
use std::io::BufReader;

fn fixture(hex: &str) -> TestResult<Vec<u8>> {
    let compact: String = hex.split_whitespace().collect();
    (0..compact.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&compact[i..i + 2], 16).map_err(Into::into))
        .collect()
}
fn animation(path: &std::path::Path) -> TestResult<(usize, u64, (u32, u32))> {
    let decoder =
        image::codecs::webp::WebPDecoder::new(BufReader::new(std::fs::File::open(path)?))?;
    let dimensions = decoder.dimensions();
    let mut count = 0;
    let mut milliseconds = 0;
    for frame in decoder.into_frames() {
        let frame = frame?;
        let (n, d) = frame.delay().numer_denom_ms();
        milliseconds += u64::from(n) / u64::from(d);
        count += 1;
    }
    Ok((count, milliseconds, dimensions))
}
#[tokio::test]
async fn h264_trim_crop_webp_gif_import_and_cache_reuse() -> TestResult {
    let dir = tempfile::tempdir()?;
    let cfg = config(dir.path());
    let path = dir.path().join("source.mp4");
    std::fs::write(
        &path,
        fixture(include_str!("fixtures/creator-h264.mp4.hex"))?,
    )?;
    let library = Library::open(cfg).await?;
    let source = library.open_creator_video(path.clone())?.wait().await?;
    assert_eq!((source.info().width, source.info().height), (64, 48));
    assert_eq!(source.info().duration_ms, 3000);
    let source_path = source.path().to_owned();
    std::fs::remove_file(path)?; // Editing reads its immutable snapshot.
    let preview = library.creator_preview(source.clone(), 750)?.wait().await?;
    assert_eq!(preview.metadata().mime, "image/png");
    let edit = VideoEdit::new(500, 2550, VideoCrop::new(0.25, 0.0, 0.5, 1.0)?, 512, 15, 80)?;
    let output = library
        .create_video_webp(source.clone(), edit)?
        .wait()
        .await?;
    assert_eq!((output.width, output.height), (32, 48));
    let (frames, duration, dimensions) = animation(&output.artifact.metadata().path)?;
    assert!(frames > 1);
    assert_eq!(duration, 2050);
    assert_eq!(dimensions, (32, 48));
    let gif = library
        .creation_gif(output.artifact.clone(), None)?
        .wait()
        .await?;
    assert_eq!(gif.metadata().mime, "image/gif");
    let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(std::fs::File::open(
        &gif.metadata().path,
    )?))?;
    let mut gif_duration = 0;
    for f in decoder.into_frames() {
        let (n, d) = f?.delay().numer_denom_ms();
        gif_duration += n / d;
    }
    assert_eq!(gif_duration, 2050);
    let outcome = library
        .import_file(
            output.artifact.metadata().path.clone(),
            ImportOptions {
                original_name: "Created.webp".into(),
                ..Default::default()
            },
        )?
        .wait()
        .await?;
    let exported = library
        .export(
            outcome.sticker.id(),
            ExportOptions::for_preset(ExportPreset::BestCompatibility, AnimationPolicy::Preserve),
        )?
        .wait()
        .await?;
    assert_eq!(exported.metadata().id, gif.metadata().id);
    let small = library
        .creation_gif(output.artifact.clone(), Some(GifSizeLimit::new(64 * 1024)?))?
        .wait()
        .await?;
    assert!(small.metadata().byte_size <= 64 * 1024);
    library.discard_creator_video(source)?.wait().await?;
    assert!(!source_path.exists());
    library.close().await?;
    Ok(())
}
#[tokio::test]
async fn hevc_decodes_and_preserves_selected_duration() -> TestResult {
    let dir = tempfile::tempdir()?;
    let library = Library::open(config(dir.path())).await?;
    let path = dir.path().join("phone.mov");
    std::fs::write(
        &path,
        fixture(include_str!("fixtures/creator-hevc.mp4.hex"))?,
    )?;
    let source = library.open_creator_video(path)?.wait().await?;
    let output = library
        .create_video_webp(
            source.clone(),
            VideoEdit::new(100, 1850, Default::default(), 128, 24, 75)?,
        )?
        .wait()
        .await?;
    assert_eq!(animation(&output.artifact.metadata().path)?.1, 1750);
    library.discard_creator_video(source)?.wait().await?;
    library.close().await?;
    Ok(())
}
#[tokio::test]
async fn invalid_input_and_cancellation_leave_no_published_results() -> TestResult {
    let dir = tempfile::tempdir()?;
    let cfg = config(dir.path());
    let library = Library::open(cfg.clone()).await?;
    let path = dir.path().join("bad.mp4");
    std::fs::write(&path, b"not a video")?;
    assert!(library.open_creator_video(path)?.wait().await.is_err());
    let path = dir.path().join("video.mp4");
    std::fs::write(
        &path,
        fixture(include_str!("fixtures/creator-h264.mp4.hex"))?,
    )?;
    let source = library.open_creator_video(path)?.wait().await?;
    assert!(
        library
            .create_video_webp(
                source.clone(),
                VideoEdit::new(2500, 3500, Default::default(), 128, 15, 80)?
            )
            .is_err()
    );
    let task = library.create_video_webp(
        source.clone(),
        VideoEdit::new(0, 3000, Default::default(), 128, 30, 90)?,
    )?;
    task.cancel();
    let error = task.wait().await.err().ok_or("expected cancellation")?;
    assert_eq!(error.code(), ErrorCode::Cancelled);
    assert_eq!(std::fs::read_dir(&cfg.export_dir)?.count(), 0);
    library.discard_creator_video(source)?.wait().await?;
    assert_eq!(std::fs::read_dir(cfg.data_dir.join("staging"))?.count(), 0);
    library.close().await?;
    Ok(())
}
#[test]
fn rejects_invalid_edit_parameters() -> TestResult {
    assert!(VideoCrop::new(f64::NAN, 0.0, 1.0, 1.0).is_err());
    assert!(VideoCrop::new(0.5, 0.0, 0.6, 1.0).is_err());
    assert!(VideoEdit::new(1, 1, Default::default(), 512, 30, 80).is_err());
    assert!(VideoEdit::new(0, 30_001, Default::default(), 512, 30, 80).is_err());
    assert!(VideoEdit::new(0, 1000, Default::default(), 512, 31, 80).is_err());
    assert!(GifSizeLimit::new(100).is_err());
    Ok(())
}

#[tokio::test]
async fn rotation_matches_display_coordinates_and_preview_pixels() -> TestResult {
    let dir = tempfile::tempdir()?;
    let library = Library::open(config(dir.path())).await?;
    let original = dir.path().join("original.mp4");
    let rotated = dir.path().join("rotated.mp4");
    std::fs::write(
        &original,
        fixture(include_str!("fixtures/creator-h264.mp4.hex"))?,
    )?;
    std::fs::write(
        &rotated,
        fixture(include_str!("fixtures/creator-rotated.mp4.hex"))?,
    )?;
    let original = library.open_creator_video(original)?.wait().await?;
    let rotated = library.open_creator_video(rotated)?.wait().await?;
    assert_eq!((rotated.info().width, rotated.info().height), (48, 64));
    let a = library
        .creator_preview(original.clone(), 500)?
        .wait()
        .await?;
    let b = library
        .creator_preview(rotated.clone(), 500)?
        .wait()
        .await?;
    let a = image::open(&a.metadata().path)?.into_rgba8();
    let b = image::open(&b.metadata().path)?.into_rgba8();
    let expected = image::imageops::rotate270(&a);
    assert_eq!(b.dimensions(), expected.dimensions());
    let error: u64 = b
        .as_raw()
        .iter()
        .zip(expected.as_raw())
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    assert!(
        error / (b.as_raw().len() as u64) < 8,
        "rotation mismatch: {error}"
    );
    library.discard_creator_video(original)?.wait().await?;
    library.discard_creator_video(rotated)?.wait().await?;
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn vfr_and_identical_frames_keep_duration_and_repeated_outputs_reuse_cache() -> TestResult {
    let dir = tempfile::tempdir()?;
    let library = Library::open(config(dir.path())).await?;
    for (name, hex, end) in [
        ("vfr", include_str!("fixtures/creator-vfr.mp4.hex"), 2400),
        (
            "still",
            include_str!("fixtures/creator-still.mp4.hex"),
            3900,
        ),
    ] {
        let path = dir.path().join(format!("{name}.mp4"));
        std::fs::write(&path, fixture(hex)?)?;
        let source = library.open_creator_video(path)?.wait().await?;
        let edit = VideoEdit::new(50, end, Default::default(), 128, 30, 80)?;
        let first = library
            .create_video_webp(source.clone(), edit)?
            .wait()
            .await?;
        let second = library
            .create_video_webp(source.clone(), edit)?
            .wait()
            .await?;
        assert_eq!(first.artifact.metadata().id, second.artifact.metadata().id);
        let (frames, duration, _) = animation(&first.artifact.metadata().path)?;
        assert!(frames > 0);
        assert_eq!(duration, end - 50);
        library.discard_creator_video(source)?.wait().await?;
    }
    library.close().await?;
    Ok(())
}

#[tokio::test]
async fn gif_byte_target_reduces_output_without_shortening_animation() -> TestResult {
    let dir = tempfile::tempdir()?;
    let library = Library::open(config(dir.path())).await?;
    let mut encoder = webp_animation::Encoder::new((256, 256))?;
    let mut seed = 0x12345678u32;
    for frame in 0..9 {
        let mut pixels = vec![0u8; 256 * 256 * 4];
        for pixel in pixels.chunks_exact_mut(4) {
            for value in &mut pixel[..3] {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                *value = seed as u8;
            }
            pixel[3] = 255;
        }
        encoder.add_frame(&pixels, frame * 1000 / 30)?;
    }
    let bytes = encoder.finalize(300)?;
    let path = dir.path().join("noise.webp");
    std::fs::write(&path, &bytes)?;
    let imported = library
        .import_file(path, ImportOptions::default())?
        .wait()
        .await?;
    let source = library
        .export_original(imported.sticker.id())?
        .wait()
        .await?;
    let original = library.creation_gif(source.clone(), None)?.wait().await?;
    assert!(original.metadata().byte_size > 128 * 1024);
    let limited = library
        .creation_gif(source, Some(GifSizeLimit::new(128 * 1024)?))?
        .wait()
        .await?;
    assert!(limited.metadata().byte_size <= 128 * 1024);
    let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(std::fs::File::open(
        &limited.metadata().path,
    )?))?;
    assert!(decoder.dimensions().0 < 256);
    let mut duration = 0;
    for frame in decoder.into_frames() {
        let (n, d) = frame?.delay().numer_denom_ms();
        duration += n / d;
    }
    assert_eq!(duration, 300);
    library.close().await?;
    Ok(())
}
