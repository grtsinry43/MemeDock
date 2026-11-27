//! Stream full replacement canvases, retaining only one encoded frame. Native
//! WebP supplies lossy pixels; the existing bounded sink supplies the container.
use super::{
    animated_webp::Sink,
    video::{self, Reader},
};
use crate::{CoreError, ErrorCode, ResourceLimits, Result, creator::VideoEdit, tasks::TaskControl};
use memedock_storage::files::{FsBlobStore, StagedFile};
use std::{sync::Arc, time::Instant};

pub(crate) struct EncodedVideo {
    pub staged: StagedFile,
    pub width: u32,
    pub height: u32,
}
fn encoding(error: Box<dyn std::error::Error + Send + Sync>) -> CoreError {
    match error.downcast::<CoreError>() {
        Ok(error) => *error,
        Err(error) => CoreError::caused(
            ErrorCode::InvalidImage,
            "WebP encoding failed",
            std::io::Error::other(error.to_string()),
        ),
    }
}
pub(crate) fn encode(
    source: &StagedFile,
    edit: VideoEdit,
    blobs: &FsBlobStore,
    limits: &ResourceLimits,
    available: u64,
    control: &Arc<TaskControl>,
) -> Result<EncodedVideo> {
    let started = Instant::now();
    let mut reader = Reader::open(source, limits, control, started)?;
    let pending = blobs.create_staging()?;
    let result = (|| {
        let mut sink = None;
        let mut dimensions = (0, 0);
        control.stage("encoding_webp");
        reader.render(edit, control, started, |rgba, w, h, start, end| {
            video::check(control, started)?;
            if sink.is_none() {
                sink = Some(
                    Sink::for_video(
                        w,
                        h,
                        control,
                        pending.path(),
                        available.min(limits.max_file_bytes),
                        edit.quality(),
                    )
                    .map_err(encoding)?,
                );
                dimensions = (w, h);
            }
            sink.as_mut()
                .ok_or_else(|| CoreError::internal("WebP sink missing"))?
                .add(rgba, start as i32, end as i32)
                .map_err(encoding)?;
            video::check(control, started)
        })?;
        sink.ok_or_else(|| {
            CoreError::new(ErrorCode::InvalidImage, "video selection has no frames")
        })?
        .finish()
        .map_err(encoding)?;
        video::check(control, started)?;
        control.stage("validating_creation");
        let checked =
            super::inspect::decode(std::fs::File::open(pending.path())?, limits, control, true)?;
        if checked.format != memedock_domain::asset::ImageFormat::WebP || !checked.animated {
            return Err(CoreError::new(
                ErrorCode::CorruptData,
                "creation format mismatch",
            ));
        }
        video::check(control, started)?;
        Ok(EncodedVideo {
            staged: pending.finish(available.min(limits.max_file_bytes), || {
                control.is_cancelled()
            })?,
            width: dimensions.0,
            height: dimensions.1,
        })
    })();
    if result.is_err() {
        pending.discard()?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::AnimationDecoder;
    #[test]
    fn transparent_replacement_frames_do_not_leave_previous_canvases()
    -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let file = tempfile::NamedTempFile::new()?;
        let control = TaskControl::new(Arc::new(tokio::sync::Notify::new()));
        let mut sink = Sink::for_video(32, 32, &control, file.path(), 1024 * 1024, 100)?;
        let mut expected = Vec::new();
        for frame in 0..32 {
            let x = frame % 24;
            let pixels = image::RgbaImage::from_fn(32, 32, |px, py| {
                if px >= x && px < x + 8 && (8..16).contains(&py) {
                    image::Rgba([255, 0, 0, 255])
                } else {
                    image::Rgba([0, 0, 0, 0])
                }
            });
            sink.add(pixels.as_raw(), frame as i32 * 50, (frame as i32 + 1) * 50)?;
            expected.push(pixels);
        }
        sink.finish()?;
        let decoder = image::codecs::webp::WebPDecoder::new(std::io::BufReader::new(
            std::fs::File::open(file.path())?,
        ))?;
        let frames = decoder.into_frames().collect_frames()?;
        assert_eq!(frames.len(), expected.len());
        for (index, (frame, expected)) in frames.iter().zip(&expected).enumerate() {
            for (pixel, expected) in frame.buffer().pixels().zip(expected.pixels()) {
                assert_eq!(pixel[3], expected[3], "alpha mismatch at frame {index}");
                if expected[3] > 0 {
                    assert!(
                        pixel[0] > 240 && pixel[1] < 15 && pixel[2] < 15,
                        "visible pixel mismatch at frame {index}"
                    );
                }
            }
        }
        Ok(())
    }
}
