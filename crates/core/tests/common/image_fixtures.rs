use super::TestResult;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

pub fn encoded(
    format: ImageFormat,
    width: u32,
    height: u32,
    color: [u8; 4],
) -> TestResult<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(width, height, Rgba(color)))
        .write_to(&mut output, format)?;
    Ok(output.into_inner())
}
fn riff_chunk(output: &mut Vec<u8>, name: &[u8; 4], data: &[u8]) -> TestResult {
    output.extend(name);
    output.extend(u32::try_from(data.len())?.to_le_bytes());
    output.extend(data);
    if !data.len().is_multiple_of(2) {
        output.push(0);
    }
    Ok(())
}
/// Synthetic lossless animation following Google's RIFF container specification.
/// The first update occupies only a subrectangle of the transparent canvas.
pub fn animated_webp() -> TestResult<Vec<u8>> {
    let mut chunks = Vec::new();
    riff_chunk(&mut chunks, b"VP8X", &[0x12, 0, 0, 0, 39, 0, 0, 19, 0, 0])?;
    riff_chunk(&mut chunks, b"ANIM", &[0; 6])?;
    for color in [[200, 10, 20, 128], [10, 200, 20, 255]] {
        let pixels = encoded(ImageFormat::WebP, 20, 10, color)?;
        let mut frame = vec![4, 0, 0, 3, 0, 0, 19, 0, 0, 9, 0, 0, 50, 0, 0, 2];
        assert_eq!(&pixels[12..16], b"VP8L");
        frame.extend(&pixels[12..]);
        riff_chunk(&mut chunks, b"ANMF", &frame)?;
    }
    let mut output = b"RIFF".to_vec();
    output.extend(u32::try_from(chunks.len() + 4)?.to_le_bytes());
    output.extend(b"WEBP");
    output.extend(chunks);
    Ok(output)
}
fn png_chunk(output: &mut Vec<u8>, name: &[u8; 4], data: &[u8]) -> TestResult {
    output.extend(u32::try_from(data.len())?.to_be_bytes());
    output.extend(name);
    output.extend(data);
    let mut crc = u32::MAX;
    for byte in name.iter().chain(data) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    output.extend((!crc).to_be_bytes());
    Ok(())
}
/// Two full frames, with sequence numbers and CRCs prescribed by PNG Third Edition.
pub fn apng() -> TestResult<Vec<u8>> {
    let png = encoded(ImageFormat::Png, 40, 20, [200, 10, 20, 128])?;
    let mut chunks = Vec::new();
    let mut at = 8;
    while at < png.len() {
        let size = u32::from_be_bytes(png[at..at + 4].try_into()?) as usize;
        chunks.push((&png[at + 4..at + 8], &png[at + 8..at + 8 + size]));
        at += size + 12;
    }
    let mut output = png[..8].to_vec();
    png_chunk(&mut output, b"IHDR", chunks[0].1)?;
    let mut animation = 2u32.to_be_bytes().to_vec();
    animation.extend(0u32.to_be_bytes());
    png_chunk(&mut output, b"acTL", &animation)?;
    for sequence in [0u32, 1u32] {
        let mut frame = sequence.to_be_bytes().to_vec();
        frame.extend(40u32.to_be_bytes());
        frame.extend(20u32.to_be_bytes());
        frame.extend([0; 8]);
        frame.extend([0, 1, 0, 10, 0, 0]);
        png_chunk(&mut output, b"fcTL", &frame)?;
        for (name, data) in &chunks {
            if *name == b"IDAT" {
                if sequence == 0 {
                    png_chunk(&mut output, b"IDAT", data)?;
                } else {
                    let mut payload = 2u32.to_be_bytes().to_vec();
                    payload.extend(*data);
                    png_chunk(&mut output, b"fdAT", &payload)?;
                }
            }
        }
    }
    png_chunk(&mut output, b"IEND", &[])?;
    Ok(output)
}
