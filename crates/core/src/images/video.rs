//! Native contexts remain on a single blocking worker. Decode into a small
//! filtered canvas; never retain full-resolution RGBA animation buffers.
use crate::{
    CoreError, ErrorCode, ResourceLimits, Result,
    creator::{VideoEdit, VideoInfo},
    tasks::TaskControl,
};
use ffmpeg_next::{self as ff, Rescale};
use memedock_storage::files::StagedFile;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) const DEADLINE: Duration = Duration::from_secs(120);
pub(super) fn check(control: &TaskControl, started: Instant) -> Result<()> {
    control.check()?;
    if started.elapsed() > DEADLINE {
        return Err(CoreError::new(
            ErrorCode::Timeout,
            "video processing timed out",
        ));
    }
    Ok(())
}
fn media(error: ff::Error) -> CoreError {
    CoreError::caused(ErrorCode::InvalidImage, "video decoding failed", error)
}
pub(crate) struct Reader {
    input: ff::format::context::Input,
    decoder: ff::decoder::Video,
    index: usize,
    base: ff::Rational,
    origin: i64,
    rotation: u32,
    sar: f64,
    dimensions: (u32, u32),
    pub info: VideoInfo,
}
fn rotation(stream: &ff::format::stream::Stream<'_>) -> Result<u32> {
    for data in stream.side_data() {
        if data.kind() != ff::codec::packet::side_data::Type::DisplayMatrix {
            continue;
        }
        let values: Vec<i32> = data
            .data()
            .chunks_exact(4)
            .map(|b| i32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        if values.len() != 9 {
            return Err(CoreError::new(
                ErrorCode::InvalidImage,
                "invalid video display matrix",
            ));
        }
        let (a, b, c, d) = (
            f64::from(values[0]),
            f64::from(values[1]),
            f64::from(values[3]),
            f64::from(values[4]),
        );
        let angle = b.atan2(a).to_degrees().rem_euclid(360.0);
        let quarter = (angle / 90.0).round();
        if a * d - b * c <= 0.0
            || (a.hypot(b) - 65536.0).abs() > 2.0
            || (c.hypot(d) - 65536.0).abs() > 2.0
            || (a * c + b * d).abs() > 131072.0
            || (angle - quarter * 90.0).abs() > 0.1
            || values[2] != 0
            || values[5] != 0
        {
            return Err(CoreError::new(
                ErrorCode::UnsupportedFormat,
                "unsupported video display transform",
            ));
        }
        return Ok((quarter as u32 % 4) * 90);
    }
    Ok(0)
}
fn sdr(transfer: ff::color::TransferCharacteristic) -> Result<()> {
    use ff::color::TransferCharacteristic::*;
    if matches!(
        transfer,
        SMPTE2084 | ARIB_STD_B67 | SMPTE428 | BT2020_10 | BT2020_12
    ) {
        return Err(CoreError::new(
            ErrorCode::UnsupportedColorProfile,
            "HDR video is not supported by Creator yet",
        ));
    }
    Ok(())
}
impl Reader {
    pub fn open(
        source: &StagedFile,
        limits: &ResourceLimits,
        control: &Arc<TaskControl>,
        started: Instant,
    ) -> Result<Self> {
        check(control, started)?;
        ff::init().map_err(media)?;
        let io =
            ff::format::context::StreamIo::from_read_seek(source.open_read()?).map_err(media)?;
        let token = control.clone();
        let mut options = ff::Dictionary::new();
        options.set("probesize", "5000000");
        options.set("analyzeduration", "5000000");
        let input =
            ff::format::input_from_stream_with_interrupt(io, None, Some(options), move || {
                token.is_cancelled() || started.elapsed() > DEADLINE
            })
            .map_err(media)?;
        let stream = input
            .streams()
            .best(ff::media::Type::Video)
            .ok_or_else(|| CoreError::new(ErrorCode::UnsupportedFormat, "video stream missing"))?;
        let base = stream.time_base();
        if base.numerator() <= 0 || base.denominator() <= 0 {
            return Err(CoreError::new(
                ErrorCode::InvalidImage,
                "invalid video time base",
            ));
        }
        let duration = if stream.duration() > 0 {
            stream.duration().rescale(base, (1, 1000))
        } else {
            input.duration() / 1000
        };
        if !(1..=86_400_000).contains(&duration) {
            return Err(CoreError::new(
                ErrorCode::UnsupportedFormat,
                "video duration missing or exceeds 24 hours",
            ));
        }
        let rotation = rotation(&stream)?;
        let rate = stream.avg_frame_rate();
        let rate = if rate.numerator() > 0 && rate.denominator() > 0 {
            f64::from(rate)
        } else {
            f64::from(stream.rate())
        };
        if !rate.is_finite() || !(0.1..=240.0).contains(&rate) {
            return Err(CoreError::new(
                ErrorCode::UnsupportedFormat,
                "unsupported video frame rate",
            ));
        }
        let index = stream.index();
        let origin = if stream.start_time() == ff::ffi::AV_NOPTS_VALUE {
            0
        } else {
            stream.start_time()
        };
        let mut context =
            ff::codec::context::Context::from_parameters(stream.parameters()).map_err(media)?;
        context.set_threading(ff::codec::threading::Config::count(1));
        let id = stream.parameters().id();
        let codec = match id {
            ff::codec::Id::VP8 => ff::decoder::find_by_name("libvpx"),
            ff::codec::Id::VP9 => ff::decoder::find_by_name("libvpx-vp9"),
            _ => ff::decoder::find(id),
        }
        .ok_or_else(|| {
            CoreError::new(
                ErrorCode::UnsupportedFormat,
                "video codec is not available in this build",
            )
        })?;
        let mut decoder_options = ff::Dictionary::new();
        decoder_options.set(
            "max_pixels",
            &limits
                .max_frame_pixels
                .min(limits.max_decode_bytes / 24)
                .to_string(),
        );
        let decoder = context
            .decoder()
            .open_as_with(codec, decoder_options)
            .map_err(media)?
            .video()
            .map_err(media)?;
        let pixels = u64::from(decoder.width()) * u64::from(decoder.height());
        if pixels == 0
            || pixels > limits.max_frame_pixels
            || pixels.saturating_mul(24) > limits.max_decode_bytes
        {
            return Err(CoreError::new(
                ErrorCode::ResourceLimit,
                "video decoder exceeds working budget",
            ));
        }
        sdr(decoder.color_transfer_characteristic())?;
        let ratio = decoder.aspect_ratio();
        let sar = if ratio.numerator() > 0 && ratio.denominator() > 0 {
            f64::from(ratio)
        } else {
            1.0
        };
        if !(0.1..=10.0).contains(&sar) {
            return Err(CoreError::new(
                ErrorCode::UnsupportedFormat,
                "unsupported video pixel aspect ratio",
            ));
        }
        let (mut width, mut height) = (
            (f64::from(decoder.width()) * sar).round().max(1.0) as u32,
            decoder.height(),
        );
        if rotation == 90 || rotation == 270 {
            std::mem::swap(&mut width, &mut height);
        }
        let info = VideoInfo {
            mime: match input.format().name() {
                name if name.contains("mov") => "video/mp4",
                name if name.contains("matroska") => "video/webm",
                "mpegts" => "video/mp2t",
                "avi" => "video/x-msvideo",
                _ => "application/octet-stream",
            }
            .into(),
            width,
            height,
            duration_ms: duration as u64,
            frame_rate: rate,
        };
        check(control, started)?;
        let dimensions = (decoder.width(), decoder.height());
        Ok(Self {
            input,
            decoder,
            index,
            base,
            origin,
            rotation,
            sar,
            dimensions,
            info,
        })
    }
    fn graph(
        &self,
        frame: &ff::frame::Video,
        edit: VideoEdit,
    ) -> Result<(ff::filter::Graph, u32, u32)> {
        let (mut w, mut h) = (frame.width(), frame.height());
        if self.rotation == 90 || self.rotation == 270 {
            std::mem::swap(&mut w, &mut h);
        }
        let crop = edit.crop();
        let x = (crop.x() * f64::from(w)).floor() as u32;
        let y = (crop.y() * f64::from(h)).floor() as u32;
        let cw = ((crop.width() * f64::from(w)).round() as u32).min(w.saturating_sub(x));
        let ch = ((crop.height() * f64::from(h)).round() as u32).min(h.saturating_sub(y));
        if cw == 0 || ch == 0 {
            return Err(CoreError::new(
                ErrorCode::InvalidInput,
                "crop is smaller than one pixel",
            ));
        }
        let (dw, dh) = if self.rotation == 90 || self.rotation == 270 {
            (f64::from(cw), f64::from(ch) * self.sar)
        } else {
            (f64::from(cw) * self.sar, f64::from(ch))
        };
        let scale = (f64::from(edit.max_edge()) / dw.max(dh)).min(1.0);
        let (ow, oh) = (
            (dw * scale).round().max(1.0) as u32,
            (dh * scale).round().max(1.0) as u32,
        );
        let mut graph = ff::filter::Graph::new();
        graph
            .add(
                &ff::filter::find("buffer")
                    .ok_or_else(|| CoreError::internal("buffer filter missing"))?,
                "in",
                &format!(
                    "video_size={}x{}:pix_fmt={}:time_base={}:pixel_aspect=1/1:colorspace={}:range={}",
                    frame.width(),
                    frame.height(),
                    Into::<ff::ffi::AVPixelFormat>::into(frame.format()) as i32,
                    self.base,
                    Into::<ff::ffi::AVColorSpace>::into(frame.color_space()) as i32,
                    Into::<ff::ffi::AVColorRange>::into(frame.color_range()) as i32
                ),
            )
            .map_err(media)?;
        graph
            .add(
                &ff::filter::find("buffersink")
                    .ok_or_else(|| CoreError::internal("sink filter missing"))?,
                "out",
                "",
            )
            .map_err(media)?;
        let rotate = match self.rotation {
            90 => "transpose=clock,",
            180 => "hflip,vflip,",
            270 => "transpose=cclock,",
            _ => "",
        };
        let filters = format!(
            "{rotate}crop={cw}:{ch}:{x}:{y}:exact=1,scale={ow}:{oh}:flags=lanczos,format=rgba,setsar=1"
        );
        graph
            .output("in", 0)
            .map_err(media)?
            .input("out", 0)
            .map_err(media)?
            .parse(&filters)
            .map_err(media)?;
        graph.validate().map_err(media)?;
        Ok((graph, ow, oh))
    }
    /// Visits sampled small canvases with exact cumulative millisecond endpoints.
    pub fn render(
        &mut self,
        edit: VideoEdit,
        control: &Arc<TaskControl>,
        started: Instant,
        mut output: impl FnMut(&[u8], u32, u32, u64, u64) -> Result<()>,
    ) -> Result<()> {
        let target = (edit.start_ms() as i64)
            .rescale((1, 1000), self.base)
            .saturating_add(self.origin)
            .rescale(self.base, (1, 1_000_000));
        self.input.seek(target, ..target).map_err(media)?;
        self.decoder.flush();
        let mut graph = None;
        let mut previous: Option<(Vec<u8>, u32, u32)> = None;
        let mut sample = 0u64;
        let mut sample_index = 0u64;
        let duration = edit.duration_ms();
        let mut last_timestamp = None;
        let mut last_end = 0i64;
        let mut reached_end = false;
        let mut decoded_count = 0usize;
        let mut packets = 0usize;
        let base = self.base;
        let origin = self.origin;
        let fallback_ms = (1000.0 / self.info.frame_rate).ceil() as i64;
        let mut frame = ff::frame::Video::empty();
        loop {
            check(control, started)?;
            match self.decoder.receive_frame(&mut frame) {
                Ok(()) => {
                    decoded_count += 1;
                    if decoded_count > 30_000 {
                        return Err(CoreError::new(
                            ErrorCode::ResourceLimit,
                            "video decode frame limit exceeded",
                        ));
                    }
                    if frame.is_corrupt() || frame.has_decode_errors() {
                        return Err(CoreError::new(
                            ErrorCode::InvalidImage,
                            "corrupt video frame",
                        ));
                    }
                    if (frame.width(), frame.height()) != self.dimensions {
                        return Err(CoreError::new(
                            ErrorCode::UnsupportedFormat,
                            "video dimensions changed",
                        ));
                    }
                    sdr(frame.color_transfer_characteristic())?;
                    let pts = frame.timestamp().ok_or_else(|| {
                        CoreError::new(ErrorCode::InvalidImage, "video frame timestamp missing")
                    })?;
                    if last_timestamp.is_some_and(|last| pts < last) {
                        return Err(CoreError::new(
                            ErrorCode::InvalidImage,
                            "video timestamp moved backwards",
                        ));
                    }
                    last_timestamp = Some(pts);
                    let ms = pts.saturating_sub(origin).rescale(base, (1, 1000));
                    let frame_duration = frame.packet().duration.rescale(base, (1, 1000)).max(1);
                    last_end = ms.saturating_add(frame_duration.max(fallback_ms));
                    while sample < duration && ((edit.start_ms() + sample) as i64) < ms {
                        if let Some((pixels, w, h)) = &previous {
                            sample_index += 1;
                            let end = (sample_index * 1000 / u64::from(edit.fps())).min(duration);
                            output(pixels, *w, *h, sample, end)?;
                            sample = end;
                        } else {
                            break;
                        }
                    }
                    if ms >= edit.end_ms() as i64 {
                        reached_end = true;
                        break;
                    }
                    if graph.is_none() {
                        graph = Some(self.graph(&frame, edit)?);
                    }
                    let (filter, w, h) = graph
                        .as_mut()
                        .ok_or_else(|| CoreError::internal("video graph missing"))?;
                    filter
                        .get("in")
                        .ok_or_else(|| CoreError::internal("video filter input missing"))?
                        .source()
                        .add(&frame)
                        .map_err(media)?;
                    let mut small = ff::frame::Video::empty();
                    filter
                        .get("out")
                        .ok_or_else(|| CoreError::internal("video filter output missing"))?
                        .sink()
                        .frame(&mut small)
                        .map_err(media)?;
                    let mut rgba = Vec::with_capacity(*w as usize * *h as usize * 4);
                    for row in small.data(0).chunks(small.stride(0)).take(*h as usize) {
                        rgba.extend_from_slice(&row[..*w as usize * 4]);
                    }
                    previous = Some((rgba, *w, *h));
                }
                Err(ff::Error::Eof) => break,
                Err(ff::Error::Other {
                    errno: ff::error::EAGAIN,
                }) => {
                    let mut packet = ff::Packet::empty();
                    loop {
                        check(control, started)?;
                        packets += 1;
                        if packets > 200_000 {
                            return Err(CoreError::new(
                                ErrorCode::ResourceLimit,
                                "video packet limit exceeded",
                            ));
                        }
                        match packet.read(&mut self.input) {
                            Ok(()) if packet.stream() == self.index => {
                                self.decoder.send_packet(&packet).map_err(media)?;
                                break;
                            }
                            Ok(()) => {}
                            Err(ff::Error::Eof) => {
                                self.decoder.send_eof().map_err(media)?;
                                break;
                            }
                            Err(error) => {
                                check(control, started)?;
                                return Err(media(error));
                            }
                        }
                    }
                }
                Err(error) => return Err(media(error)),
            }
        }
        if !reached_end && last_end + 2 < edit.end_ms() as i64 {
            return Err(CoreError::new(
                ErrorCode::InvalidImage,
                "video ended before selected interval",
            ));
        }
        let (pixels, w, h) = previous.ok_or_else(|| {
            CoreError::new(ErrorCode::InvalidImage, "video selection has no frames")
        })?;
        while sample < duration {
            check(control, started)?;
            sample_index += 1;
            let end = (sample_index * 1000 / u64::from(edit.fps())).min(duration);
            output(&pixels, w, h, sample, end)?;
            sample = end;
        }
        Ok(())
    }
}
pub(crate) fn inspect(
    source: &StagedFile,
    limits: &ResourceLimits,
    control: &Arc<TaskControl>,
) -> Result<VideoInfo> {
    let started = Instant::now();
    let result = Reader::open(source, limits, control, started).map(|v| v.info);
    check(control, started)?;
    result
}
