use crate::{CoreError, ErrorCode, ResourceLimits, Result, tasks::TaskControl};
use image::{DynamicImage, ImageEncoder, RgbaImage, imageops::FilterType};
use memedock_domain::{asset::ImageFormat, export::ExportOptions};
use memedock_storage::files::{FsBlobStore, StagedFile};
use std::{
    fs::File,
    io::{self, Write},
};

struct BoundedWriter<'a> {
    file: File,
    remaining: u64,
    control: &'a TaskControl,
    exceeded: bool,
}
impl Write for BoundedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.control.is_cancelled() {
            return Err(io::Error::other("export cancelled"));
        }
        if bytes.len() as u64 > self.remaining {
            self.exceeded = true;
            return Err(io::Error::other("export output quota exceeded"));
        }
        let written = self.file.write(bytes)?;
        self.remaining -= written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn resize(mut rgba: RgbaImage, edge: Option<u32>, control: &TaskControl) -> Result<RgbaImage> {
    let Some(edge) = edge.filter(|edge| rgba.width().max(rgba.height()) > *edge) else {
        return Ok(rgba);
    };
    // Filter premultiplied colors so invisible RGB cannot create dark halos.
    for row in rgba.rows_mut() {
        control.check()?;
        for pixel in row {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel.0[..3] {
                *channel = ((u32::from(*channel) * alpha + 127) / 255) as u8;
            }
        }
    }
    let mut result = DynamicImage::ImageRgba8(rgba)
        .resize(edge, edge, FilterType::Lanczos3)
        .into_rgba8();
    for row in result.rows_mut() {
        control.check()?;
        for pixel in row {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel.0[..3] {
                *channel = if alpha == 0 {
                    0
                } else {
                    ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8
                };
            }
        }
    }
    Ok(result)
}

pub(crate) fn encode(
    original: File,
    blobs: &FsBlobStore,
    options: ExportOptions,
    limits: &ResourceLimits,
    available: u64,
    control: &TaskControl,
) -> Result<StagedFile> {
    control.stage("decoding_export");
    let decoded = super::inspect::decode_export(original, limits, control)?;
    // Lanczos retains an f32 intermediate canvas in addition to RGBA source
    // and destination. Reserve its peak, not merely the decoder's buffers.
    let pixels = u64::from(decoded.image.width()) * u64::from(decoded.image.height());
    if pixels
        .checked_mul(32)
        .is_none_or(|bytes| bytes > limits.max_decode_bytes)
    {
        return Err(CoreError::new(
            ErrorCode::ResourceLimit,
            "export exceeds image working budget",
        ));
    }
    let format = options.output_format(decoded.format);
    let mut rgba = resize(decoded.image.into_rgba8(), options.max_edge(), control)?;
    if let Some(background) = options.background_rgba() {
        let background = background.to_be_bytes();
        for row in rgba.rows_mut() {
            control.check()?;
            for pixel in row {
                let a = u32::from(pixel[3]);
                let b = u32::from(background[3]);
                let alpha = a * 255 + b * (255 - a);
                for channel in 0..3 {
                    pixel[channel] = if alpha == 0 {
                        0
                    } else {
                        ((u32::from(pixel[channel]) * a * 255
                            + u32::from(background[channel]) * b * (255 - a)
                            + alpha / 2)
                            / alpha) as u8
                    };
                }
                pixel[3] = ((alpha + 127) / 255) as u8;
            }
        }
    }
    control.stage("encoding_export");
    let pending = blobs.create_staging()?;
    let result = (|| -> Result<StagedFile> {
        let mut writer = BoundedWriter {
            file: File::options().write(true).open(pending.path())?,
            remaining: available,
            control,
            exceeded: false,
        };
        let encoded = match format {
            ImageFormat::Png => image::codecs::png::PngEncoder::new(&mut writer).write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgba8,
            ),
            ImageFormat::Jpeg => {
                let rgb = DynamicImage::ImageRgba8(rgba).into_rgb8();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 85)
                    .encode_image(&rgb)
            }
            _ => return Err(CoreError::internal("invalid derived output format")),
        };
        control.check()?;
        if writer.exceeded {
            return Err(CoreError::new(
                ErrorCode::ResourceLimit,
                "export output quota exceeded",
            ));
        }
        encoded.map_err(|error| match error {
            image::ImageError::IoError(error) => CoreError::from(error),
            other => super::inspect::error(other),
        })?;
        writer.flush()?;
        drop(writer);
        pending
            .finish(available, || control.is_cancelled())
            .map_err(Into::into)
    })();
    if result.is_err() {
        pending.discard()?;
    }
    result
}
