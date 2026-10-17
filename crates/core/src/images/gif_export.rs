//! Bounded animation conversion. Native/image contexts never leave this worker.
use super::{export::BoundedWriter, inspect};
use crate::{CoreError, ErrorCode, ResourceLimits, Result, tasks::TaskControl};
use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::metadata::{LoopCount, Orientation};
use image::{AnimationDecoder, Delay, DynamicImage, Frame, ImageDecoder, ImageReader};
use memedock_storage::files::{FsBlobStore, StagedFile};
use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom, Write},
    time::{Duration, Instant},
};

const MAX_FRAMES: usize = 1800;
const MAX_DURATION_MS: f64 = 30_000.0;
const MAX_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TIME: Duration = Duration::from_secs(60);
// More than the accumulated f64 error of 1800 bounded intervals, less than
// a centisecond of playback. Keeps exact 30 s / half-tick boundaries stable.
const TIME_EPSILON_MS: f64 = 1e-6;

fn check(control: &TaskControl, started: Instant) -> Result<()> {
    control.check()?;
    if started.elapsed() >= MAX_TIME {
        return Err(CoreError::new(
            ErrorCode::Timeout,
            "GIF conversion timed out",
        ));
    }
    Ok(())
}

struct Animation {
    frames: image::Frames<'static>,
    repeat: Option<Repeat>,
    orientation: Orientation,
    dimensions: (u32, u32),
}

fn prepare(decoder: &mut impl ImageDecoder, limits: &ResourceLimits) -> Result<Orientation> {
    let (w, h) = inspect::bounds(decoder, limits)?;
    // Composition, orientation and Lanczos's f32 intermediate coexist.
    if u64::from(w) * u64::from(h) > limits.max_decode_bytes / 48 {
        return Err(CoreError::new(
            ErrorCode::ResourceLimit,
            "GIF working budget exceeded",
        ));
    }
    inspect::check_profile(decoder, true)?;
    decoder.orientation().map_err(inspect::error)
}

fn repeat(count: LoopCount) -> Result<Option<Repeat>> {
    match count {
        LoopCount::Infinite => Ok(Some(Repeat::Infinite)),
        // WebP/APNG count total plays; GIF counts repetitions after the first.
        LoopCount::Finite(n) if n.get() == 1 => Ok(None),
        LoopCount::Finite(n) => Ok(Some(Repeat::Finite(u16::try_from(n.get() - 1).map_err(
            |_| {
                CoreError::new(
                    ErrorCode::UnsupportedFormat,
                    "GIF loop count is unrepresentable",
                )
            },
        )?))),
    }
}

/// image's GIF loop_count maps an absent extension to infinite and cannot
/// distinguish a single play. Read only the header's extension blocks, leaving
/// image responsible for all pixel decoding and validation.
fn gif_repeat(file: &mut File, control: &TaskControl, started: Instant) -> Result<Option<Repeat>> {
    file.seek(SeekFrom::Start(0))?;
    let mut reader = BufReader::new(&mut *file);
    let mut header = [0; 13];
    reader.read_exact(&mut header)?;
    if &header[..6] != b"GIF87a" && &header[..6] != b"GIF89a" {
        return Err(CoreError::new(
            ErrorCode::InvalidImage,
            "invalid GIF header",
        ));
    }
    if header[10] & 0x80 != 0 {
        reader.seek(SeekFrom::Current(3 * (1i64 << ((header[10] & 7) + 1))))?;
    }
    let mut result = None;
    loop {
        check(control, started)?;
        let mut tag = [0];
        reader.read_exact(&mut tag)?;
        if matches!(tag[0], 0x2c | 0x3b) {
            break;
        }
        if tag[0] != 0x21 {
            return Err(CoreError::new(ErrorCode::InvalidImage, "invalid GIF block"));
        }
        let mut label = [0];
        reader.read_exact(&mut label)?;
        let mut application = false;
        let mut first = true;
        loop {
            check(control, started)?;
            reader.read_exact(&mut tag)?;
            let size = usize::from(tag[0]);
            if size == 0 {
                break;
            }
            let mut block = [0; 255];
            reader.read_exact(&mut block[..size])?;
            if first && label[0] == 0xff {
                application = &block[..size] == b"NETSCAPE2.0" || &block[..size] == b"ANIMEXTS1.0";
            } else if application && size == 3 && block[0] == 1 {
                let n = u16::from_le_bytes([block[1], block[2]]);
                result = Some(if n == 0 {
                    Repeat::Infinite
                } else {
                    Repeat::Finite(n)
                });
            }
            first = false;
        }
    }
    drop(reader);
    file.seek(SeekFrom::Start(0))?;
    Ok(result)
}

fn animation(
    mut file: File,
    limits: &ResourceLimits,
    control: &TaskControl,
    started: Instant,
) -> Result<Animation> {
    if file.metadata()?.len() > limits.max_file_bytes {
        return Err(CoreError::new(
            ErrorCode::ResourceLimit,
            "GIF input budget exceeded",
        ));
    }
    let format = ImageReader::new(BufReader::new(file.try_clone()?))
        .with_guessed_format()?
        .format();
    file.seek(SeekFrom::Start(0))?;
    let result = match format {
        Some(image::ImageFormat::Gif) => {
            let repeat = gif_repeat(&mut file, control, started)?;
            let mut decoder = GifDecoder::new(BufReader::new(file)).map_err(inspect::error)?;
            let orientation = prepare(&mut decoder, limits)?;
            Animation {
                dimensions: decoder.dimensions(),
                frames: decoder.into_frames(),
                repeat,
                orientation,
            }
        }
        Some(image::ImageFormat::WebP) => {
            let mut decoder = image::codecs::webp::WebPDecoder::new(BufReader::new(file))
                .map_err(inspect::error)?;
            let orientation = prepare(&mut decoder, limits)?;
            let repeat = repeat(decoder.loop_count())?;
            Animation {
                dimensions: decoder.dimensions(),
                frames: decoder.into_frames(),
                repeat,
                orientation,
            }
        }
        Some(image::ImageFormat::Png) => {
            let mut codec_limits = image::Limits::default();
            codec_limits.max_alloc = Some(limits.max_decode_bytes);
            let mut decoder =
                image::codecs::png::PngDecoder::with_limits(BufReader::new(file), codec_limits)
                    .map_err(inspect::error)?;
            let orientation = prepare(&mut decoder, limits)?;
            let dimensions = decoder.dimensions();
            let decoder = decoder.apng().map_err(inspect::error)?;
            let repeat = repeat(decoder.loop_count())?;
            Animation {
                dimensions,
                frames: decoder.into_frames(),
                repeat,
                orientation,
            }
        }
        _ => {
            return Err(CoreError::new(
                ErrorCode::UnsupportedFormat,
                "unsupported GIF source",
            ));
        }
    };
    check(control, started)?;
    Ok(result)
}

#[derive(Default)]
struct Timeline {
    milliseconds: f64,
    ticks: u32,
    frames: usize,
}
impl Timeline {
    fn advance(&mut self, delay: Delay) -> Result<()> {
        self.frames += 1;
        let (n, d) = delay.numer_denom_ms();
        if n == 0 {
            return Err(CoreError::new(
                ErrorCode::UnsupportedFormat,
                "GIF cannot represent zero frame timing",
            ));
        }
        self.milliseconds += f64::from(n) / f64::from(d);
        if self.frames > MAX_FRAMES || self.milliseconds > MAX_DURATION_MS + TIME_EPSILON_MS {
            return Err(CoreError::new(
                ErrorCode::ResourceLimit,
                "GIF animation budget exceeded",
            ));
        }
        Ok(())
    }
    fn next(&mut self, delay: Delay) -> Result<Delay> {
        self.advance(delay)?;
        // Round cumulative endpoints, not every frame independently. f64 error
        // across this bounded 30-second timeline is far below a centisecond.
        let end = ((self.milliseconds + TIME_EPSILON_MS) / 10.0).round() as u32;
        if end <= self.ticks {
            return Err(CoreError::new(
                ErrorCode::UnsupportedFormat,
                "GIF cannot represent source frame timing",
            ));
        }
        let delay = Delay::from_numer_denom_ms((end - self.ticks) * 10, 1);
        self.ticks = end;
        Ok(delay)
    }
}

struct TimedWriter<'a> {
    inner: BoundedWriter<'a>,
    started: Instant,
}
impl Write for TimedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        check(self.inner.control, self.started).map_err(io::Error::other)?;
        self.inner.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) fn encode(
    original: File,
    blobs: &FsBlobStore,
    limits: &ResourceLimits,
    available: u64,
    control: &TaskControl,
) -> Result<(StagedFile, bool)> {
    let started = Instant::now();
    check(control, started)?;
    control.stage("decoding_export");
    let mut animation = animation(original, limits, control, started)?;
    let pending = blobs.create_staging()?;
    let result = (|| {
        let mut writer = TimedWriter {
            inner: BoundedWriter {
                file: File::options().write(true).open(pending.path())?,
                remaining: available.min(MAX_BYTES),
                control,
                exceeded: false,
            },
            started,
        };
        let encoded = (|| -> Result<()> {
            let mut encoder = GifEncoder::new_with_speed(&mut writer, 10);
            if let Some(repeat) = animation.repeat {
                encoder.set_repeat(repeat).map_err(inspect::error)?;
            }
            let mut timeline = Timeline::default();
            let mut next_sample: f64 = 0.0;
            let mut pending_frame: Option<(image::RgbaImage, u32)> = None;
            loop {
                check(control, started)?;
                let Some(frame) = animation.frames.next() else {
                    break;
                };
                let frame = frame.map_err(inspect::error)?;
                check(control, started)?;
                if frame.buffer().dimensions() != animation.dimensions {
                    return Err(CoreError::new(
                        ErrorCode::InvalidImage,
                        "animation canvas changed",
                    ));
                }
                timeline.advance(frame.delay())?;
                // Keep the sampling grid stable when a point rounds to the
                // next GIF tick; the following source canvas can supply that
                // same tick without shifting every subsequent sample.
                let sample = next_sample;
                let tick = ((sample + TIME_EPSILON_MS) / 10.0).round() as u32;
                let end_tick = ((timeline.milliseconds + TIME_EPSILON_MS) / 10.0).round() as u32;
                if sample >= timeline.milliseconds - TIME_EPSILON_MS || end_tick <= tick {
                    continue;
                }
                // Decode every source frame for composition, but scale and
                // quantize only the canvases selected for <=30 FPS output.
                next_sample = (sample + 1000.0 / 30.0).max(timeline.milliseconds);
                let mut image = DynamicImage::ImageRgba8(frame.into_buffer());
                image.apply_orientation(animation.orientation);
                let mut rgba = super::export::resize(image.into_rgba8(), Some(512), control)?;
                for row in rgba.rows_mut() {
                    check(control, started)?;
                    for pixel in row {
                        if pixel[3] < 128 {
                            pixel.0 = [0; 4];
                        } else {
                            pixel[3] = 255;
                        }
                    }
                }
                control.stage("encoding_export");
                if let Some((previous, start)) = pending_frame.replace((rgba, tick)) {
                    encoder
                        .encode_frame(Frame::from_parts(
                            previous,
                            0,
                            0,
                            Delay::from_numer_denom_ms((tick - start) * 10, 1),
                        ))
                        .map_err(inspect::error)?;
                }
            }
            if timeline.frames == 0 {
                return Err(CoreError::new(
                    ErrorCode::InvalidImage,
                    "animation has no frames",
                ));
            }
            let (last, start) = pending_frame.ok_or_else(|| {
                CoreError::new(
                    ErrorCode::UnsupportedFormat,
                    "GIF animation is shorter than one tick",
                )
            })?;
            let end = ((timeline.milliseconds + TIME_EPSILON_MS) / 10.0).round() as u32;
            encoder
                .encode_frame(Frame::from_parts(
                    last,
                    0,
                    0,
                    Delay::from_numer_denom_ms((end - start) * 10, 1),
                ))
                .map_err(inspect::error)?;
            Ok(())
        })();
        // GifEncoder writes its trailer in Drop. Observe quota/cancellation
        // afterwards and verify the trailer instead of ignoring final errors.
        check(control, started)?;
        if writer.inner.exceeded {
            return Err(CoreError::new(
                ErrorCode::ResourceLimit,
                "GIF output quota exceeded",
            ));
        }
        encoded?;
        writer.flush()?;
        drop(writer);
        let animated = verify(File::open(pending.path())?, limits, control, started)?;
        check(control, started)?;
        Ok((
            pending.finish(available.min(MAX_BYTES), || control.is_cancelled())?,
            animated,
        ))
    })();
    if result.is_err() {
        pending.discard()?;
    }
    result
}

/// Full, bounded validation before publishing, also checks metadata on cache reuse.
pub(crate) fn verify(
    mut file: File,
    limits: &ResourceLimits,
    control: &TaskControl,
    started: Instant,
) -> Result<bool> {
    check(control, started)?;
    if file.metadata()?.len() > MAX_BYTES {
        return Err(CoreError::new(
            ErrorCode::ResourceLimit,
            "GIF output quota exceeded",
        ));
    }
    file.seek(SeekFrom::End(-1))?;
    let mut trailer = [0];
    file.read_exact(&mut trailer)?;
    if trailer[0] != 0x3b {
        return Err(CoreError::new(
            ErrorCode::CorruptData,
            "GIF trailer missing",
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut decoder = GifDecoder::new(BufReader::new(file)).map_err(inspect::error)?;
    let (w, h) = inspect::bounds(&mut decoder, limits)?;
    if w.max(h) > 512 {
        return Err(CoreError::new(
            ErrorCode::CorruptData,
            "GIF dimensions exceed preset",
        ));
    }
    let mut timeline = Timeline::default();
    let mut frames = decoder.into_frames();
    loop {
        check(control, started)?;
        let Some(frame) = frames.next() else { break };
        timeline.next(frame.map_err(inspect::error)?.delay())?;
    }
    if timeline.frames == 0 {
        return Err(CoreError::new(ErrorCode::CorruptData, "GIF has no frames"));
    }
    Ok(timeline.frames > 1)
}

/// Cached bytes were fully decoded before publication and their hash has just
/// been verified by ExportStore. Count container frames without LZW decoding.
pub(crate) fn verify_cached(
    file: File,
    limits: &ResourceLimits,
    control: &TaskControl,
    started: Instant,
) -> Result<bool> {
    let mut reader = BufReader::new(file);
    let mut header = [0; 13];
    reader.read_exact(&mut header)?;
    let width = u16::from_le_bytes([header[6], header[7]]);
    let height = u16::from_le_bytes([header[8], header[9]]);
    if (&header[..6] != b"GIF89a" && &header[..6] != b"GIF87a")
        || width == 0
        || height == 0
        || width.max(height) > 512
    {
        return Err(CoreError::new(
            ErrorCode::CorruptData,
            "invalid cached GIF header",
        ));
    }
    if u64::from(width) * u64::from(height) > limits.max_frame_pixels {
        return Err(CoreError::new(
            ErrorCode::ResourceLimit,
            "GIF frame exceeds pixel budget",
        ));
    }
    if header[10] & 0x80 != 0 {
        reader.seek(SeekFrom::Current(3 * (1i64 << ((header[10] & 7) + 1))))?;
    }
    let mut frames = 0;
    loop {
        check(control, started)?;
        let mut tag = [0];
        reader.read_exact(&mut tag)?;
        match tag[0] {
            0x3b if frames > 0 => return Ok(frames > 1),
            0x21 => {
                reader.read_exact(&mut tag)?;
            }
            0x2c => {
                frames += 1;
                if frames > MAX_FRAMES {
                    return Err(CoreError::new(
                        ErrorCode::CorruptData,
                        "cached GIF frame limit",
                    ));
                }
                let mut descriptor = [0; 9];
                reader.read_exact(&mut descriptor)?;
                if descriptor[8] & 0x80 != 0 {
                    reader.seek(SeekFrom::Current(3 * (1i64 << ((descriptor[8] & 7) + 1))))?;
                }
                reader.read_exact(&mut tag)?; // LZW minimum code size.
            }
            _ => {
                return Err(CoreError::new(
                    ErrorCode::CorruptData,
                    "invalid cached GIF block",
                ));
            }
        }
        loop {
            check(control, started)?;
            reader.read_exact(&mut tag)?;
            if tag[0] == 0 {
                break;
            }
            let mut data = [0; 255];
            reader.read_exact(&mut data[..usize::from(tag[0])])?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    #[ignore = "manual performance measurement; generates a large full-color animation"]
    fn benchmark_large_webp() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let dir = tempfile::tempdir()?;
        let source = tempfile::NamedTempFile::new()?;
        let control = TaskControl::new(Arc::new(tokio::sync::Notify::new()));
        let mut sink =
            super::super::animated_webp::Sink::new(768, 512, &control, source.path(), MAX_BYTES)?;
        for i in 0..48 {
            let pixels = image::RgbaImage::from_fn(768, 512, |x, y| {
                image::Rgba([
                    ((x + i * 5) % 256) as u8,
                    ((y + i * 3) % 256) as u8,
                    ((x / 3 + y / 2 + i * 7) % 256) as u8,
                    if x < 20 { 0 } else { 255 },
                ])
            });
            sink.add(
                pixels.as_raw(),
                (i * 1000 / 60) as i32,
                ((i + 1) * 1000 / 60) as i32,
            )?;
        }
        sink.finish()?;
        let limits = ResourceLimits::default();
        let blobs = FsBlobStore::open(dir.path())?;
        let started = Instant::now();
        let (staged, _) = encode(
            File::open(source.path())?,
            &blobs,
            &limits,
            MAX_BYTES,
            &control,
        )?;
        eprintln!(
            "768x512 / 48 frames / 60 FPS: conversion={:?}, bytes={}",
            started.elapsed(),
            staged.byte_size()
        );
        let started = Instant::now();
        verify_cached(staged.open_read()?, &limits, &control, started)?;
        eprintln!("cache validation={:?}", started.elapsed());
        staged.discard()?;
        Ok(())
    }

    #[test]
    fn fractional_delays_round_cumulative_endpoints_without_drift() -> Result<()> {
        let mut timeline = Timeline::default();
        let mut total = 0;
        for _ in 0..MAX_FRAMES {
            let (n, d) = timeline
                .next(Delay::from_numer_denom_ms(1000, 60))?
                .numer_denom_ms();
            assert_eq!(d, 1);
            assert!(matches!(n, 10 | 20));
            total += n;
        }
        assert_eq!(total, 30_000);
        Ok(())
    }

    #[test]
    fn timeout_and_cancellation_are_checked_during_output_writes()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let control = TaskControl::new(Arc::new(tokio::sync::Notify::new()));
        let file = tempfile::tempfile()?;
        let started = Instant::now().checked_sub(MAX_TIME).ok_or("clock")?;
        let mut writer = TimedWriter {
            inner: BoundedWriter {
                file,
                remaining: 100,
                control: &control,
                exceeded: false,
            },
            started,
        };
        assert_eq!(
            check(&control, started).err().ok_or("timeout")?.code(),
            ErrorCode::Timeout
        );
        assert!(writer.write_all(b"GIF89a").is_err());
        writer.started = Instant::now();
        control.cancel();
        assert!(writer.write_all(b"GIF89a").is_err());
        assert_eq!(writer.inner.file.metadata()?.len(), 0);
        assert_eq!(
            check(&control, writer.started)
                .err()
                .ok_or("cancel")?
                .code(),
            ErrorCode::Cancelled
        );
        Ok(())
    }
}
