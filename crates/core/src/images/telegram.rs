//! Telegram-only conversion. Native contexts stay entirely on one blocking worker.
use super::animated_webp::Sink;
use crate::sources::telegram::TelegramFormat;
use crate::{CoreError, ErrorCode, Result, tasks::TaskControl};
use ffmpeg_next as ff;
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
    dimensions(width, height)?;
    let fallback_ms = if rate.numerator() > 0 && rate.denominator() > 0 {
        (1000.0 * f64::from(rate.denominator()) / f64::from(rate.numerator())).round() as i32
    } else {
        return Err("video frame rate missing".into());
    };
    let mut sink = Sink::new(
        width,
        height,
        control,
        output,
        max_output_bytes.min(MAX_BYTES),
    )?;
    let mut scaler = None;
    let mut first_ms = None;
    if base.numerator() <= 0 || base.denominator() <= 0 || !(1..=1000).contains(&fallback_ms) {
        return Err("invalid video time base".into());
    }
    let to_ms = |ticks: i64| -> i64 {
        (ticks as f64 * f64::from(base.numerator()) * 1000.0 / f64::from(base.denominator()))
            .round() as i64
    };
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
            if decoded.is_corrupt() || decoded.has_decode_errors() {
                return Err("corrupt decoded frame".into());
            }
            if decoded.width() != width || decoded.height() != height {
                return Err("dimensions changed".into());
            }
            let ms = to_ms(decoded.timestamp().ok_or("frame timestamp missing")?);
            let start_ms = i32::try_from(
                ms.checked_sub(*first_ms.get_or_insert(ms))
                    .ok_or("timestamp overflow")?,
            )?;
            let delta = i32::try_from(to_ms(decoded.packet().duration))?;
            let end_ms = start_ms
                .checked_add(if delta > 0 { delta } else { fallback_ms })
                .ok_or("duration overflow")?;
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
            let row_size = width as usize * 4;
            let mut rgba = Vec::with_capacity(row_size * height as usize);
            for row in frame.data(0).chunks(frame.stride(0)).take(height as usize) {
                rgba.extend_from_slice(&row[..row_size]);
            }
            sink.add(&rgba, start_ms, end_ms)?;
        }
        Ok(())
    };
    let mut packet = ff::Packet::empty();
    loop {
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
    sink.finish()
}
