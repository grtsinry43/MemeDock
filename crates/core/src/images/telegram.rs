//! Telegram-only conversion. Native contexts stay entirely on one blocking worker.
use super::animated_webp::Sink;
use crate::sources::telegram::TelegramFormat;
use crate::{CoreError, ErrorCode, Result, tasks::TaskControl};
use ffmpeg_next::{self as ff, Rescale};
use flate2::read::GzDecoder;
use std::{fs::File, io::Read, path::Path};
pub(super) type MediaResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const MAX_BYTES: u64 = 32 * 1024 * 1024;
const MAX_JSON: u64 = 4 * 1024 * 1024;
const MAX_FRAMES: usize = 180;
const MAX_DURATION: i32 = 3000;

pub(crate) fn convert(
    format: TelegramFormat,
    input: &Path,
    output: &Path,
    control: &TaskControl,
    max_output_bytes: u64,
) -> Result<()> {
    control.check()?;
    let result = match format {
        TelegramFormat::Tgs => tgs(input, output, control, max_output_bytes),
        TelegramFormat::Webm => webm(input, output, control, max_output_bytes),
        TelegramFormat::Static => {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "static media does not need conversion",
            ));
        }
    };
    // Cancellation must retain its stable code, including between native calls.
    control.check()?;
    result.map_err(|error| match error.downcast::<CoreError>() {
        Ok(error) => *error,
        Err(error) => CoreError::caused(
            ErrorCode::InvalidImage,
            "telegram media conversion failed",
            std::io::Error::other(error.to_string()),
        ),
    })
}
fn dimensions(width: u32, height: u32) -> MediaResult<()> {
    if width == 0 || height == 0 || width > 512 || height > 512 {
        return Err("invalid sticker dimensions".into());
    }
    Ok(())
}
fn tgs(
    input: &Path,
    output: &Path,
    control: &TaskControl,
    max_output_bytes: u64,
) -> MediaResult<()> {
    control.check()?;
    if input.metadata()?.len() > MAX_BYTES {
        return Err("input budget exceeded".into());
    }
    let mut json = Vec::new();
    GzDecoder::new(File::open(input)?)
        .take(MAX_JSON + 1)
        .read_to_end(&mut json)?;
    if json.len() as u64 > MAX_JSON || json.contains(&0) {
        return Err("invalid or oversized JSON".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&json)?;
    let width = u32::try_from(value["w"].as_u64().ok_or("missing width")?)?;
    let height = u32::try_from(value["h"].as_u64().ok_or("missing height")?)?;
    dimensions(width, height)?;
    if value["assets"]
        .as_array()
        .is_some_and(|a| a.iter().any(|v| v.get("p").is_some()))
    {
        return Err("external resources forbidden".into());
    }
    let mut animation = rlottie::Animation::from_data(json, input.to_string_lossy().as_bytes(), "")
        .ok_or("rlottie rejected input")?;
    let rate = animation.framerate();
    let total = animation.totalframe();
    if !rate.is_finite() || rate <= 0.0 || rate > 60.0 || total == 0 || total > MAX_FRAMES {
        return Err("invalid animation rate or frame budget exceeded".into());
    }
    if total as f64 * 1000.0 / rate > f64::from(MAX_DURATION) {
        return Err("animation duration exceeded".into());
    }
    if animation.size() != rlottie::Size::new(width as usize, height as usize) {
        return Err("animation dimensions mismatch".into());
    }
    let mut surface = rlottie::Surface::new(animation.size());
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    let mut sink = Sink::new(
        width,
        height,
        control,
        output,
        max_output_bytes.min(MAX_BYTES),
    )?;
    for i in 0..total {
        sink.check()?;
        animation.render(i, &mut surface);
        for (dst, bgra) in rgba.chunks_exact_mut(4).zip(surface.data()) {
            let a = u32::from(bgra.a);
            let straight = |c: u8| {
                if a == 0 {
                    0
                } else {
                    ((u32::from(c) * 255 + a / 2) / a).min(255) as u8
                }
            };
            dst.copy_from_slice(&[straight(bgra.r), straight(bgra.g), straight(bgra.b), bgra.a]);
        }
        sink.add(
            &rgba,
            (i as f64 * 1000.0 / rate).round() as i32,
            ((i + 1) as f64 * 1000.0 / rate).round() as i32,
        )?;
    }
    sink.finish()
}
fn webm(
    input: &Path,
    output: &Path,
    control: &TaskControl,
    max_output_bytes: u64,
) -> MediaResult<()> {
    control.check()?;
    if input.metadata()?.len() > MAX_BYTES {
        return Err("input budget exceeded".into());
    }
    ff::init()?;
    let mut context = ff::format::input(input)?;
    let stream = context
        .streams()
        .best(ff::media::Type::Video)
        .ok_or("video stream missing")?;
    let index = stream.index();
    let base = stream.time_base();
    let rate = stream.avg_frame_rate();
    let name = match stream.parameters().id() {
        ff::codec::Id::VP9 => "libvpx-vp9",
        ff::codec::Id::VP8 => "libvpx",
        _ => return Err("unsupported video codec".into()),
    };
    let codec = ff::decoder::find_by_name(name).ok_or("libvpx decoder missing")?;
    let decoder_context = ff::codec::context::Context::from_parameters(stream.parameters())?;
    let mut decoder = decoder_context.decoder().open_as(codec)?.video()?;
    let (width, height) = (decoder.width(), decoder.height());
    let mut filter = fps_filter(width, height, base, rate)?;
    let fallback = 1i64.rescale((rate.denominator(), rate.numerator()), base);
    if fallback <= 0 {
        return Err("invalid video frame duration".into());
    }
    let mut sink = Sink::new(
        width,
        height,
        control,
        output,
        max_output_bytes.min(MAX_BYTES),
    )?;
    let mut scaler = None;
    let mut first_timestamp = None;
    let mut last_timestamp = None;
    let mut decoded_frames = 0;
    let mut end_timestamp = 0;
    let mut drain = |decoder: &mut ff::decoder::Video| -> MediaResult<()> {
        let mut decoded = ff::util::frame::Video::empty();
        loop {
            sink.check()?;
            match decoder.receive_frame(&mut decoded) {
                Ok(()) => {}
                Err(ff::Error::Eof)
                | Err(ff::Error::Other {
                    errno: ff::error::EAGAIN,
                }) => break,
                Err(e) => return Err(e.into()),
            }
            // Bound input work even when the fps filter drops repeated timestamps.
            if decoded_frames >= MAX_FRAMES {
                return Err("frame budget exceeded".into());
            }
            decoded_frames += 1;
            if decoded.is_corrupt() || decoded.has_decode_errors() {
                return Err("corrupt decoded frame".into());
            }
            if decoded.width() != width || decoded.height() != height {
                return Err("dimensions changed".into());
            }
            let timestamp = decoded.timestamp().ok_or("frame timestamp missing")?;
            if last_timestamp.is_some_and(|last| timestamp < last) {
                return Err("video timestamp moved backwards".into());
            }
            last_timestamp = Some(timestamp);
            let start = timestamp
                .checked_sub(*first_timestamp.get_or_insert(timestamp))
                .ok_or("timestamp overflow")?;
            let duration = decoded.packet().duration;
            end_timestamp = start
                .checked_add(if duration > 0 { duration } else { fallback })
                .ok_or("duration overflow")?;
            if start < 0 || end_timestamp.rescale(base, (1, 1000)) > i64::from(MAX_DURATION) {
                return Err("animation duration exceeded".into());
            }
            if scaler.is_none() {
                scaler = Some(ff::software::scaling::context::Context::get(
                    decoded.format(),
                    width,
                    height,
                    ff::format::Pixel::RGBA,
                    width,
                    height,
                    ff::software::scaling::flag::Flags::BILINEAR,
                )?);
            }
            let mut frame = ff::util::frame::Video::empty();
            scaler
                .as_mut()
                .ok_or("scaler missing")?
                .run(&decoded, &mut frame)?;
            frame.set_pts(Some(start));
            filter
                .get("in")
                .ok_or("filter input missing")?
                .source()
                .add(&frame)?;
            drain_fps(&mut filter, &mut sink, width, height)?;
        }
        Ok(())
    };
    let mut packet = ff::Packet::empty();
    loop {
        control.check()?;
        match packet.read(&mut context) {
            Ok(()) if packet.stream() == index => {
                decoder.send_packet(&packet)?;
                drain(&mut decoder)?;
            }
            Ok(()) => {}
            Err(ff::Error::Eof) => break,
            Err(e) => return Err(e.into()),
        }
    }
    decoder.send_eof()?;
    drain(&mut decoder)?;
    // Explicit EOF time lets fps round the final interval in its output time base.
    filter
        .get("in")
        .ok_or("filter input missing")?
        .source()
        .close(end_timestamp)?;
    drain_fps(&mut filter, &mut sink, width, height)?;
    sink.finish()
}

fn fps_filter(
    width: u32,
    height: u32,
    base: ff::Rational,
    rate: ff::Rational,
) -> MediaResult<ff::filter::Graph> {
    dimensions(width, height)?;
    if base.numerator() <= 0 || base.denominator() <= 0 {
        return Err("invalid video time base".into());
    }
    if rate.numerator() <= 0 || rate.denominator() <= 0 {
        return Err("video frame rate missing".into());
    }
    let frame_ms = 1i64.rescale((rate.denominator(), rate.numerator()), (1, 1000));
    if !(1..=1000).contains(&frame_ms) {
        return Err("invalid video frame rate".into());
    }
    let mut graph = ff::filter::Graph::new();
    graph.add(
        &ff::filter::find("buffer").ok_or("buffer filter missing")?,
        "in",
        &format!("video_size={width}x{height}:pix_fmt=rgba:time_base={base}:pixel_aspect=1/1:frame_rate={rate}"),
    )?;
    graph.add(
        &ff::filter::find("buffersink").ok_or("buffersink filter missing")?,
        "out",
        "",
    )?;
    // Use the source rate, never a hardcoded 25 FPS. FFmpeg owns duplicate/drop
    // decisions and EOF rounding; alpha stays RGBA throughout the filter graph.
    graph.output("in", 0)?.input("out", 0)?.parse(&format!(
        "fps=fps={rate}:start_time=0:round=near:eof_action=round"
    ))?;
    graph.validate()?;
    Ok(graph)
}

fn drain_fps(
    graph: &mut ff::filter::Graph,
    sink: &mut Sink<'_>,
    width: u32,
    height: u32,
) -> MediaResult<()> {
    let mut output = graph.get("out").ok_or("filter output missing")?;
    let base = output.sink().time_base();
    let mut frame = ff::util::frame::Video::empty();
    loop {
        sink.check()?;
        match output.sink().frame(&mut frame) {
            Ok(()) => {}
            Err(ff::Error::Eof)
            | Err(ff::Error::Other {
                errno: ff::error::EAGAIN,
            }) => break,
            Err(error) => return Err(error.into()),
        }
        if frame.width() != width
            || frame.height() != height
            || frame.format() != ff::format::Pixel::RGBA
        {
            return Err("unexpected filtered frame format".into());
        }
        let timestamp = frame.pts().ok_or("filtered timestamp missing")?;
        // fps produces one output tick per frame. Rescale each endpoint so
        // fractional rates do not accumulate millisecond rounding error.
        let start = i32::try_from(timestamp.rescale(base, (1, 1000)))?;
        let end = i32::try_from(
            timestamp
                .checked_add(1)
                .ok_or("timestamp overflow")?
                .rescale(base, (1, 1000)),
        )?;
        let row_size = width as usize * 4;
        let mut rgba = Vec::with_capacity(row_size * height as usize);
        for row in frame.data(0).chunks(frame.stride(0)).take(height as usize) {
            rgba.extend_from_slice(&row[..row_size]);
        }
        sink.add(&rgba, start, end)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn fps_filter_preserves_last_pixels_alpha_and_duration_at_duplicate_times() -> MediaResult<()> {
        ff::init()?;
        let output = tempfile::NamedTempFile::new()?;
        let control = TaskControl::new(Arc::new(tokio::sync::Notify::new()));
        let mut sink = Sink::new(1, 1, &control, output.path(), MAX_BYTES)?;
        let mut graph = fps_filter(1, 1, (1, 1000).into(), (25, 1).into())?;
        let first = [10, 20, 30, 100];
        let middle = [40, 50, 60, 150];
        let last = [70, 80, 90, 200];
        for (timestamp, pixels) in [
            (0, [0; 4]),
            (0, first),
            (40, [0; 4]),
            (40, middle),
            (120, [0; 4]),
            (120, last),
        ] {
            let mut frame = ff::util::frame::Video::new(ff::format::Pixel::RGBA, 1, 1);
            frame.data_mut(0)[..4].copy_from_slice(&pixels);
            frame.set_pts(Some(timestamp));
            graph
                .get("in")
                .ok_or("filter input missing")?
                .source()
                .add(&frame)?;
            drain_fps(&mut graph, &mut sink, 1, 1)?;
        }
        graph
            .get("in")
            .ok_or("filter input missing")?
            .source()
            .close(160)?;
        drain_fps(&mut graph, &mut sink, 1, 1)?;
        sink.finish()?;
        let bytes = std::fs::read(output.path())?;
        let decoded: Vec<_> = webp_animation::Decoder::new(&bytes)?.into_iter().collect();
        assert_eq!(decoded.len(), 4);
        for (frame, pixels) in decoded.iter().zip([first, middle, middle, last]) {
            assert_eq!(frame.data(), pixels);
        }
        assert_eq!(
            decoded
                .iter()
                .map(|frame| frame.timestamp())
                .collect::<Vec<_>>(),
            [40, 80, 120, 160]
        );
        assert_eq!(&bytes[38..44], &[0; 6]);
        Ok(())
    }

    #[test]
    fn fps_filter_uses_fractional_source_rate_and_flushes_last_frame() -> MediaResult<()> {
        ff::init()?;
        let output = tempfile::NamedTempFile::new()?;
        let control = TaskControl::new(Arc::new(tokio::sync::Notify::new()));
        let mut sink = Sink::new(1, 1, &control, output.path(), MAX_BYTES)?;
        let mut graph = fps_filter(1, 1, (1, 30000).into(), (30000, 1001).into())?;
        for timestamp in [0, 1001, 2002] {
            let mut frame = ff::util::frame::Video::new(ff::format::Pixel::RGBA, 1, 1);
            frame.data_mut(0)[..4].copy_from_slice(&[100, 150, 200, 255]);
            frame.set_pts(Some(timestamp));
            graph
                .get("in")
                .ok_or("filter input missing")?
                .source()
                .add(&frame)?;
            drain_fps(&mut graph, &mut sink, 1, 1)?;
        }
        graph
            .get("in")
            .ok_or("filter input missing")?
            .source()
            .close(3003)?;
        drain_fps(&mut graph, &mut sink, 1, 1)?;
        sink.finish()?;
        let bytes = std::fs::read(output.path())?;
        let decoded: Vec<_> = webp_animation::Decoder::new(&bytes)?.into_iter().collect();
        assert_eq!(
            decoded
                .iter()
                .map(|frame| frame.timestamp())
                .collect::<Vec<_>>(),
            [33, 67, 100]
        );
        Ok(())
    }

    #[test]
    fn fps_filter_output_remains_bounded_and_cancellable() -> MediaResult<()> {
        ff::init()?;
        for (budget, cancel) in [(MAX_BYTES, false), (60, false), (MAX_BYTES, true)] {
            let output = tempfile::NamedTempFile::new()?;
            let control = TaskControl::new(Arc::new(tokio::sync::Notify::new()));
            let mut sink = Sink::new(1, 1, &control, output.path(), budget)?;
            let mut graph = fps_filter(1, 1, (1, 1000).into(), (100, 1).into())?;
            let mut frame = ff::util::frame::Video::new(ff::format::Pixel::RGBA, 1, 1);
            frame.data_mut(0)[..4].copy_from_slice(&[10, 20, 30, 255]);
            frame.set_pts(Some(0));
            graph
                .get("in")
                .ok_or("filter input missing")?
                .source()
                .add(&frame)?;
            graph
                .get("in")
                .ok_or("filter input missing")?
                .source()
                .close(3000)?;
            if cancel {
                control.cancel();
            }
            let error = drain_fps(&mut graph, &mut sink, 1, 1)
                .err()
                .ok_or("filter escaped limits")?;
            if cancel {
                assert_eq!(
                    error
                        .downcast_ref::<CoreError>()
                        .ok_or("missing core error")?
                        .code(),
                    ErrorCode::Cancelled
                );
            } else if budget == 60 {
                assert_eq!(
                    error
                        .downcast_ref::<CoreError>()
                        .ok_or("missing core error")?
                        .code(),
                    ErrorCode::ResourceLimit
                );
                assert!(output.path().metadata()?.len() <= budget);
            } else {
                assert_eq!(error.to_string(), "frame budget exceeded");
            }
        }
        Ok(())
    }
}
