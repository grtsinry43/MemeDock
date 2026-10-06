use memedock_domain::{
    asset::ImageFormat,
    export::{AnimationPolicy, ExportOptions, ExportPreset},
};

#[test]
fn presets_preserve_original_and_require_animation_consent()
-> Result<(), Box<dyn std::error::Error>> {
    for preset in [
        ExportPreset::Original,
        ExportPreset::CompatiblePng,
        ExportPreset::WhiteBackground,
        ExportPreset::SmallJpeg,
    ] {
        let options = ExportOptions::for_preset(preset, AnimationPolicy::Preserve);
        assert!(options.validate_source(false).is_ok());
        assert_eq!(
            options.validate_source(true).is_ok(),
            preset == ExportPreset::Original
        );
        assert!(
            ExportOptions::for_preset(preset, AnimationPolicy::FirstFrame)
                .validate_source(true)
                .is_ok()
        );
        assert_eq!(
            serde_json::from_str::<ExportOptions>(&serde_json::to_string(&options)?)?,
            options
        );
    }
    let original = ExportOptions::for_preset(ExportPreset::Original, AnimationPolicy::FirstFrame);
    assert_eq!(original.recipe(), "original-v1");
    assert_eq!(original.output_format(ImageFormat::WebP), ImageFormat::WebP);
    Ok(())
}

#[test]
fn invalid_options_are_rejected_even_through_serialization() {
    for json in [
        r#"{"preset":"original","max_edge":512,"background_rgba":null,"strip_metadata":false,"animation":"preserve"}"#,
        r#"{"preset":"small_jpeg","max_edge":512,"background_rgba":4294967040,"strip_metadata":true,"animation":"first_frame"}"#,
        r#"{"preset":"compatible_png","max_edge":0,"background_rgba":null,"strip_metadata":true,"animation":"preserve"}"#,
        r#"{"preset":"compatible_png","max_edge":1024,"background_rgba":null,"strip_metadata":false,"animation":"preserve"}"#,
    ] {
        assert!(
            serde_json::from_str::<ExportOptions>(json).is_err(),
            "{json}"
        );
    }
}

#[test]
fn recipe_isolates_actual_processing_parameters() -> Result<(), Box<dyn std::error::Error>> {
    let png = ExportOptions::for_preset(ExportPreset::CompatiblePng, AnimationPolicy::FirstFrame);
    for other in [
        ExportOptions::for_preset(ExportPreset::WhiteBackground, AnimationPolicy::FirstFrame),
        ExportOptions::for_preset(ExportPreset::SmallJpeg, AnimationPolicy::FirstFrame),
        ExportOptions::for_preset(ExportPreset::CompatiblePng, AnimationPolicy::Preserve),
        ExportOptions::new(
            ExportPreset::CompatiblePng,
            Some(512),
            None,
            true,
            AnimationPolicy::FirstFrame,
        )?,
        ExportOptions::new(
            ExportPreset::CompatiblePng,
            Some(1024),
            Some(0x123456ff),
            true,
            AnimationPolicy::FirstFrame,
        )?,
    ] {
        assert_ne!(png.recipe(), other.recipe());
    }
    Ok(())
}
