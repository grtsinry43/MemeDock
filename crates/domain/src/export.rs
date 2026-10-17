//! Versioned, platform-independent rules for immutable export outputs.
use crate::{asset::ImageFormat, error::DomainError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportPreset {
    Original,
    CompatiblePng,
    WhiteBackground,
    SmallJpeg,
    CompatibleGif,
    BestCompatibility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationPolicy {
    Preserve,
    FirstFrame,
}

/// Validated options. Deserialization performs the same checks as construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "OptionsWire", into = "OptionsWire")]
pub struct ExportOptions {
    preset: ExportPreset,
    max_edge: Option<u32>,
    background_rgba: Option<u32>,
    strip_metadata: bool,
    animation: AnimationPolicy,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionsWire {
    preset: ExportPreset,
    max_edge: Option<u32>,
    background_rgba: Option<u32>,
    strip_metadata: bool,
    animation: AnimationPolicy,
}

impl ExportOptions {
    /// Preserve is deliberate: animated conversion requires explicit consent.
    pub const fn for_preset(preset: ExportPreset, animation: AnimationPolicy) -> Self {
        Self {
            preset,
            max_edge: match preset {
                ExportPreset::Original
                | ExportPreset::WhiteBackground
                | ExportPreset::BestCompatibility => None,
                ExportPreset::CompatiblePng => Some(1024),
                ExportPreset::SmallJpeg | ExportPreset::CompatibleGif => Some(512),
            },
            background_rgba: match preset {
                ExportPreset::WhiteBackground | ExportPreset::SmallJpeg => Some(0xffff_ffff),
                _ => None,
            },
            strip_metadata: !matches!(preset, ExportPreset::Original),
            animation: if matches!(preset, ExportPreset::Original | ExportPreset::CompatibleGif) {
                AnimationPolicy::Preserve
            } else {
                animation
            },
        }
    }

    pub fn new(
        preset: ExportPreset,
        max_edge: Option<u32>,
        background_rgba: Option<u32>,
        strip_metadata: bool,
        animation: AnimationPolicy,
    ) -> Result<Self, DomainError> {
        if max_edge == Some(0) {
            return Err(DomainError::InvalidValue("export max_edge"));
        }
        if preset == ExportPreset::BestCompatibility
            && (max_edge.is_some() || background_rgba.is_some() || !strip_metadata)
        {
            return Err(DomainError::InvalidValue(
                "best compatibility export options",
            ));
        }
        if preset == ExportPreset::CompatibleGif
            && (max_edge != Some(512)
                || background_rgba.is_some()
                || !strip_metadata
                || animation != AnimationPolicy::Preserve)
        {
            return Err(DomainError::InvalidValue("GIF export options"));
        }
        if matches!(preset, ExportPreset::Original) {
            if max_edge.is_some()
                || background_rgba.is_some()
                || strip_metadata
                || animation != AnimationPolicy::Preserve
            {
                return Err(DomainError::InvalidValue("original export options"));
            }
        } else {
            if !strip_metadata {
                return Err(DomainError::InvalidValue(
                    "derived metadata preservation unsupported",
                ));
            }
            if matches!(
                preset,
                ExportPreset::WhiteBackground | ExportPreset::SmallJpeg
            ) && background_rgba.is_none_or(|rgba| rgba & 0xff != 0xff)
            {
                return Err(DomainError::InvalidValue(
                    "opaque export background required",
                ));
            }
        }
        Ok(Self {
            preset,
            max_edge,
            background_rgba,
            strip_metadata,
            animation,
        })
    }

    pub fn validate_source(self, animated: bool) -> Result<(), DomainError> {
        if self.preset == ExportPreset::BestCompatibility {
            return Ok(());
        }
        if self.preset == ExportPreset::CompatibleGif {
            return if animated {
                Ok(())
            } else {
                Err(DomainError::InvalidValue("GIF export requires animation"))
            };
        }
        if animated
            && self.preset != ExportPreset::Original
            && self.animation != AnimationPolicy::FirstFrame
        {
            return Err(DomainError::InvalidValue(
                "animated export requires first frame consent",
            ));
        }
        Ok(())
    }
    /// Resolve adaptive policy in the domain, before cache lookup or encoding.
    /// Legacy PNG requests retain their explicitly static behavior.
    pub fn resolve_source(self, animated: bool) -> Result<Self, DomainError> {
        self.validate_source(animated)?;
        if self.preset != ExportPreset::BestCompatibility {
            return Ok(self);
        }
        Ok(Self::for_preset(
            if animated && self.animation == AnimationPolicy::Preserve {
                ExportPreset::CompatibleGif
            } else {
                ExportPreset::CompatiblePng
            },
            self.animation,
        ))
    }
    pub const fn preset(self) -> ExportPreset {
        self.preset
    }
    pub const fn max_edge(self) -> Option<u32> {
        self.max_edge
    }
    pub const fn background_rgba(self) -> Option<u32> {
        self.background_rgba
    }
    pub const fn animation(self) -> AnimationPolicy {
        self.animation
    }
    pub const fn output_format(self, source: ImageFormat, animated: bool) -> ImageFormat {
        match self.preset {
            ExportPreset::Original => source,
            ExportPreset::SmallJpeg => ImageFormat::Jpeg,
            ExportPreset::CompatibleGif => ImageFormat::Gif,
            ExportPreset::BestCompatibility
                if animated && matches!(self.animation, AnimationPolicy::Preserve) =>
            {
                ImageFormat::Gif
            }
            _ => ImageFormat::Png,
        }
    }
    /// Includes actual parameters and processing/encoder policy versions.
    /// Changing orientation, resizing, compositing or JPEG quality bumps v1.
    pub fn recipe(self) -> String {
        if self.preset == ExportPreset::Original {
            return "original-v1".into();
        }
        if self.preset == ExportPreset::CompatibleGif {
            return "gif-v2:edge=512:resize=lanczos3:alpha=128:fps=max30:timing=cumulative-centiseconds:repeat=source:quantizer=neuquant:speed=10:metadata=strip".into();
        }
        if self.preset == ExportPreset::BestCompatibility {
            return "best-compatibility-v1:static=png1024:animated=gif512-max30:metadata=strip"
                .into();
        }
        format!(
            "derived-v1:{}:edge={}:rgba={}:metadata=strip:animation={}:resize=lanczos3:jpeg=85",
            match self.preset {
                ExportPreset::Original => "original",
                ExportPreset::CompatiblePng => "compatible_png",
                ExportPreset::WhiteBackground => "white_background",
                ExportPreset::SmallJpeg => "small_jpeg",
                ExportPreset::CompatibleGif => "compatible_gif",
                ExportPreset::BestCompatibility => "best_compatibility",
            },
            self.max_edge
                .map_or_else(|| "none".into(), |v| v.to_string()),
            self.background_rgba
                .map_or_else(|| "none".into(), |v| format!("{v:08x}")),
            match self.animation {
                AnimationPolicy::Preserve => "preserve",
                AnimationPolicy::FirstFrame => "first_frame",
            }
        )
    }
}

impl TryFrom<OptionsWire> for ExportOptions {
    type Error = DomainError;
    fn try_from(w: OptionsWire) -> Result<Self, Self::Error> {
        Self::new(
            w.preset,
            w.max_edge,
            w.background_rgba,
            w.strip_metadata,
            w.animation,
        )
    }
}
impl From<ExportOptions> for OptionsWire {
    fn from(v: ExportOptions) -> Self {
        Self {
            preset: v.preset,
            max_edge: v.max_edge,
            background_rgba: v.background_rgba,
            strip_metadata: v.strip_metadata,
            animation: v.animation,
        }
    }
}
