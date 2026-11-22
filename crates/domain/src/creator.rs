//! Validated, platform-independent video editing parameters.
use crate::error::DomainError;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoCrop {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
impl VideoCrop {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Result<Self, DomainError> {
        if ![x, y, width, height].iter().all(|v| v.is_finite())
            || x < 0.0
            || y < 0.0
            || width <= 0.0
            || height <= 0.0
            || x + width > 1.0 + 1e-9
            || y + height > 1.0 + 1e-9
        {
            return Err(DomainError::InvalidValue("video crop"));
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }
    pub fn x(self) -> f64 {
        self.x
    }
    pub fn y(self) -> f64 {
        self.y
    }
    pub fn width(self) -> f64 {
        self.width
    }
    pub fn height(self) -> f64 {
        self.height
    }
}
impl Default for VideoCrop {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoEdit {
    start_ms: u64,
    end_ms: u64,
    crop: VideoCrop,
    max_edge: u32,
    fps: u32,
    quality: u8,
}
impl VideoEdit {
    pub fn new(
        start_ms: u64,
        end_ms: u64,
        crop: VideoCrop,
        max_edge: u32,
        fps: u32,
        quality: u8,
    ) -> Result<Self, DomainError> {
        if start_ms >= end_ms
            || end_ms - start_ms > 30_000
            || end_ms > 86_400_000
            || !(32..=512).contains(&max_edge)
            || !(1..=30).contains(&fps)
            || !(1..=100).contains(&quality)
        {
            return Err(DomainError::InvalidValue("video edit"));
        }
        Ok(Self {
            start_ms,
            end_ms,
            crop,
            max_edge,
            fps,
            quality,
        })
    }
    pub fn start_ms(self) -> u64 {
        self.start_ms
    }
    pub fn end_ms(self) -> u64 {
        self.end_ms
    }
    pub fn duration_ms(self) -> u64 {
        self.end_ms - self.start_ms
    }
    pub fn crop(self) -> VideoCrop {
        self.crop
    }
    pub fn max_edge(self) -> u32 {
        self.max_edge
    }
    pub fn fps(self) -> u32 {
        self.fps
    }
    pub fn quality(self) -> u8 {
        self.quality
    }
}
