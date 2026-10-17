using MemeDock.Bridge;
using MemeDock.Windows.Models;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.Storage.Streams;

namespace MemeDock.Windows.Services;

internal sealed class PreparedSticker(ArtifactLeaseHandle lease, ExportArtifact artifact) : IDisposable
{
    public ArtifactLeaseHandle Lease { get; } = lease;
    public ExportArtifact Artifact { get; } = artifact;
    public void Dispose() => Lease.Dispose();
}

internal sealed class StickerOutputService(LibrarySession session)
{
    public async Task<PreparedSticker> PrepareAsync(StickerOutputTarget target, StickerOutputKind kind,
        DataPackage package, bool drag, CancellationToken cancellation)
    {
        var detail = await session.DetailAsync(target.Id, cancellation);
        target.Validate(detail.Sticker);
        var options = StickerOutputPolicy.Options(detail.Asset.Animated, kind);
        var lease = await session.ExportAsync(target.Id, options, cancellation);
        try
        {
            // Export accepts an ID. Check its generation again before any platform handoff.
            target.Validate((await session.DetailAsync(target.Id, cancellation)).Sticker);
            var artifact = await session.PrepareHandoffAsync(lease, cancellation);
            var path = artifact.Path.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase)
                ? @"\\" + artifact.Path[8..] : artifact.Path.StartsWith(@"\\?\", StringComparison.Ordinal) ? artifact.Path[4..] : artifact.Path;
            var file = await StorageFile.GetFileFromPathAsync(path);
            cancellation.ThrowIfCancellationRequested();
            package.SetStorageItems(new IStorageItem[] { file }, readOnly: true);
            package.RequestedOperation = DataPackageOperation.Copy;
            package.Properties.Title = detail.Sticker.Title;
            if (!drag && kind != StickerOutputKind.OriginalFile && !artifact.Animated && artifact.Mime == "image/png")
                package.SetBitmap(RandomAccessStreamReference.CreateFromFile(file));
            return new PreparedSticker(lease, artifact);
        }
        catch { lease.Dispose(); throw; }
    }
}
