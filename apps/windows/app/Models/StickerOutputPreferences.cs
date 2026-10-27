using MemeDock.Bridge;

namespace MemeDock.Windows.Models;

public enum AppearanceMode { System, Light, Dark }

public enum OutputAction { Copy, Drag }

/// <summary>
/// Output presets for still and animated content. The two are kept separate because
/// animations cannot use the still-only presets (transparent PNG, small JPEG).
/// </summary>
public sealed record OutputDefaults(ExportPreset Still, ExportPreset Animated)
{
    public static OutputDefaults CopyDefault { get; } =
        new(ExportPreset.BestCompatibility, ExportPreset.Original);

    public static OutputDefaults DragDefault { get; } =
        new(ExportPreset.Original, ExportPreset.Original);

    public ExportPreset For(bool animated) => animated ? Animated : Still;

    public OutputDefaults WithPreset(bool animated, ExportPreset preset) => animated
        ? this with { Animated = preset }
        : this with { Still = preset };

    /// <summary>
    /// Replaces presets that are invalid for their content kind (for example a GIF
    /// preset persisted for stills) with the fallback, so a hand-edited file cannot
    /// produce an export that the domain rejects.
    /// </summary>
    public OutputDefaults Sanitized(OutputDefaults fallback) => new(
        StickerOutputPresets.Supports(Still, animated: false) ? Still : fallback.Still,
        StickerOutputPresets.Supports(Animated, animated: true) ? Animated : fallback.Animated);
}

/// <summary>
/// The presets a single output operation should use. Implemented by the preference
/// store and faked by tests.
/// </summary>
public interface IOutputDefaults
{
    OutputDefaults Copy { get; }
    OutputDefaults Drag { get; }
}

public static class StickerOutputPresets
{
    private static readonly ExportPreset[] StillChoices =
        [ExportPreset.Original, ExportPreset.CompatiblePng, ExportPreset.SmallJpeg, ExportPreset.BestCompatibility];

    private static readonly ExportPreset[] AnimatedChoices =
        [ExportPreset.Original, ExportPreset.CompatibleGif];

    private const uint OpaqueWhite = 0xFFFFFFFF;

    public static IReadOnlyList<ExportPreset> For(bool animated) => animated ? AnimatedChoices : StillChoices;

    public static bool Supports(ExportPreset preset, bool animated)
    {
        foreach (var candidate in For(animated)) if (candidate == preset) return true;
        return false;
    }

    public static string LabelKey(ExportPreset preset) => preset switch
    {
        ExportPreset.Original => "export_original",
        ExportPreset.CompatiblePng => "export_png",
        ExportPreset.SmallJpeg => "export_small",
        ExportPreset.BestCompatibility => "export_best",
        ExportPreset.CompatibleGif => "export_gif",
        _ => throw new ArgumentOutOfRangeException(nameof(preset), preset, "Unsupported output preset.")
    };

    public static string HintKey(ExportPreset preset, bool animated)
    {
        if (!Supports(preset, animated))
            throw new ArgumentOutOfRangeException(nameof(preset), preset, "Preset does not support this content kind.");
        return preset switch
        {
            ExportPreset.Original => "export_original_hint",
            ExportPreset.CompatiblePng => "export_png_hint",
            ExportPreset.SmallJpeg => "export_small_hint",
            ExportPreset.BestCompatibility => "export_best_hint",
            ExportPreset.CompatibleGif => "export_gif_hint",
            _ => throw new ArgumentOutOfRangeException(nameof(preset), preset, "Unsupported output preset.")
        };
    }

    /// <summary>
    /// Mirrors <c>crates/domain/src/export.rs::ExportOptions::for_preset</c>, which is
    /// the authority for these numbers. Keep both in sync when the domain changes.
    /// </summary>
    public static ExportOptions Options(ExportPreset preset, bool animated)
    {
        if (!Supports(preset, animated))
            throw new ArgumentOutOfRangeException(nameof(preset), preset, "Preset does not support this content kind.");
        return preset switch
        {
            ExportPreset.Original => new(ExportPreset.Original, null, null, false, AnimationPolicy.Preserve),
            ExportPreset.CompatiblePng => new(ExportPreset.CompatiblePng, 1024, null, true, AnimationPolicy.Preserve),
            ExportPreset.SmallJpeg => new(ExportPreset.SmallJpeg, 512, OpaqueWhite, true, AnimationPolicy.Preserve),
            ExportPreset.BestCompatibility => new(ExportPreset.BestCompatibility, null, null, true, AnimationPolicy.Preserve),
            ExportPreset.CompatibleGif => new(ExportPreset.CompatibleGif, 512, null, true, AnimationPolicy.Preserve),
            _ => throw new ArgumentOutOfRangeException(nameof(preset), preset, "Unsupported output preset.")
        };
    }
}
