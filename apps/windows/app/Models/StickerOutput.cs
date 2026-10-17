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
    public static ExportOptions Options(bool animated, StickerOutputKind kind)
    {
        var original = kind == StickerOutputKind.OriginalFile || animated && kind != StickerOutputKind.FirstFrame;
        return original
            ? new(ExportPreset.Original, null, null, false, AnimationPolicy.Preserve)
            : new(ExportPreset.CompatiblePng, 1024, null, true,
                kind == StickerOutputKind.FirstFrame ? AnimationPolicy.FirstFrame : AnimationPolicy.Preserve);
    }
}
