//! Full-canvas lossless frames, streamed into the standard WebP RIFF container.
//! See https://developers.google.com/speed/webp/docs/riff_container.
//! Retains one encoded frame so its duration can use the next frame timestamp.
use super::telegram::MediaResult;
use crate::{CoreError, ErrorCode, tasks::TaskControl};
use image::codecs::webp::WebPEncoder;
use std::{
    fs::File,
    io::{Seek, SeekFrom, Write},
    path::Path,
};

pub(super) struct Sink<'a> {
    file: File,
    width: u32,
    height: u32,
    control: &'a TaskControl,
    current: Option<(i32, Vec<u8>)>,
    end_ms: i32,
    frames: usize,
    max_bytes: u64,
    alpha: bool,
}
fn u24(value: u32) -> [u8; 3] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}
impl<'a> Sink<'a> {
    pub(super) fn new(
        width: u32,
        height: u32,
        control: &'a TaskControl,
        output: &Path,
        max_bytes: u64,
    ) -> MediaResult<Self> {
        let file = File::options().write(true).truncate(true).open(output)?;
        let mut sink = Self {
            file,
            width,
            height,
            control,
            current: None,
            end_ms: 0,
            frames: 0,
            max_bytes,
            alpha: false,
        };
        sink.write(b"RIFF\0\0\0\0WEBP")?;
        let mut extended = vec![0x02, 0, 0, 0]; // Animation; alpha is finalized after encoding.
        extended.extend_from_slice(&u24(width - 1));
        extended.extend_from_slice(&u24(height - 1));
        sink.chunk(b"VP8X", &extended)?;
        sink.chunk(b"ANIM", &[0; 6])?; // Transparent background; loop forever.
        Ok(sink)
    }
    pub(super) fn check(&self) -> MediaResult<()> {
        self.control.check()?;
        Ok(())
    }
    fn write(&mut self, bytes: &[u8]) -> MediaResult<()> {
        if self
            .file
            .stream_position()?
            .checked_add(bytes.len() as u64)
            .is_none_or(|end| end > self.max_bytes)
        {
            return Err(
                CoreError::new(ErrorCode::ResourceLimit, "converted output exceeds limit").into(),
            );
        }
        self.file.write_all(bytes)?;
        Ok(())
    }
    fn chunk(&mut self, kind: &[u8; 4], data: &[u8]) -> MediaResult<()> {
        self.write(kind)?;
        self.write(&u32::try_from(data.len())?.to_le_bytes())?;
        self.write(data)?;
        if !data.len().is_multiple_of(2) {
            self.write(&[0])?;
        }
        Ok(())
    }
    fn flush_frame(&mut self, end_ms: i32) -> MediaResult<()> {
        if let Some((start, data)) = self.current.take() {
            let duration = u32::try_from(end_ms.checked_sub(start).ok_or("timestamp overflow")?)?;
            if duration == 0 || duration > 3000 {
                return Err("invalid frame duration".into());
            }
            let mut payload = Vec::with_capacity(16 + data.len());
            payload.extend_from_slice(&[0; 6]);
            payload.extend_from_slice(&u24(self.width - 1));
            payload.extend_from_slice(&u24(self.height - 1));
            payload.extend_from_slice(&u24(duration));
            payload.push(2); // Replace the full canvas, including transparent pixels.
            payload.extend_from_slice(&data);
            self.chunk(b"ANMF", &payload)?;
        }
        Ok(())
    }
    pub(super) fn add(&mut self, rgba: &[u8], start: i32, end: i32) -> MediaResult<()> {
        self.check()?;
        if self.frames >= 180 {
            return Err("frame budget exceeded".into());
        }
        self.alpha |= rgba.chunks_exact(4).any(|pixel| pixel[3] != 255);
        if start < 0
            || end <= start
            || end > 3000
            || self
                .current
                .as_ref()
                .is_some_and(|(last, _)| start <= *last)
            || self.frames == 0 && start != 0
        {
            return Err("invalid frame timing".into());
        }
        self.flush_frame(start)?;
        if rgba.len() != self.width as usize * self.height as usize * 4 {
            return Err("invalid frame buffer".into());
        }
        let mut encoded = Vec::new();
        WebPEncoder::new_lossless(&mut encoded).encode(
            rgba,
            self.width,
            self.height,
            image::ExtendedColorType::Rgba8,
        )?;
        // The image library owns the pixel codec; only its standard image chunks
        // are placed inside ANMF. No native pointers or animation buffers escape.
        if encoded.len() < 12 || &encoded[..4] != b"RIFF" || &encoded[8..12] != b"WEBP" {
            return Err("invalid encoded frame".into());
        }
        let mut at = 12usize;
        let mut image_data = Vec::new();
        let mut image_found = false;
        while at.checked_add(8).is_some_and(|end| end <= encoded.len()) {
            let size = u32::from_le_bytes(encoded[at + 4..at + 8].try_into()?) as usize;
            let next = at
                .checked_add(8)
                .and_then(|v| v.checked_add(size))
                .and_then(|v| v.checked_add(size % 2))
                .ok_or("frame chunk overflow")?;
            if next > encoded.len() {
                return Err("invalid frame chunk".into());
            }
            let tag = &encoded[at..at + 4];
            if tag == b"ALPH" || tag == b"VP8 " || tag == b"VP8L" {
                image_data.extend_from_slice(&encoded[at..next]);
                image_found |= tag != b"ALPH";
            }
            at = next;
        }
        if !image_found || at != encoded.len() {
            return Err("encoded image chunk missing".into());
        }
        self.current = Some((start, image_data));
        self.end_ms = end;
        self.frames += 1;
        Ok(())
    }
    pub(super) fn finish(mut self) -> MediaResult<()> {
        self.control.check()?;
        if self.frames == 0 {
            return Err("animation has no frames".into());
        }
        self.flush_frame(self.end_ms)?;
        let size = u32::try_from(
            self.file
                .stream_position()?
                .checked_sub(8)
                .ok_or("invalid RIFF size")?,
        )?;
        self.file.seek(SeekFrom::Start(4))?;
        self.file.write_all(&size.to_le_bytes())?;
        self.file.seek(SeekFrom::Start(20))?;
        self.file
            .write_all(&[if self.alpha { 0x12 } else { 0x02 }])?;
        self.file.sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streamed_frames_preserve_alpha_pixels_timing_and_loop() -> MediaResult<()> {
        let output = tempfile::NamedTempFile::new()?;
        let control = TaskControl::new(std::sync::Arc::new(tokio::sync::Notify::new()));
        let pixels = [
            vec![10, 20, 30, 100, 40, 50, 60, 200],
            vec![70, 80, 90, 50, 0, 0, 0, 0],
            vec![100, 110, 120, 255, 130, 140, 150, 10],
        ];
        let mut sink = Sink::new(2, 1, &control, output.path(), 1024 * 1024)?;
        sink.add(&pixels[0], 0, 33)?;
        sink.add(&pixels[1], 40, 73)?;
        sink.add(&pixels[2], 100, 133)?;
        sink.finish()?;
        let bytes = std::fs::read(output.path())?;
        let frames: Vec<_> = webp_animation::Decoder::new(&bytes)?.into_iter().collect();
        assert_eq!(frames.len(), 3);
        for (frame, expected) in frames.iter().zip(&pixels) {
            assert_eq!(frame.data(), expected);
        }
        assert_eq!(
            frames.iter().map(|f| f.timestamp()).collect::<Vec<_>>(),
            [40, 100, 133]
        );
        assert_eq!(&bytes[38..44], &[0; 6]);
        let reader = image::ImageReader::open(output.path())?.with_guessed_format()?;
        assert_eq!(reader.into_dimensions()?, (2, 1));
        Ok(())
    }
    #[test]
    fn output_limit_is_enforced_during_frame_writes() -> MediaResult<()> {
        let output = tempfile::NamedTempFile::new()?;
        let control = TaskControl::new(std::sync::Arc::new(tokio::sync::Notify::new()));
        let mut sink = Sink::new(1, 1, &control, output.path(), 60)?;
        sink.add(&[10, 20, 30, 255], 0, 30)?;
        assert!(sink.finish().is_err());
        assert!(output.as_file().metadata()?.len() <= 60);
        Ok(())
    }
}
