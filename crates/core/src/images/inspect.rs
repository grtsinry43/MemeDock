use crate::{CoreError, ErrorCode, ResourceLimits, Result, tasks::TaskControl};
use image::{AnimationDecoder, DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use memedock_domain::asset::ImageFormat as Format;
use std::{
    fs::File,
    io::{BufReader, Seek, SeekFrom},
};

pub(crate) struct Decoded {
    pub(crate) image: DynamicImage,
    pub(crate) format: Format,
    pub(crate) dimensions: (u32, u32),
    pub(crate) animated: bool,
}
pub(crate) fn error(e: image::ImageError) -> CoreError {
    let code = if matches!(e, image::ImageError::Limits(_)) {
        ErrorCode::ResourceLimit
    } else {
        ErrorCode::InvalidImage
    };
    CoreError::caused(code, "image decoding failed", e)
}
fn bounds(decoder: &mut impl ImageDecoder, limits: &ResourceLimits) -> Result<(u32, u32)> {
    let (w, h) = decoder.dimensions();
    let pixels = u64::from(w) * u64::from(h);
    // Animation composition and orientation may retain multiple RGBA canvases.
    let estimate = pixels
        .checked_mul(16)
        .ok_or_else(|| CoreError::new(ErrorCode::ResourceLimit, "image allocation overflow"))?;
    if w == 0 || h == 0 || pixels > limits.max_frame_pixels || estimate > limits.max_decode_bytes {
        return Err(CoreError::new(
            ErrorCode::ResourceLimit,
            "image exceeds decode budget",
        ));
    }
    let mut codec_limits = Limits::default();
    codec_limits.max_image_width = Some(w);
    codec_limits.max_image_height = Some(h);
    codec_limits.max_alloc = Some(limits.max_decode_bytes);
    decoder.set_limits(codec_limits).map_err(error)?;
    Ok((w, h))
}
fn frames<'a>(
    mut frames: image::Frames<'a>,
    validate_all: bool,
    control: &TaskControl,
) -> Result<(DynamicImage, bool)> {
    control.check()?;
    let first = frames
        .next()
        .ok_or_else(|| CoreError::new(ErrorCode::InvalidImage, "image has no display frame"))?
        .map_err(error)?;
    let mut count = 1usize;
    if validate_all {
        for frame in frames {
            control.check()?;
            frame.map_err(error)?;
            count = count.saturating_add(1);
        }
    }
    Ok((DynamicImage::ImageRgba8(first.into_buffer()), count > 1))
}
/// Full import validation streams all animation frames; thumbnail generation
/// decodes only the first composed frame. Neither path collects the animation.
pub(crate) fn decode(
    file: File,
    limits: &ResourceLimits,
    control: &TaskControl,
    validate_all: bool,
) -> Result<Decoded> {
    decode_internal(file, limits, control, validate_all, false)
}
pub(crate) fn decode_export(
    file: File,
    limits: &ResourceLimits,
    control: &TaskControl,
) -> Result<Decoded> {
    decode_internal(file, limits, control, false, true)
}
fn check_profile(decoder: &mut impl ImageDecoder, reject_profile: bool) -> Result<()> {
    if reject_profile && decoder.icc_profile().map_err(error)?.is_some() {
        return Err(CoreError::new(
            ErrorCode::UnsupportedColorProfile,
            "embedded color profile requires unverified color conversion; use original",
        ));
    }
    Ok(())
}
fn decode_internal(
    mut file: File,
    limits: &ResourceLimits,
    control: &TaskControl,
    validate_all: bool,
    reject_profile: bool,
) -> Result<Decoded> {
    control.check()?;
    let reader = ImageReader::new(BufReader::new(file.try_clone()?))
        .with_guessed_format()
        .map_err(CoreError::from)?;
    let detected = reader
        .format()
        .ok_or_else(|| CoreError::new(ErrorCode::UnsupportedFormat, "unsupported image format"))?;
    let format = match detected {
        ImageFormat::Png => Format::Png,
        ImageFormat::Jpeg => Format::Jpeg,
        ImageFormat::Gif => Format::Gif,
        ImageFormat::WebP => Format::WebP,
        _ => {
            return Err(CoreError::new(
                ErrorCode::UnsupportedFormat,
                "unsupported image format",
            ));
        }
    };
    // Duplicated file descriptors share the seek position.
    file.seek(SeekFrom::Start(0))?;
    let (mut image, dimensions, animated, orientation) = match format {
        Format::Gif => {
            let mut decoder =
                image::codecs::gif::GifDecoder::new(BufReader::new(file)).map_err(error)?;
            let dimensions = bounds(&mut decoder, limits)?;
            check_profile(&mut decoder, reject_profile)?;
            let (image, animated) = frames(decoder.into_frames(), validate_all, control)?;
            (
                image,
                dimensions,
                animated,
                image::metadata::Orientation::NoTransforms,
            )
        }
        Format::WebP => {
            let mut decoder =
                image::codecs::webp::WebPDecoder::new(BufReader::new(file)).map_err(error)?;
            let dimensions = bounds(&mut decoder, limits)?;
            check_profile(&mut decoder, reject_profile)?;
            let animated = decoder.has_animation();
            let orientation = decoder.orientation().map_err(error)?;
            let image = if animated {
                frames(decoder.into_frames(), validate_all, control)?.0
            } else {
                DynamicImage::from_decoder(decoder).map_err(error)?
            };
            (image, dimensions, animated, orientation)
        }
        Format::Png => {
            let mut initial = Limits::default();
            initial.max_alloc = Some(limits.max_decode_bytes);
            let mut decoder =
                image::codecs::png::PngDecoder::with_limits(BufReader::new(file), initial)
                    .map_err(error)?;
            let dimensions = bounds(&mut decoder, limits)?;
            check_profile(&mut decoder, reject_profile)?;
            let animated = decoder.is_apng().map_err(error)?;
            let orientation = decoder.orientation().map_err(error)?;
            let image = if animated {
                frames(
                    decoder.apng().map_err(error)?.into_frames(),
                    validate_all,
                    control,
                )?
                .0
            } else {
                DynamicImage::from_decoder(decoder).map_err(error)?
            };
            (image, dimensions, animated, orientation)
        }
        Format::Jpeg => {
            let mut decoder = ImageReader::with_format(BufReader::new(file), detected)
                .into_decoder()
                .map_err(error)?;
            let dimensions = bounds(&mut decoder, limits)?;
            check_profile(&mut decoder, reject_profile)?;
            let orientation = decoder.orientation().map_err(error)?;
            (
                DynamicImage::from_decoder(decoder).map_err(error)?,
                dimensions,
                false,
                orientation,
            )
        }
    };
    control.check()?;
    image.apply_orientation(orientation);
    Ok(Decoded {
        image,
        format,
        dimensions,
        animated,
    })
}
