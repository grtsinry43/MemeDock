using MemeDock.Bridge;
using MemeDock.Windows.Models;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.Storage.Streams;

namespace MemeDock.Windows.Services;

internal sealed class PreparedSticker(ArtifactLeaseHandle lease, ExportArtifact artifact, bool ownsLease = true) : IDisposable
{
    public ArtifactLeaseHandle Lease { get; } = lease;
    public ExportArtifact Artifact { get; } = artifact;
    public void Dispose()
    {
        if (ownsLease) Lease.Dispose();
    }
}

internal sealed class StickerOutputService(LibrarySession session, IOutputDefaults defaults)
{
    public static string NormalizePath(string path) => path.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase)
        ? @"\\" + path[8..] : path.StartsWith(@"\\?\", StringComparison.Ordinal) ? path[4..] : path;

    public async Task<PreparedSticker> PrepareLeaseAsync(ArtifactLeaseHandle lease, string title,
        DataPackage package, CancellationToken cancellation)
    {
        var artifact = await session.PrepareHandoffAsync(lease, cancellation);
        var file = await StorageFile.GetFileFromPathAsync(NormalizePath(artifact.Path));
        cancellation.ThrowIfCancellationRequested();
        package.SetStorageItems(new IStorageItem[] { file }, readOnly: true);
        package.RequestedOperation = DataPackageOperation.Copy;
        package.SetData(StickerImportService.InternalDragFormat, true);
        package.Properties.Title = string.IsNullOrWhiteSpace(title) ? artifact.FileName : title;
        return new PreparedSticker(lease, artifact, ownsLease: false);
    }

    public async Task<PreparedSticker> PrepareAsync(StickerOutputTarget target, StickerOutputKind kind,
        DataPackage package, bool drag, CancellationToken cancellation, GifLimitOverride? gifLimitOverride = null)
    {
        var detail = await session.DetailAsync(target.Id, cancellation);
        target.Validate(detail.Sticker);
        // Read the presets once per operation so a later change applies to the next
        // operation instead of mutating one that is already running.
        var gifMaxBytes = gifLimitOverride is { } over ? over.MaxBytes : defaults.GifMaxBytes;
        var options = StickerOutputPolicy.Options(detail.Asset.Animated, kind,
            drag ? defaults.Drag : defaults.Copy, gifMaxBytes);
        var lease = await session.ExportAsync(target.Id, options, cancellation);
        try
        {
            // Export accepts an ID. Check its generation again before any platform handoff.
            target.Validate((await session.DetailAsync(target.Id, cancellation)).Sticker);
            var artifact = await session.PrepareHandoffAsync(lease, cancellation);
            var file = await StorageFile.GetFileFromPathAsync(NormalizePath(artifact.Path));
            cancellation.ThrowIfCancellationRequested();
            package.SetStorageItems(new IStorageItem[] { file }, readOnly: true);
            package.RequestedOperation = DataPackageOperation.Copy;
            if (drag) package.SetData(StickerImportService.InternalDragFormat, true);
            package.Properties.Title = detail.Sticker.Title;
            if (!drag && kind != StickerOutputKind.OriginalFile && !artifact.Animated && artifact.Mime == "image/png")
                package.SetBitmap(RandomAccessStreamReference.CreateFromFile(file));
            return new PreparedSticker(lease, artifact);
        }
        catch { lease.Dispose(); throw; }
    }
}
