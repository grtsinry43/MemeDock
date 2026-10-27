using MemeDock.Bridge;

namespace MemeDock.Windows.Models;

public enum StickerOutputKind { Image, OriginalFile, FirstFrame }

public sealed record StickerOutputTarget(string Id, long Generation)
{
    public void Validate(StickerMetadata metadata)
    {
        if (metadata.Id != Id || metadata.Lifecycle.Generation != Generation)
            throw new BridgeException.Failure(ErrorCode.Conflict, "Output target changed generation.");
        if (metadata.Lifecycle.DeletedAt is not null)
            throw new BridgeException.Failure(ErrorCode.EntityDeleted, "Output target was deleted.");
    }
}

public static class StickerOutputPolicy
{
    /// <summary>
    /// <see cref="StickerOutputKind.OriginalFile"/> and <see cref="StickerOutputKind.FirstFrame"/>
    /// are explicit user choices and never follow the configured presets.
    /// </summary>
    public static ExportOptions Options(bool animated, StickerOutputKind kind, OutputDefaults defaults)
    {
        if (kind == StickerOutputKind.OriginalFile)
            return new(ExportPreset.Original, null, null, false, AnimationPolicy.Preserve);
        if (kind == StickerOutputKind.FirstFrame)
            return new(ExportPreset.CompatiblePng, 1024, null, true, AnimationPolicy.FirstFrame);
        return StickerOutputPresets.Options(defaults.For(animated), animated);
    }
}
