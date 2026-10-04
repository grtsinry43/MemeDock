use crate::{
    identity::{ContentHash, StickerId},
    version::{ByteSize, TimestampMs},
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/gif")]
    Gif,
    #[serde(rename = "image/webp")]
    WebP,
}
impl ImageFormat {
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::WebP => "image/webp",
        }
    }
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::WebP => "webp",
        }
    }
}

/// Metadata of immutable bytes. Storage must verify the bytes before publishing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    hash: ContentHash,
    byte_size: ByteSize,
    mime: ImageFormat,
    width: NonZeroU32,
    height: NonZeroU32,
    animated: bool,
    created_at: TimestampMs,
}
impl Asset {
    pub fn new(
        hash: ContentHash,
        byte_size: ByteSize,
        format: ImageFormat,
        dimensions: (NonZeroU32, NonZeroU32),
        animated: bool,
        at: TimestampMs,
    ) -> Self {
        Self {
            hash,
            byte_size,
            mime: format,
            width: dimensions.0,
            height: dimensions.1,
            animated,
            created_at: at,
        }
    }
    pub const fn hash(&self) -> ContentHash {
        self.hash
    }
    pub const fn sticker_id(&self) -> StickerId {
        StickerId::new(self.hash)
    }
    pub const fn byte_size(&self) -> ByteSize {
        self.byte_size
    }
    pub const fn format(&self) -> ImageFormat {
        self.mime
    }
    pub const fn width(&self) -> u32 {
        self.width.get()
    }
    pub const fn height(&self) -> u32 {
        self.height.get()
    }
    pub const fn animated(&self) -> bool {
        self.animated
    }
    pub const fn created_at(&self) -> TimestampMs {
        self.created_at
    }
}
